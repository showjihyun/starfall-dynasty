//! `WORLD_SNAPSHOT` 관측 계측 (p1-01). **IO 가 없고 시계를 읽지 않는다** — `ledger` 와 같은 규칙이다.
//!
//! 여기서 재는 것은 스프린트 계약의 다음 항목들이고, 전부 **봇이 받은 것만**으로 판정된다:
//!
//! | 항목 | 이 모듈이 세는 것 |
//! |------|-----------------|
//! | SC-28 | 연속 스냅샷의 tick 간격. **각 세션의 첫 스냅샷은 제외**하고 제외 수를 남긴다 |
//! | SC-29 | `ships` 의 `ship_id` 오름차순 위반 (정규 소문자 문자열 사전순 = 서버 `BTreeMap` 바이트순) |
//! | SC-30 | `controlled_ship_id` 가 `ships` 안에 있는가 |
//! | SC-31 | `ack_input_seq` 의 **세션 내** 단조 비감소 |
//! | SC-32 | `presence` 전이와 디스폰 후 소멸 |
//! | SC-71 | 수신 바이트 (서버 메트릭과 **독립 출처**) |
//! | M-7·M-8 | designer 지표: 경로 길이/변위, 최근접 거리, 속도 분포 |

use std::collections::BTreeMap;

use serde::Serialize;
use uuid::Uuid;

use crate::wire::{ShipState, WorldSnapshotMessage};

/// CSV 한 줄. **client 와 표기까지 같다**(계약 §3.1, `02_client_ack.md` §⑨).
///
/// `tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg`
///
/// **R23**: 뒤의 두 열은 client 가 R22 에서 도입한 재조정 평활화(F-33)의 렌더 오프셋이다.
/// 같은 헤더의 생산자가 **셋**이다 — 이 파일, `ObserverCsv.cs`(client),
/// `tests/e2e/two_client_view.py`(qa). `ObserverCsv.cs:49-51` 이 이름·순서·개수 일치를
/// 계약으로 걸고 불일치를 `NotImplementedYet` 으로 거부하므로, 둘만 고치면
/// **봇을 B 로 쓰는 경로(계약 §0.11·SC-64)가 조용히 죽는다.**
///
/// 봇은 예측도 평활화도 하지 않으므로 두 값은 **언제나 0 이고, 그것이 옳다** —
/// "봇이라서 0" 이지 "평활화가 죽어서 0" 이 아니다. 두 경우를 값으로는 구분할 수 없으니
/// 그 구분은 이 주석과 `two_client_view.py` 의 리포트가 진다.
pub const SNAPSHOT_CSV_HEADER: &str = "tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg";

#[derive(Debug, Clone, Serialize)]
pub struct ShipSample {
    pub tick: u64,
    pub ship_id: Uuid,
    pub position_mm: (i64, i64, i64),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SnapshotSummary {
    pub observer_actor_id: Option<String>,
    pub controlled_ship_id: Option<String>,
    /// 받은 스냅샷 수. `snapshots_sent_total` 델타와 대조한다(SC-70).
    pub snapshots_received: u64,
    /// 수신 바이트 합. **WebSocket 페이로드 길이**다(TCP 아님 — SC-71 주의).
    pub snapshot_bytes_received: u64,
    /// SC-28: 검사한 간격 수와 위반 수, 그리고 **제외한 첫 스냅샷 수**.
    pub intervals_checked: u64,
    pub interval_violations: u64,
    pub first_snapshots_excluded: u64,
    pub snapshot_interval_ticks: Option<u32>,
    /// SC-29
    pub ship_order_violations: u64,
    /// SC-30
    pub controlled_ship_missing: u64,
    /// SC-31 (세션 내 단조)
    pub ack_regressions: u64,
    pub ack_last: Option<u64>,
    /// SC-32
    pub presence_values: BTreeMap<String, u64>,
    pub lingering_seen: bool,
    /// `ships` 가 빈 스냅샷 수 — CSV 에 행이 안 생기므로 따로 센다(client ack §⑨).
    pub empty_ship_ticks: u64,
    pub max_ships_seen: usize,
    /// designer 지표 (M-7·M-8). 전부 **봇이 독립 계산**한다.
    pub own_path_length_m: f64,
    pub own_displacement_m: f64,
    pub own_speed_max_mps: f64,
    pub own_max_radius_m: f64,
    pub nearest_ship_min_m: Option<f64>,
    pub first_contact_2000m_tick: Option<u64>,
    /// 파싱 실패 등 이 관측자가 본 이상.
    pub errors: Vec<String>,
}

/// 자기 함선의 스냅샷 1건 — SC-24 (c)(d) 처럼 **tick 단위 차분**이 필요한 probe 가 쓴다
/// (`range_turn` 모듈). 판정은 거기서 하고, 여기서는 받은 값을 그대로 남기기만 한다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OwnSample {
    pub tick: u64,
    pub position_mm: (i64, i64, i64),
    pub velocity_mm_s: (i32, i32, i32),
    /// `(x, y, z, w)` 마이크로 단위.
    pub orientation_micro: (i32, i32, i32, i32),
    pub angular_velocity_mdeg_s: (i32, i32, i32),
    pub angular_velocity_roll_mdeg_s: i32,
    pub ack_input_seq: Option<u64>,
}

#[derive(Debug, Default)]
pub struct SnapshotLedger {
    pub observer_actor_id: Option<Uuid>,
    controlled: Option<Uuid>,
    interval: Option<u32>,
    prev_tick: Option<u64>,
    prev_ack: Option<u64>,
    first_done: bool,
    sum: SnapshotSummary,
    own_prev: Option<(i64, i64, i64)>,
    own_first: Option<(i64, i64, i64)>,
    rows: Vec<String>,
    /// 마지막으로 본 각 함선의 표본 (2-클라이언트 대조·이동 거리에 쓴다).
    pub last_seen: BTreeMap<Uuid, ShipState>,
    own_series: Vec<OwnSample>,
}

impl SnapshotLedger {
    pub fn new(observer_actor_id: Option<Uuid>) -> Self {
        Self {
            observer_actor_id,
            sum: SnapshotSummary {
                observer_actor_id: observer_actor_id.map(|v| v.to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn set_observer(&mut self, actor: Uuid) {
        self.observer_actor_id = Some(actor);
        self.sum.observer_actor_id = Some(actor.to_string());
    }

    /// 스냅샷 1건. `bytes` 는 **수신 프레임의 페이로드 길이**다.
    pub fn on_snapshot(&mut self, m: &WorldSnapshotMessage, bytes: usize) {
        self.sum.snapshots_received += 1;
        self.sum.snapshot_bytes_received += bytes as u64;
        self.sum.snapshot_interval_ticks = Some(m.payload.snapshot_interval_ticks);
        self.interval = Some(m.payload.snapshot_interval_ticks);
        self.controlled = m.payload.controlled_ship_id;
        if let Some(c) = m.payload.controlled_ship_id {
            self.sum.controlled_ship_id = Some(c.to_string());
        }

        // ── SC-28: 첫 스냅샷은 제외한다(전역 tick 기준 발사라 첫 간격이 0~1 tick이다)
        match (self.first_done, self.prev_tick) {
            (false, _) => {
                self.first_done = true;
                self.sum.first_snapshots_excluded += 1;
            }
            (true, Some(prev)) => {
                self.sum.intervals_checked += 1;
                let want = u64::from(m.payload.snapshot_interval_ticks);
                if m.tick.saturating_sub(prev) != want {
                    self.sum.interval_violations += 1;
                }
            }
            (true, None) => {}
        }
        self.prev_tick = Some(m.tick);

        // ── SC-31: 세션 내 단조 비감소. null → 값은 후퇴가 아니다.
        if let Some(ack) = m.payload.ack_input_seq {
            if let Some(prev) = self.prev_ack
                && ack < prev
            {
                self.sum.ack_regressions += 1;
            }
            self.prev_ack = Some(ack);
            self.sum.ack_last = Some(ack);
        }

        // ── SC-29: ship_id 오름차순 (문자열 사전순)
        let mut prev_id: Option<String> = None;
        for ship in &m.payload.ships {
            let id = ship.ship_id.to_string();
            if let Some(p) = &prev_id
                && &id < p
            {
                self.sum.ship_order_violations += 1;
            }
            prev_id = Some(id);
        }

        // ── SC-30: controlled_ship_id 가 배열 안에 있는가.
        // **null 은 위반이 아니다** — 함선이 아직 없는 세션의 정당한 상태다(계약 fixture
        // `empty-nulls-and-bounds.json`). 값이 있는데 배열에 없을 때만 센다.
        let controlled = m.payload.controlled_ship_id;
        if let Some(c) = controlled
            && !m.payload.ships.iter().any(|s| s.ship_id == c)
        {
            self.sum.controlled_ship_missing += 1;
        }

        if m.payload.ships.is_empty() {
            self.sum.empty_ship_ticks += 1;
        }
        self.sum.max_ships_seen = self.sum.max_ships_seen.max(m.payload.ships.len());

        // ── SC-32 · CSV · designer 지표
        let mut nearest_sq: Option<f64> = None;
        let own = controlled.and_then(|c| m.payload.ships.iter().find(|s| s.ship_id == c).cloned());
        for ship in &m.payload.ships {
            *self
                .sum
                .presence_values
                .entry(ship.presence.clone())
                .or_insert(0) += 1;
            if ship.is_lingering() {
                self.sum.lingering_seen = true;
            }
            self.rows.push(format!(
                // 끝의 `,0,0` 이 render_offset_mm / render_offset_deg 다 — 봇은 평활화가
                // 없으므로 상수다(위 SNAPSHOT_CSV_HEADER 주석 참조).
                "{tick},{obs},{sid},{pres},{px},{py},{pz},{vx},{vy},{vz},0,0",
                tick = m.tick,
                obs = self
                    .observer_actor_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                sid = ship.ship_id,
                pres = ship.presence,
                px = ship.position_x_mm,
                py = ship.position_y_mm,
                pz = ship.position_z_mm,
                vx = ship.velocity_x_mm_s,
                vy = ship.velocity_y_mm_s,
                vz = ship.velocity_z_mm_s,
            ));
            self.last_seen.insert(ship.ship_id, ship.clone());

            if let Some(o) = &own
                && Some(ship.ship_id) != controlled
            {
                let d = o.distance_mm(ship);
                nearest_sq = Some(nearest_sq.map_or(d, |c: f64| c.min(d)));
            }
        }

        if let Some(d_mm) = nearest_sq {
            let d_m = d_mm / 1000.0;
            self.sum.nearest_ship_min_m =
                Some(self.sum.nearest_ship_min_m.map_or(d_m, |c: f64| c.min(d_m)));
            if d_m <= 2000.0 && self.sum.first_contact_2000m_tick.is_none() {
                self.sum.first_contact_2000m_tick = Some(m.tick);
            }
        }

        if let Some(o) = own {
            self.own_series.push(OwnSample {
                tick: m.tick,
                position_mm: (o.position_x_mm, o.position_y_mm, o.position_z_mm),
                velocity_mm_s: (o.velocity_x_mm_s, o.velocity_y_mm_s, o.velocity_z_mm_s),
                orientation_micro: (
                    o.orientation_x_micro,
                    o.orientation_y_micro,
                    o.orientation_z_micro,
                    o.orientation_w_micro,
                ),
                angular_velocity_mdeg_s: (
                    o.angular_velocity_x_mdeg_s,
                    o.angular_velocity_y_mdeg_s,
                    o.angular_velocity_z_mdeg_s,
                ),
                angular_velocity_roll_mdeg_s: o.angular_velocity_roll_mdeg_s,
                ack_input_seq: m.payload.ack_input_seq,
            });
            let p = (o.position_x_mm, o.position_y_mm, o.position_z_mm);
            if self.own_first.is_none() {
                self.own_first = Some(p);
            }
            if let Some(prev) = self.own_prev {
                let dx = (p.0 - prev.0) as f64;
                let dy = (p.1 - prev.1) as f64;
                let dz = (p.2 - prev.2) as f64;
                self.sum.own_path_length_m += (dx * dx + dy * dy + dz * dz).sqrt() / 1000.0;
            }
            self.own_prev = Some(p);
            self.sum.own_speed_max_mps = self.sum.own_speed_max_mps.max(o.speed_mm_s() / 1000.0);
            self.sum.own_max_radius_m = self.sum.own_max_radius_m.max(o.radius_mm() / 1000.0);
        }
    }

    pub fn on_error(&mut self, msg: String) {
        if self.sum.errors.len() < 20 {
            self.sum.errors.push(msg);
        }
    }

    pub fn csv_rows(&self) -> &[String] {
        &self.rows
    }

    /// 자기 함선의 스냅샷 시계열(받은 순서).
    pub fn own_series(&self) -> &[OwnSample] {
        &self.own_series
    }

    /// 관측된 함선 하나의 이동 거리 — 두 클라이언트 대조(SC-61)에 쓴다.
    pub fn ship_positions(&self, ship_id: Uuid) -> Vec<ShipSample> {
        self.rows
            .iter()
            .filter_map(|row| {
                let mut it = row.split(',');
                let tick: u64 = it.next()?.parse().ok()?;
                let _obs = it.next()?;
                let sid: Uuid = it.next()?.parse().ok()?;
                if sid != ship_id {
                    return None;
                }
                let _pres = it.next()?;
                let x: i64 = it.next()?.parse().ok()?;
                let y: i64 = it.next()?.parse().ok()?;
                let z: i64 = it.next()?.parse().ok()?;
                Some(ShipSample {
                    tick,
                    ship_id: sid,
                    position_mm: (x, y, z),
                })
            })
            .collect()
    }

    pub fn finish(&self) -> SnapshotSummary {
        let mut out = self.sum.clone();
        if let (Some(a), Some(b)) = (self.own_first, self.own_prev) {
            let dx = (b.0 - a.0) as f64;
            let dy = (b.1 - a.1) as f64;
            let dz = (b.2 - a.2) as f64;
            out.own_displacement_m = (dx * dx + dy * dy + dz * dz).sqrt() / 1000.0;
        }
        out
    }

    /// SC-28~32 의 게이트. 판정은 리포트가 하지만, 봇도 스스로 빨간불을 켠다.
    pub fn gates_ok(&self) -> bool {
        self.sum.interval_violations == 0
            && self.sum.ship_order_violations == 0
            && self.sum.controlled_ship_missing == 0
            && self.sum.ack_regressions == 0
            && self.sum.errors.is_empty()
    }
}

/// 여러 관측자의 요약을 합친다(부하 전체 집계용).
pub fn aggregate(all: &[SnapshotSummary]) -> SnapshotSummary {
    let mut out = SnapshotSummary::default();
    for s in all {
        out.snapshots_received += s.snapshots_received;
        out.snapshot_bytes_received += s.snapshot_bytes_received;
        out.intervals_checked += s.intervals_checked;
        out.interval_violations += s.interval_violations;
        out.first_snapshots_excluded += s.first_snapshots_excluded;
        out.ship_order_violations += s.ship_order_violations;
        out.controlled_ship_missing += s.controlled_ship_missing;
        out.ack_regressions += s.ack_regressions;
        out.empty_ship_ticks += s.empty_ship_ticks;
        out.max_ships_seen = out.max_ships_seen.max(s.max_ships_seen);
        out.own_path_length_m += s.own_path_length_m;
        out.own_displacement_m += s.own_displacement_m;
        out.own_speed_max_mps = out.own_speed_max_mps.max(s.own_speed_max_mps);
        out.own_max_radius_m = out.own_max_radius_m.max(s.own_max_radius_m);
        out.lingering_seen |= s.lingering_seen;
        if let Some(n) = s.nearest_ship_min_m {
            out.nearest_ship_min_m = Some(out.nearest_ship_min_m.map_or(n, |c: f64| c.min(n)));
        }
        if s.snapshot_interval_ticks.is_some() {
            out.snapshot_interval_ticks = s.snapshot_interval_ticks;
        }
        for (k, v) in &s.presence_values {
            *out.presence_values.entry(k.clone()).or_insert(0) += v;
        }
        out.errors.extend(s.errors.iter().cloned());
    }
    out
}
