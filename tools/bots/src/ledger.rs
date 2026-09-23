//! 계측의 핵심. **IO 가 없고 시계를 읽지 않는다** — 모든 시각은 호출자가 마이크로초로 넣는다.
//!
//! 그래서 `tests/ledger_accounting.rs` 가 서버 없이 합성 프레임으로 이 로직을 검증할 수 있다.
//! 봇 하네스가 틀리면 슬라이스의 판정이 통째로 거짓이 되므로, 계측기 자체에 테스트가 있어야 한다.
//!
//! 판정 기준(스프린트 계약 SC-53·54·55):
//! - **손실** = 보냈는데 `COMMAND_RESULT` 가 오지 않은 명령 수 (`missing_results`)
//! - **1:1** = `sent_total == results_total` 이고 중복 0, 미대응 0
//! - **순서**(I-15) = 그 명령의 `COMMAND_RESULT` **뒤에** `PING_REPLY` 가 왔는가
//! - **왕복 지연** = `PING_REPLY` 시각 − `PING_SERVER` 송신 시각, **봇의 단조 시계**로만
//!   (`client_sent_at` 도 서버 `tick` 도 쓰지 않는다 — I-11, ADR-0006 §4)

use std::collections::BTreeMap;

use serde::Serialize;
use uuid::Uuid;

use crate::stats;

/// 서버가 먼저 닫을 때의 close code → `SESSION_CLOSED.close_reason` (ADR-0005 §2 표).
///
/// 봇이 DB 의 `close_reason` 을 대신하지는 않는다(판정 정본은 DB). 다만 **표에 없는 code 는 조용히
/// 넘기지 않고 `UNKNOWN` 으로 드러낸다** — 새 code(예: 4001)가 생겼는데 도구가 모르면 그 경로가
/// 탔는지 아무도 모른다(계약 §7a). `1001` 은 두 사유(`IDLE_TIMEOUT`·`SERVER_SHUTDOWN`)가 공유한다.
pub fn close_code_meaning(code: u16) -> &'static str {
    match code {
        1000 => "CLIENT_CLOSED",
        1001 => "IDLE_TIMEOUT|SERVER_SHUTDOWN",
        1002 => "PROTOCOL_VIOLATION",
        1011 => "SLOW_CONSUMER",
        4001 => "SUPERSEDED",
        _ => "UNKNOWN",
    }
}

pub const STATUS_ACCEPTED: &str = "ACCEPTED";
pub const STATUS_REJECTED: &str = "REJECTED";

#[derive(Debug, Clone)]
struct CommandRecord {
    probe_seq: u32,
    /// 이 명령이 `PING_REPLY` 를 기대하는가.
    ///
    /// **`PING_SERVER` 만 true 다.** 정본은 스펙 `docs/specs/p1-01-ship-movement.md` **§5.1a**
    /// (명령 → 기대 응답 규범 표): `SET_SHIP_CONTROL` 은 `COMMAND_RESULT` 만 내고 `PING_REPLY` 를
    /// 내지 않는다. 명령 타입이 늘면 **그 표를 먼저 보고** 여기를 고친다. 이 구분이 없으면
    /// `accepted == replies_total` 게이트가 **수락된 조작 명령마다 거짓 실패**한다.
    expects_ping_reply: bool,
    sent_us: u64,
    sent_count: u32,
    result_us: Option<u64>,
    /// `COMMAND_RESULT` envelope 의 tick. AC-6(c)는 `PING_REPLY` 와 **같은 tick**을 요구한다.
    result_tick: Option<u64>,
    result_count: u32,
    status: Option<String>,
    reason_code: Option<String>,
    reply_us: Option<u64>,
    reply_tick: Option<u64>,
    reply_count: u32,
    order_violation: bool,
    tick_mismatch: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRecord {
    pub bot: String,
    pub session_id: Uuid,
    pub correlation_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub tick_hz: u32,
    pub server_version: String,
    pub ready_tick: u64,
    pub connected_at_us: u64,
    pub ready_at_us: u64,
    pub closed_at_us: Option<u64>,
    /// 우리가 관측한 close code.
    pub close_code: Option<u16>,
    /// 서버가 보낸 Close 프레임의 code (교환의 반대편). SC-28 의 close code 표는 이 값을 본다.
    pub peer_close_code: Option<u16>,
    pub close_reason_text: Option<String>,
    /// `client` = 봇이 먼저 Close 를 보냈다 / `server` = 서버가 먼저 보냈다 / `error` = 소켓 오류.
    pub close_initiator: String,
}

/// 한 연결(=한 세션)의 계측.
#[derive(Debug)]
pub struct Ledger {
    pub bot: String,
    commands: BTreeMap<Uuid, CommandRecord>,
    sent_total: u64,
    results_total: u64,
    replies_total: u64,
    accepted: u64,
    /// ACCEPTED 중 **`PING_REPLY` 를 기대하는** 명령 수. `replies_total` 의 짝이다.
    accepted_expecting_reply: u64,
    rejected: u64,
    rejected_by_reason: BTreeMap<String, u64>,
    unknown_status: u64,
    duplicate_results: u64,
    duplicate_replies: u64,
    unmatched_results: u64,
    unmatched_replies: u64,
    probe_seq_mismatches: u64,
    order_violations: u64,
    /// AC-6(c) 위반: `COMMAND_RESULT` 와 `PING_REPLY` 의 envelope tick 이 다른 수.
    tick_mismatches: u64,
    unknown_message_types: BTreeMap<String, u64>,
    wire_errors: Vec<String>,
    pub session: Option<SessionRecord>,
    pub errors: Vec<String>,
    /// probe 가 붙이는 이름표 `(표지, command_id)`. 분석이 "어느 명령이 어느 단계였나"를 알게 한다
    /// (예: `range_turn` 의 주입 프레임·선회 시작). 판정 게이트에는 쓰지 않는다.
    pub marks: Vec<(String, Uuid)>,
}

/// 한 명령의 결과 요약 (`Ledger::result_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub status: String,
    pub reason_code: Option<String>,
    pub tick: u64,
}

impl Ledger {
    pub fn new(bot: impl Into<String>) -> Self {
        Self {
            bot: bot.into(),
            commands: BTreeMap::new(),
            sent_total: 0,
            results_total: 0,
            replies_total: 0,
            accepted: 0,
            accepted_expecting_reply: 0,
            rejected: 0,
            rejected_by_reason: BTreeMap::new(),
            unknown_status: 0,
            duplicate_results: 0,
            duplicate_replies: 0,
            unmatched_results: 0,
            unmatched_replies: 0,
            probe_seq_mismatches: 0,
            order_violations: 0,
            tick_mismatches: 0,
            unknown_message_types: BTreeMap::new(),
            wire_errors: Vec::new(),
            session: None,
            errors: Vec::new(),
            marks: Vec::new(),
        }
    }

    pub fn mark(&mut self, label: &str, command_id: Uuid) {
        self.marks.push((label.to_owned(), command_id));
    }

    /// 표지가 `label` 인 명령들의 `(probe_seq, 결과)`. 결과가 없으면 `None` 이다(빠뜨리지 않는다).
    pub fn marked_outcomes(&self, label: &str) -> Vec<(u32, Option<CommandOutcome>)> {
        self.marks
            .iter()
            .filter(|(l, _)| l == label)
            .filter_map(|(_, id)| {
                let seq = self.commands.get(id)?.probe_seq;
                Some((seq, self.result_of(*id)))
            })
            .collect()
    }

    pub fn result_of(&self, command_id: Uuid) -> Option<CommandOutcome> {
        let r = self.commands.get(&command_id)?;
        Some(CommandOutcome {
            status: r.status.clone()?,
            reason_code: r.reason_code.clone(),
            tick: r.result_tick?,
        })
    }

    pub fn on_session_ready(&mut self, rec: SessionRecord) {
        if self.session.is_some() {
            // 한 연결에 SESSION_READY 는 정확히 1건이어야 한다(ADR-0005 §3).
            self.errors
                .push("SESSION_READY received more than once on one connection".to_owned());
        }
        self.session = Some(rec);
    }

    /// 종료 기록. **먼저 쓴 쪽이 이긴다.**
    ///
    /// 정상 종료는 Close 프레임 **교환**이다(ADR-0005 §2): 봇이 Close 를 보내고, 서버가 Close 로
    /// 답한다. 나중 값으로 덮어쓰면 우리가 먼저 닫은 연결이 전부 `initiator=server` 로 기록되어
    /// **SC-52("서버가 먼저 닫은 연결 0건")가 모든 정상 세션에서 거짓 FAIL** 이 된다.
    /// 실서버 probe 에서 실제로 그렇게 나왔다(2026-09-19). 지금은 첫 기록만 남긴다.
    ///
    /// close code 는 **서버가 보낸 프레임에서 읽은 값을 우선**한다 — 우리가 보낸 1000 보다
    /// 서버가 말한 코드가 판정에 쓰인다(SC-28).
    pub fn on_close(
        &mut self,
        at_us: u64,
        code: Option<u16>,
        reason: Option<String>,
        initiator: &str,
    ) {
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if s.close_initiator == "none" {
            s.closed_at_us = Some(at_us);
            s.close_initiator = initiator.to_owned();
            s.close_code = code;
            s.close_reason_text = reason;
            return;
        }
        // 이미 누가 먼저 닫았는지는 정해졌다. 상대의 Close 프레임에서 온 code/reason 만 채운다.
        if initiator == "server" {
            if let Some(c) = code {
                s.peer_close_code = Some(c);
            }
            if s.close_code.is_none() {
                s.close_code = code;
            }
            if s.close_reason_text.is_none() {
                s.close_reason_text = reason;
            }
        }
    }

    /// `PING_SERVER` 송신 기록 — `PING_REPLY` 를 기대한다.
    pub fn on_sent(&mut self, command_id: Uuid, probe_seq: u32, at_us: u64) {
        self.on_sent_inner(command_id, probe_seq, at_us, true);
    }

    /// `SET_SHIP_CONTROL` 처럼 **`COMMAND_RESULT` 만 받는** 명령의 송신 기록.
    pub fn on_sent_no_reply(&mut self, command_id: Uuid, probe_seq: u32, at_us: u64) {
        self.on_sent_inner(command_id, probe_seq, at_us, false);
    }

    fn on_sent_inner(
        &mut self,
        command_id: Uuid,
        probe_seq: u32,
        at_us: u64,
        expects_ping_reply: bool,
    ) {
        self.sent_total += 1;
        self.commands
            .entry(command_id)
            .and_modify(|r| r.sent_count += 1)
            .or_insert(CommandRecord {
                probe_seq,
                expects_ping_reply,
                sent_us: at_us,
                sent_count: 1,
                result_us: None,
                result_tick: None,
                result_count: 0,
                status: None,
                reason_code: None,
                reply_us: None,
                reply_tick: None,
                reply_count: 0,
                order_violation: false,
                tick_mismatch: false,
            });
    }

    pub fn on_command_result(
        &mut self,
        command_id: Uuid,
        status: &str,
        reason_code: Option<&str>,
        at_us: u64,
        tick: u64,
    ) {
        self.results_total += 1;
        match status {
            STATUS_ACCEPTED => {
                self.accepted += 1;
                if self
                    .commands
                    .get(&command_id)
                    .is_some_and(|r| r.expects_ping_reply)
                {
                    self.accepted_expecting_reply += 1;
                }
            }
            STATUS_REJECTED => {
                self.rejected += 1;
                let key = reason_code.unwrap_or("<null>").to_owned();
                *self.rejected_by_reason.entry(key).or_insert(0) += 1;
            }
            // 닫힌 집합 밖의 값은 죽지 않고 드러낸다(ADR-0005 §4).
            _ => self.unknown_status += 1,
        }
        match self.commands.get_mut(&command_id) {
            None => self.unmatched_results += 1,
            Some(rec) => {
                rec.result_count += 1;
                if rec.result_count == 1 {
                    rec.result_us = Some(at_us);
                    rec.result_tick = Some(tick);
                    rec.status = Some(status.to_owned());
                    rec.reason_code = reason_code.map(str::to_owned);
                } else {
                    self.duplicate_results += 1;
                }
            }
        }
    }

    pub fn on_ping_reply(&mut self, command_id: Uuid, probe_seq: u32, at_us: u64, tick: u64) {
        self.replies_total += 1;
        match self.commands.get_mut(&command_id) {
            None => self.unmatched_replies += 1,
            Some(rec) => {
                rec.reply_count += 1;
                if rec.probe_seq != probe_seq {
                    self.probe_seq_mismatches += 1;
                }
                if rec.reply_count == 1 {
                    rec.reply_us = Some(at_us);
                    rec.reply_tick = Some(tick);
                    // AC-6(c): 접수와 타입별 결과는 **같은 tick**에서 만들어진다.
                    if let Some(rt) = rec.result_tick
                        && rt != tick
                    {
                        rec.tick_mismatch = true;
                        self.tick_mismatches += 1;
                    }
                    // I-15: COMMAND_RESULT 가 먼저 와야 한다. 아직 없으면 위반이다.
                    if rec.result_us.is_none() {
                        rec.order_violation = true;
                        self.order_violations += 1;
                    }
                } else {
                    self.duplicate_replies += 1;
                }
            }
        }
    }

    pub fn on_unknown_message(&mut self, message_type: &str) {
        *self
            .unknown_message_types
            .entry(message_type.to_owned())
            .or_insert(0) += 1;
    }

    pub fn on_wire_error(&mut self, reason: String) {
        if self.wire_errors.len() < 20 {
            self.wire_errors.push(reason);
        }
    }

    pub fn on_error(&mut self, msg: String) {
        self.errors.push(msg);
    }

    /// CSV 한 줄씩 (`commands.csv`). 헤더는 [`COMMANDS_CSV_HEADER`].
    pub fn command_rows(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(|(id, r)| {
                format!(
                    "{bot},{id},{seq},{sent},{result},{status},{reason},{reply},{order},{rtt}",
                    bot = self.bot,
                    seq = r.probe_seq,
                    sent = r.sent_us,
                    result = r.result_us.map(|v| v.to_string()).unwrap_or_default(),
                    status = r.status.clone().unwrap_or_default(),
                    reason = r.reason_code.clone().unwrap_or_default(),
                    reply = r.reply_us.map(|v| v.to_string()).unwrap_or_default(),
                    order = if r.order_violation {
                        "VIOLATION"
                    } else if r.tick_mismatch {
                        "TICK_MISMATCH"
                    } else {
                        "ok"
                    },
                    rtt = r
                        .reply_us
                        .map(|v| format!("{:.3}", (v.saturating_sub(r.sent_us)) as f64 / 1000.0))
                        .unwrap_or_default(),
                )
            })
            .collect()
    }

    /// 창(window) 안에서 **송신**된 명령만 집계한다. AC-19(b)(DB 중단 구간에도 왕복이
    /// 계속됐는가)를 재는 데 쓴다. 경계는 `[from_us, to_us)`.
    pub fn round_trips_in_window(&self, from_us: u64, to_us: u64) -> (u64, u64) {
        let mut sent = 0;
        let mut completed = 0;
        for r in self.commands.values() {
            if r.sent_us >= from_us && r.sent_us < to_us {
                sent += 1;
                if r.reply_us.is_some() {
                    completed += 1;
                }
            }
        }
        (sent, completed)
    }

    pub fn rtt_samples_ms(&self) -> Vec<f64> {
        self.commands
            .values()
            .filter_map(|r| {
                r.reply_us
                    .map(|v| (v.saturating_sub(r.sent_us)) as f64 / 1000.0)
            })
            .collect()
    }

    pub fn ack_samples_ms(&self) -> Vec<f64> {
        self.commands
            .values()
            .filter_map(|r| {
                r.result_us
                    .map(|v| (v.saturating_sub(r.sent_us)) as f64 / 1000.0)
            })
            .collect()
    }

    pub fn finish(&self) -> LedgerSummary {
        let mut missing_results = 0;
        let mut missing_replies = 0;
        for r in self.commands.values() {
            if r.result_count == 0 {
                missing_results += 1;
            }
            // **`expects_ping_reply` 를 빠뜨리면 같은 버그가 여기서 되살아난다.**
            // `accepted_reply_pairing_holds` 를 고치고 이 줄을 그대로 두는 바람에
            // 라운드 3 의 회귀 테스트가 한 번 더 빨간불을 냈다 — 응답을 내지 않는
            // 명령은 "응답 누락"이 아니다.
            if r.expects_ping_reply
                && r.status.as_deref() == Some(STATUS_ACCEPTED)
                && r.reply_count == 0
            {
                missing_replies += 1;
            }
        }
        LedgerSummary {
            bot: self.bot.clone(),
            distinct_command_ids: self.commands.len() as u64,
            sent_total: self.sent_total,
            results_total: self.results_total,
            replies_total: self.replies_total,
            accepted: self.accepted,
            accepted_expecting_reply: self.accepted_expecting_reply,
            rejected: self.rejected,
            rejected_by_reason: self.rejected_by_reason.clone(),
            unknown_status: self.unknown_status,
            missing_results,
            missing_replies,
            duplicate_results: self.duplicate_results,
            duplicate_replies: self.duplicate_replies,
            unmatched_results: self.unmatched_results,
            unmatched_replies: self.unmatched_replies,
            probe_seq_mismatches: self.probe_seq_mismatches,
            order_violations: self.order_violations,
            tick_mismatches: self.tick_mismatches,
            ticks_compared: self
                .commands
                .values()
                .filter(|r| r.result_tick.is_some() && r.reply_tick.is_some())
                .count() as u64,
            unknown_message_types: self.unknown_message_types.clone(),
            wire_errors: self.wire_errors.clone(),
            errors: self.errors.clone(),
            rtt: stats::summarize(self.rtt_samples_ms()),
            ack: stats::summarize(self.ack_samples_ms()),
        }
    }
}

pub const COMMANDS_CSV_HEADER: &str =
    "bot,command_id,probe_seq,sent_us,result_us,status,reason_code,reply_us,order,rtt_ms";

#[derive(Debug, Clone, Serialize)]
pub struct LedgerSummary {
    pub bot: String,
    pub distinct_command_ids: u64,
    pub sent_total: u64,
    pub results_total: u64,
    pub replies_total: u64,
    pub accepted: u64,
    /// ACCEPTED 중 `PING_REPLY` 를 기대하는 명령 수.
    pub accepted_expecting_reply: u64,
    pub rejected: u64,
    pub rejected_by_reason: BTreeMap<String, u64>,
    pub unknown_status: u64,
    /// **손실**: 보냈는데 `COMMAND_RESULT` 가 오지 않은 `command_id` 수.
    pub missing_results: u64,
    /// `ACCEPTED` 인데 `PING_REPLY` 가 오지 않은 수.
    pub missing_replies: u64,
    pub duplicate_results: u64,
    pub duplicate_replies: u64,
    pub unmatched_results: u64,
    pub unmatched_replies: u64,
    pub probe_seq_mismatches: u64,
    /// I-15 위반: `PING_REPLY` 가 그 명령의 `COMMAND_RESULT` 보다 먼저 도착한 수.
    pub order_violations: u64,
    /// AC-6(c) 위반: 두 메시지의 envelope tick 이 다른 수.
    pub tick_mismatches: u64,
    /// tick 을 실제로 비교한 명령 수(빈 순회 방지 — 0이면 아무것도 확인하지 않은 것이다).
    pub ticks_compared: u64,
    pub unknown_message_types: BTreeMap<String, u64>,
    pub wire_errors: Vec<String>,
    pub errors: Vec<String>,
    pub rtt: stats::Summary,
    pub ack: stats::Summary,
}

impl LedgerSummary {
    /// SC-53 의 판정: 보낸 수 == 받은 수, 중복 0, 미대응 0, 손실 0.
    pub fn one_to_one_holds(&self) -> bool {
        self.sent_total == self.results_total
            && self.missing_results == 0
            && self.duplicate_results == 0
            && self.unmatched_results == 0
    }

    /// SC-54 의 판정: ACCEPTED 수 == PING_REPLY 수, 중복 0, probe_seq 불일치 0.
    /// AC-6(c): 비교한 건이 있고 불일치가 없어야 한다.
    pub fn same_tick_holds(&self) -> bool {
        self.ticks_compared > 0 && self.tick_mismatches == 0
    }

    /// ACCEPTED 중 **응답을 기대하는 것만** `PING_REPLY` 와 1:1 이어야 한다.
    ///
    /// **라운드 2 에서 이 게이트가 거짓 통과했다**: 옛 판정은 `accepted == replies_total` 이었는데,
    /// `SET_SHIP_CONTROL` 은 `PING_REPLY` 를 내지 않으므로 **명령이 하나도 수락되지 않을 때만**
    /// (`0 == 0`) 통과하는 검사였다. 서버가 조작 명령을 전부 거부하던 동안 조용히 초록이었고,
    /// 고쳐진 뒤에야 `accepted=199, replies=0` 으로 거짓 실패가 됐다(계약 §3.3 — 도구가 틀리면
    /// 빨간불이 켜져야 하는데, 이 항목은 **틀렸는데 초록불**이었다).
    pub fn accepted_reply_pairing_holds(&self) -> bool {
        self.accepted_expecting_reply == self.replies_total
            && self.missing_replies == 0
            && self.duplicate_replies == 0
            && self.unmatched_replies == 0
            && self.probe_seq_mismatches == 0
    }

    /// 스펙 §5.1a 공통 규칙: 판정에 들어간 명령은 수락이든 거부든 **정확히 하나의**
    /// `COMMAND_RESULT` 를 낳는다. 받은 결과는 전부 둘 중 하나로 분류돼야 한다
    /// (닫힌 집합 밖의 `status` 가 섞이면 여기서 깨진다).
    pub fn results_partition_holds(&self) -> bool {
        self.results_total == self.accepted + self.rejected
    }

    /// **위 두 항등식의 짝.** 스펙 §5.1a "게이트를 쓸 때의 의무"(architect R3 판정 2).
    ///
    /// `results_partition_holds` 와 `accepted_reply_pairing_holds` 는 입력이 전부 0 이면
    /// `0 == 0 + 0`, `0 == 0` 으로 **자명하게 성립한다.** 라운드 2 의 짝 게이트가 바로 그렇게
    /// 초록불이었다 — 서버가 조작 명령 200 건을 전부 `UNKNOWN_COMMAND_TYPE` 으로 거부하는 동안
    /// `accepted = 0` 이었고, **어떤 응답 표를 참조했든 통과했을 것이다.** 수락 경로가 한 번도
    /// 타지 않았으면 그 게이트는 아무것도 검사하지 않은 것이다.
    pub fn accepted_path_exercised(&self) -> bool {
        self.accepted > 0
    }

    /// 스펙 §5.1a 에서 유도한 명령→응답 게이트 **3단언**. 입력이 전부 0 이면 반드시 false 다.
    ///
    /// | 단언 | 대상 |
    /// |------|------|
    /// | `results_total == accepted + rejected` | 모든 명령 (1:1) |
    /// | `replies_total == accepted_expecting_reply` | 타입별 응답 (`PING_SERVER` 만 `PING_REPLY`) |
    /// | `accepted > 0` | 위 둘이 자명하게 성립하는 상태를 통과로 읽지 않는다 |
    pub fn command_reply_gates_hold(&self) -> bool {
        self.results_partition_holds()
            && self.accepted_reply_pairing_holds()
            && self.accepted_path_exercised()
    }
}

/// 여러 봇의 요약을 합친다. 지연 분포는 **표본을 합쳐서** 다시 계산해야 하므로
/// (요약끼리 p99 를 평균내면 거짓이 된다) 표본을 따로 받는다.
pub fn aggregate(
    summaries: &[LedgerSummary],
    rtt_samples: Vec<f64>,
    ack_samples: Vec<f64>,
) -> LedgerSummary {
    let mut out = LedgerSummary {
        bot: format!("<aggregate of {}>", summaries.len()),
        distinct_command_ids: 0,
        sent_total: 0,
        results_total: 0,
        replies_total: 0,
        accepted: 0,
        accepted_expecting_reply: 0,
        rejected: 0,
        rejected_by_reason: BTreeMap::new(),
        unknown_status: 0,
        missing_results: 0,
        missing_replies: 0,
        duplicate_results: 0,
        duplicate_replies: 0,
        unmatched_results: 0,
        unmatched_replies: 0,
        probe_seq_mismatches: 0,
        order_violations: 0,
        tick_mismatches: 0,
        ticks_compared: 0,
        unknown_message_types: BTreeMap::new(),
        wire_errors: Vec::new(),
        errors: Vec::new(),
        rtt: stats::summarize(rtt_samples),
        ack: stats::summarize(ack_samples),
    };
    for s in summaries {
        out.distinct_command_ids += s.distinct_command_ids;
        out.sent_total += s.sent_total;
        out.results_total += s.results_total;
        out.replies_total += s.replies_total;
        out.accepted += s.accepted;
        out.accepted_expecting_reply += s.accepted_expecting_reply;
        out.rejected += s.rejected;
        out.unknown_status += s.unknown_status;
        out.missing_results += s.missing_results;
        out.missing_replies += s.missing_replies;
        out.duplicate_results += s.duplicate_results;
        out.duplicate_replies += s.duplicate_replies;
        out.unmatched_results += s.unmatched_results;
        out.unmatched_replies += s.unmatched_replies;
        out.probe_seq_mismatches += s.probe_seq_mismatches;
        out.order_violations += s.order_violations;
        out.tick_mismatches += s.tick_mismatches;
        out.ticks_compared += s.ticks_compared;
        for (k, v) in &s.rejected_by_reason {
            *out.rejected_by_reason.entry(k.clone()).or_insert(0) += v;
        }
        for (k, v) in &s.unknown_message_types {
            *out.unknown_message_types.entry(k.clone()).or_insert(0) += v;
        }
        out.wire_errors.extend(s.wire_errors.iter().cloned());
        out.errors.extend(s.errors.iter().cloned());
    }
    out
}
