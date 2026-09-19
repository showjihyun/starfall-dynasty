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

pub const STATUS_ACCEPTED: &str = "ACCEPTED";
pub const STATUS_REJECTED: &str = "REJECTED";

#[derive(Debug, Clone)]
struct CommandRecord {
    probe_seq: u32,
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
        }
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

    pub fn on_sent(&mut self, command_id: Uuid, probe_seq: u32, at_us: u64) {
        self.sent_total += 1;
        self.commands
            .entry(command_id)
            .and_modify(|r| r.sent_count += 1)
            .or_insert(CommandRecord {
                probe_seq,
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
            STATUS_ACCEPTED => self.accepted += 1,
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
            if r.status.as_deref() == Some(STATUS_ACCEPTED) && r.reply_count == 0 {
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

    pub fn accepted_reply_pairing_holds(&self) -> bool {
        self.accepted == self.replies_total
            && self.missing_replies == 0
            && self.duplicate_replies == 0
            && self.unmatched_replies == 0
            && self.probe_seq_mismatches == 0
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
