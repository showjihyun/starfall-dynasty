//! SC-40 / AC-8(a) — 판매가 섞인 재생도 결정적이다(`mining_replay.rs`의 채굴 전용
//! 재생과 다른 골든 — 이 파일은 `SELL_MINERAL`도 입력열에 넣는다).
//!
//! # 비교 대상 셋
//!
//! - `snapshots.jsonl` — 물리(이동·채굴·채굴 재생과 같은 관례).
//! - `events.jsonl` — 이 tick이 발행한 도메인 이벤트 전체(`MINERAL_MINED`·
//!   `MINERAL_SOLD` 포함) — `mining_replay.rs`와 같은 계약 직렬화 규율
//!   ([`EventLine`] — sim 내부 타입 대신 레지스트리 이름 + 계약 payload 만 쓴다).
//! - `results.jsonl` — 이 tick에 난 `COMMAND_RESULT`(세션 A·B 전부) 전체 — 수락·
//!   거부 사유까지 포함해 판정 결정성을 직접 잰다(거부 사유가 하나도 안 섞이면
//!   "거부 경로는 결정적인가"를 전혀 안 본 것이 된다, qa3 ⊘ — 2026-10-07 재지적:
//!   사유별(쿨다운·도킹 밖·속도·보유 부족) 전부와 두 번째 함선이 입력열에 있어야
//!   한다).
//!
//! # 골든 경로
//!
//! `tests/data/replay_trading/` — 채굴 재생(`tests/data/replay_mining/`)·이동 재생
//! (`tests/data/replay/`)과 또 다른 디렉터리(같은 관례, ADR-0010 §3.1). 처음 만들
//! 때만 `STARFALL_REPLAY_BLESS=1`로 생성한다.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // 테스트 전용 헬퍼·하네스

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use starfall_contracts::primitives::{
    ConstSchemaVersion, ControlAxisMilli, DataId, GameCalendar, GameTime, InputSeq, MassKg,
    QuaternionComponentMicro, ServerVersion, UuidV7,
};
use starfall_contracts::{
    MineResourceCommand, MineResourcePayload, SellMineralCommand, SellMineralPayload,
    SetShipControlCommand, SetShipControlPayload, ShipState,
};
use starfall_sim::world::{
    BoundaryConstants, Quat, ShipClassConstants, Vec3, facing_toward_origin,
};
use starfall_sim::{
    DepositConstants, DomainEventBody, IdSource, InboundCommand, ListingConstants, MarketConstants,
    MineralConstants, MiningRuleConstants, PendingEvent, PriceRule, ServerMessage, ShipClassData,
    Simulation, Submission, WorldConstants,
};

fn id(n: u64) -> UuidV7 {
    let text = format!(
        "01a0b1c2-0002-7{:03x}-8{:03x}-{:012x}",
        n & 0xfff,
        (n >> 12) & 0xfff,
        n
    );
    UuidV7::parse(&text).expect("정규 UUIDv7 이어야 한다")
}

fn actor_a() -> UuidV7 {
    id(10)
}
fn session_a() -> UuidV7 {
    id(11)
}

/// 둘째 함선(qa3 2026-10-07 지적) — 도킹 구역 밖·과속 거부를 전담한다. A 와는
/// 독립된 actor 라 쿨다운·보유량이 서로 섞이지 않는다.
fn actor_b() -> UuidV7 {
    id(20)
}
fn session_b() -> UuidV7 {
    id(21)
}

/// 재생 길이 — `recovery_interval_ticks`(60)의 배수를 **두 번**(60·120) 지나가게
/// 한다(qa3 ⊘ "회복 배수 통과 ≥ 1" — 여유를 둬서 둘로 확실히 만든다).
const TOTAL_TICKS: u64 = 130;

const SPAWN_POINT_M: [f64; 3] = [2500.0, 250.0, 0.0];
const MINERAL: &str = "replay-mineral";
const DEPOSIT: &str = "replay-deposit";
const MARKET: &str = "replay-market";

fn scout_class() -> ShipClassData {
    ShipClassData {
        movement: ShipClassConstants {
            max_speed_mps: 140.0,
            main_thrust_mps2: 35.0,
            reverse_thrust_mps2: 18.0,
            lateral_thrust_mps2: 18.0,
            brake_decel_mps2: 50.0,
            assist_linear_decel_mps2: 7.0,
            assist_lateral_decel_mps2: 22.0,
            turn_rate_max_deg_s: 75.0,
            turn_accel_deg_s2: 220.0,
            turn_gain_deg_s_per_sin_half: 290.0,
            turn_deadzone_sin_half: 0.0009,
            roll_rate_max_deg_s: 90.0,
            roll_accel_deg_s2: 300.0,
            auto_level_rate_deg_s: 40.0,
            auto_level_deadzone_sin: 0.002,
        },
        hull_radius_m: 12.0,
    }
}

fn world() -> WorldConstants {
    let mut listings = BTreeMap::new();
    listings.insert(
        DataId::parse(MINERAL).unwrap(),
        ListingConstants {
            price: PriceRule {
                base_price_cr_per_kg: 10,
                band_kg: 2000,
                price_floor_pct: 30,
                price_drop_pct_per_band: 10,
            },
            recovery_kg: 1000,
        },
    );
    let mut markets = BTreeMap::new();
    markets.insert(
        DataId::parse(MARKET).unwrap(),
        MarketConstants {
            position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
            radius_m: 10.0,
            dock_range_from_surface_m: 150.0,
            max_ship_speed_mps: 10.0,
            sell_cooldown_ticks: 20,
            // 60 — TOTAL_TICKS(130) 안에 회복 배수 tick을 두 번(60·120) 지난다.
            recovery_interval_ticks: 60,
            pricing_version: starfall_contracts::primitives::RuleVersion::parse("replay-market@1")
                .unwrap(),
            listings,
        },
    );

    WorldConstants {
        world_id: id(1),
        calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
            .unwrap(),
        server_version: ServerVersion::parse("0.1.0").unwrap(),
        star_system_id: DataId::parse("cradle-replay-trading").unwrap(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        },
        // 매장지·시장이 스폰 지점과 같은 좌표 — 사거리 판정을 입력열 설계에서
        // 없애고 판정 자체(쿨다운·소진·회복·가격)에 집중한다(`mining_replay.rs`와
        // 같은 이유).
        spawn_points_m: vec![SPAWN_POINT_M],
        spawn_clearance_m: 150.0,
        spawn_max_probe_attempts: 12,
        spawn_radial_offset_step_m: 100.0,
        spawn_world_seed: 7,
        linger_seconds: 30.0,
        reconnect_resume_window_seconds: 30.0,
        ship_class_id: DataId::parse("scout-s01").unwrap(),
        ship_class: scout_class(),
        snapshot_interval_ticks: 2,
        carry_forward_max_ticks: 10_000,
        rate_limit_per_tick_cap: 5,
        max_entities_per_snapshot: 64,
        minerals: BTreeMap::from([(
            DataId::parse(MINERAL).unwrap(),
            MineralConstants {
                yield_per_extraction_kg: 500,
                regen_kg: 5,
                regen_interval_ticks: 10,
            },
        )]),
        deposits: BTreeMap::from([(
            DataId::parse(DEPOSIT).unwrap(),
            DepositConstants {
                mineral_id: DataId::parse(MINERAL).unwrap(),
                position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
                radius_m: 10.0,
                initial_reserve_kg: 10_000,
            },
        )]),
        mining_rules: MiningRuleConstants {
            mining_range_from_surface_m: 150.0,
            max_ship_speed_mps: 10.0,
            cooldown_ticks: 10,
        },
        markets,
    }
}

struct SeqIds(u64);
impl IdSource for SeqIds {
    fn next_id(&mut self) -> UuidV7 {
        self.0 += 1;
        id(1_000 + self.0)
    }
}

/// `inputs.jsonl` 한 줄 — 이동·채굴·판매 셋 다 나른다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum InputLine {
    SetShipControl {
        tick: u64,
        session: UuidV7,
        payload: SetShipControlPayload,
    },
    MineResource {
        tick: u64,
        session: UuidV7,
        deposit_id: DataId,
    },
    SellMineral {
        tick: u64,
        session: UuidV7,
        market_id: DataId,
        mineral_id: DataId,
        quantity_kg: i32,
    },
}

impl InputLine {
    const fn tick(&self) -> u64 {
        match self {
            Self::SetShipControl { tick, .. }
            | Self::MineResource { tick, .. }
            | Self::SellMineral { tick, .. } => *tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitialWorld {
    tick: u64,
    star_system_id: DataId,
    ships: Vec<ShipState>,
}

/// `events.jsonl` 한 줄의 사건 하나 — `mining_replay.rs::EventLine`과 같은 규율
/// (계약 payload 의 `Serialize` 만 쓴다, sim 내부 타입 `Debug` 아님).
#[derive(Debug, Serialize)]
struct EventLine {
    tick: u64,
    sequence: u64,
    event_type: &'static str,
    payload: serde_json::Value,
}

fn event_line(event: &PendingEvent) -> EventLine {
    let payload = match &event.body {
        DomainEventBody::SessionOpened(payload) => {
            serde_json::to_value(payload).expect("SessionOpenedPayload 직렬화")
        }
        DomainEventBody::SessionClosed(payload) => {
            serde_json::to_value(payload).expect("SessionClosedPayload 직렬화")
        }
        DomainEventBody::ShipSpawned(payload) => {
            serde_json::to_value(payload).expect("ShipSpawnedPayload 직렬화")
        }
        DomainEventBody::ShipDespawned(payload) => {
            serde_json::to_value(payload).expect("ShipDespawnedPayload 직렬화")
        }
        DomainEventBody::MineralMined(payload) => {
            serde_json::to_value(payload).expect("MineralMinedPayload 직렬화")
        }
        DomainEventBody::MineralSold(payload) => {
            serde_json::to_value(payload).expect("MineralSoldPayload 직렬화")
        }
    };
    EventLine {
        tick: event.tick.get(),
        sequence: event.sequence.get(),
        event_type: event.body.event_type(),
        payload,
    }
}

/// `results.jsonl` 한 줄 — 세션 A·B 가 받은 `COMMAND_RESULT` 하나(수락·거부 사유
/// 포함, qa3 ⊘ "거부 사유별 ≥ 1" 을 검증 가능하게 한다). `session` 을 넣어 어느
/// 함선의 결과인지 재생 파일만 보고도 알 수 있게 한다.
#[derive(Debug, Serialize)]
struct ResultLine {
    tick: u64,
    session: UuidV7,
    command_id: UuidV7,
    status: starfall_contracts::CommandStatus,
    reason_code: Option<starfall_contracts::RejectReasonCode>,
}

fn milli(x: f64) -> ControlAxisMilli {
    #[allow(clippy::cast_possible_truncation)]
    ControlAxisMilli::new((x * 1000.0).round() as i32).expect("범위 안 milli 값이어야 한다")
}

fn micro(x: f64) -> QuaternionComponentMicro {
    #[allow(clippy::cast_possible_truncation)]
    QuaternionComponentMicro::new((x * 1_000_000.0).round() as i32)
        .expect("범위 안 micro 값이어야 한다")
}

fn control_payload(input_seq: u32, thrust_z: f64, aim: Quat) -> SetShipControlPayload {
    SetShipControlPayload {
        input_seq: InputSeq::new(input_seq).expect("1 이상"),
        thrust_x_milli: milli(0.0),
        thrust_y_milli: milli(0.0),
        thrust_z_milli: milli(thrust_z),
        roll_milli: milli(0.0),
        aim_x_micro: micro(aim.x),
        aim_y_micro: micro(aim.y),
        aim_z_micro: micro(aim.z),
        aim_w_micro: micro(aim.w),
        brake: false,
        flight_assist: true,
    }
}

/// 계획 — 함선 둘(A·B), 네 거부 사유 전부 + 수락 둘 + 구간 걸침(간접) + 회복
/// 배수 경계(qa3 2026-10-07 지적대로 넓혔다):
///
/// **A(경제 쪽)**: 채굴(tick 1, 500kg) → 판매(tick 2, 300kg, 수락, 쿨다운 20
/// tick 시작) → **COOLDOWN_ACTIVE**(tick 3, 쿨다운 중 재시도) → 짧은 추력(물리도
/// 같은 재생에 섞인다) → 쿨다운이 끝난 뒤(tick 22+) **INSUFFICIENT_QUANTITY**
/// (tick 25, 보유 200kg 인데 300kg 요청 — 쿨다운에 가려지지 않는 진짜 거부) →
/// 거부는 쿨다운을 다시 걸지 않으므로(I-79) 바로 다음 tick(26)에 남은 200kg
/// 전부(수락, 구간이 걸치는 두 번째 판매).
///
/// **B(기동 쪽)**: tick 2 부터 전진 추력 100% 지속(멈추는 명령 없음) → 짧은
/// 가속만으로 tick 9 에 **TARGET_OUT_OF_RANGE**(판정 순서 4가 5보다 먼저라
/// 도킹 구역을 벗어났다는 사실 하나로 거부가 결정된다 — 그 시점 속도가 시장
/// 상한을 넘었는지는 안 본다) → 그 뒤로도 추력이 안 바뀌어 더 가속하다가
/// tick 70 에는 **SHIP_TOO_FAST**(실측 — 설계 의도는 "가까운데 빠름"을 먼저,
/// "먼데 빠름"을 나중에 보려 했으나 실제 물리 적분 결과는 반대로 나왔다, qa3
/// 2026-10-08 지적. 판정 결과엔 영향 없다 — 두 사유 다 ≥1건씩 재생에 있다는
/// ⊘만 충족하면 되고, 어느 tick이 어느 사유인지는 이 재생의 요구사항이
/// 아니다).
///
/// 그 뒤로는 아무 명령도 없이 TOTAL_TICKS(130) 까지 흘려보내 회복 배수
/// tick(60·120)을 지난다.
#[allow(clippy::vec_init_then_push)]
fn maneuver_plan() -> Vec<InputLine> {
    let facing = facing_toward_origin(Vec3::new(
        SPAWN_POINT_M[0],
        SPAWN_POINT_M[1],
        SPAWN_POINT_M[2],
    ));
    let mut lines = Vec::new();

    // --- A: 경제 판정(쿨다운·보유량, 수락 둘) ---
    lines.push(InputLine::MineResource {
        tick: 1,
        session: session_a(),
        deposit_id: DataId::parse(DEPOSIT).unwrap(),
    });
    lines.push(InputLine::SellMineral {
        tick: 2,
        session: session_a(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 300,
    });
    // ⊘ — 쿨다운 중(tick 2 수락 → tick 22 까지) 재시도 → COOLDOWN_ACTIVE.
    lines.push(InputLine::SellMineral {
        tick: 3,
        session: session_a(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 50,
    });
    lines.push(InputLine::SetShipControl {
        tick: 5,
        session: session_a(),
        payload: control_payload(1, 0.05, facing),
    });
    lines.push(InputLine::SetShipControl {
        tick: 6,
        session: session_a(),
        payload: control_payload(2, 0.0, facing),
    });
    // ⊘ — 쿨다운(20 tick)이 끝난 뒤인 tick 25. 보유 200kg 인데 300kg 요청 →
    // INSUFFICIENT_QUANTITY(쿨다운에 가려지지 않는 진짜 거부).
    lines.push(InputLine::SellMineral {
        tick: 25,
        session: session_a(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 300,
    });
    // 거부는 쿨다운을 다시 걸지 않는다(I-79) — 바로 다음 tick 에 남은 200kg 전부.
    lines.push(InputLine::SellMineral {
        tick: 26,
        session: session_a(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 200,
    });

    // --- B: 기동 판정(도킹 구역·속도) ---
    // 전진 100% 추력 — 다음 SetShipControl 이 없으므로 이후 쭉 유지된다.
    lines.push(InputLine::SetShipControl {
        tick: 2,
        session: session_b(),
        payload: control_payload(1, 1.0, facing),
    });
    // ⊘(실측) — 짧은 가속만으로 이미 도킹 구역을 벗어난다 → TARGET_OUT_OF_RANGE
    // (보유량 0 이어도 도킹 구역 판정(4)이 보유량 판정(6)보다 먼저라 이 사유가
    // 그대로 난다).
    lines.push(InputLine::SellMineral {
        tick: 9,
        session: session_b(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 1,
    });
    // ⊘(실측) — 같은 추력으로 더 가속한 뒤 → SHIP_TOO_FAST(이 재생에선 두 사유
    // 모두 ≥1건이면 충분하다 — 설계 당시 "가까운데 빠름을 먼저, 먼데 빠름을
    // 나중"으로 의도했던 tick 배치와 실제 물리 적분 결과가 반대로 나왔다, qa3
    // 2026-10-08 지적. 판정 로직엔 영향 없음).
    lines.push(InputLine::SellMineral {
        tick: 70,
        session: session_b(),
        market_id: DataId::parse(MARKET).unwrap(),
        mineral_id: DataId::parse(MINERAL).unwrap(),
        quantity_kg: 1,
    });

    lines.sort_by_key(InputLine::tick);
    lines
}

fn write_inputs_jsonl(path: &Path, lines: &[InputLine]) {
    let mut out = String::new();
    for line in lines {
        out.push_str(&serde_json::to_string(line).expect("InputLine 직렬화"));
        out.push('\n');
    }
    fs::create_dir_all(path.parent().expect("부모 디렉터리")).expect("디렉터리 생성");
    fs::write(path, out).expect("inputs.jsonl 쓰기");
}

fn read_inputs_jsonl(path: &Path) -> Vec<InputLine> {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("inputs.jsonl 을 읽을 수 없다({}): {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|e| panic!("입력 줄 파싱 실패: {line}: {e}"))
        })
        .collect()
}

#[test]
#[ignore = "부모(sc40_...) 가 자식 프로세스로만 실행한다"]
fn trading_replay_worker() {
    let inputs_path = PathBuf::from(
        std::env::var("STARFALL_REPLAY_INPUTS").expect("STARFALL_REPLAY_INPUTS 필요"),
    );
    let out_dir = PathBuf::from(
        std::env::var("STARFALL_REPLAY_OUT_DIR").expect("STARFALL_REPLAY_OUT_DIR 필요"),
    );
    fs::create_dir_all(&out_dir).expect("출력 디렉터리 생성");

    let inputs = read_inputs_jsonl(&inputs_path);

    let mut sim = Simulation::new(world(), 0);
    let mut ids = SeqIds(0);
    let mut seq: u64 = 0;
    let mut next_seq = || {
        let s = seq;
        seq += 1;
        s
    };

    let mut snapshots_jsonl = String::new();
    let mut events_jsonl = String::new();
    let mut results_jsonl = String::new();
    let mut initial_ships: Option<Vec<ShipState>> = None;

    for tick in 0..TOTAL_TICKS {
        let mut submissions = Vec::new();

        if tick == 0 {
            submissions.push(Submission::OpenSession {
                seq: next_seq(),
                session_id: session_a(),
                actor_id: actor_a(),
            });
            submissions.push(Submission::OpenSession {
                seq: next_seq(),
                session_id: session_b(),
                actor_id: actor_b(),
            });
        }

        for (index, line) in inputs.iter().enumerate().filter(|(_, l)| l.tick() == tick) {
            let command_id = id(90_000 + index as u64);
            let command = match line {
                InputLine::SetShipControl {
                    session, payload, ..
                } => (
                    *session,
                    InboundCommand::SetShipControl(SetShipControlCommand {
                        command_id,
                        command_type:
                            starfall_contracts::commands::SetShipControlType::SetShipControl,
                        schema_version: ConstSchemaVersion,
                        client_sent_at: None,
                        payload: *payload,
                    }),
                ),
                InputLine::MineResource {
                    session,
                    deposit_id,
                    ..
                } => (
                    *session,
                    InboundCommand::MineResource(MineResourceCommand {
                        command_id,
                        command_type: starfall_contracts::commands::MineResourceType::MineResource,
                        schema_version: ConstSchemaVersion,
                        client_sent_at: None,
                        payload: MineResourcePayload {
                            deposit_id: deposit_id.clone(),
                        },
                    }),
                ),
                InputLine::SellMineral {
                    session,
                    market_id,
                    mineral_id,
                    quantity_kg,
                    ..
                } => (
                    *session,
                    InboundCommand::SellMineral(SellMineralCommand {
                        command_id,
                        command_type: starfall_contracts::commands::SellMineralType::SellMineral,
                        schema_version: ConstSchemaVersion,
                        client_sent_at: None,
                        payload: SellMineralPayload {
                            market_id: market_id.clone(),
                            mineral_id: mineral_id.clone(),
                            quantity_kg: MassKg::new(*quantity_kg).expect("범위 안 질량"),
                        },
                    }),
                ),
            };
            submissions.push(Submission::Command {
                seq: next_seq(),
                session_id: command.0,
                command: command.1,
            });
        }

        let outcome = sim.step(submissions, &mut ids);

        if !outcome.events.is_empty() {
            let lines: Vec<EventLine> = outcome.events.iter().map(event_line).collect();
            events_jsonl.push_str(&serde_json::to_string(&lines).expect("이벤트 직렬화"));
            events_jsonl.push('\n');
        }

        for outbound in &outcome.outbound {
            match &outbound.message {
                // 스냅샷(물리 비교 대상)은 세션 A 기준만 쓴다 — 이동 재생·채굴
                // 재생과 같은 관례.
                ServerMessage::WorldSnapshot(message) if outbound.session_id == session_a() => {
                    snapshots_jsonl.push_str(
                        &serde_json::to_string(&message.payload).expect("payload 직렬화"),
                    );
                    snapshots_jsonl.push('\n');
                    if tick == 0 {
                        initial_ships = Some(message.payload.ships.clone());
                    }
                }
                // COMMAND_RESULT 는 **세션 A·B 전부** 싣는다 — B 의 도킹 구역·
                // 속도 거부도 결정성 비교 대상이다(qa3 2026-10-07 지적).
                ServerMessage::CommandResult(message) => {
                    let line = ResultLine {
                        tick,
                        session: outbound.session_id,
                        command_id: message.payload.command_id,
                        status: message.payload.status,
                        reason_code: message.payload.reason_code,
                    };
                    results_jsonl
                        .push_str(&serde_json::to_string(&line).expect("ResultLine 직렬화"));
                    results_jsonl.push('\n');
                }
                _ => {}
            }
        }
    }

    let ships =
        initial_ships.expect("tick 0 은 snapshot_interval_ticks=2 의 배수라 스냅샷이 있어야 한다");
    let initial = InitialWorld {
        tick: 0,
        star_system_id: world().star_system_id,
        ships,
    };
    let initial_json = serde_json::to_string_pretty(&initial).expect("initial.json 직렬화") + "\n";

    fs::write(out_dir.join("initial.json"), &initial_json).expect("initial.json 쓰기");
    fs::write(out_dir.join("snapshots.jsonl"), &snapshots_jsonl).expect("snapshots.jsonl 쓰기");
    fs::write(out_dir.join("events.jsonl"), &events_jsonl).expect("events.jsonl 쓰기");
    fs::write(out_dir.join("results.jsonl"), &results_jsonl).expect("results.jsonl 쓰기");

    println!(
        "[trading_replay_worker] tick 0..{TOTAL_TICKS} 완료, snapshots {} 줄, events {} 줄, \
         results {} 줄",
        snapshots_jsonl.lines().count(),
        events_jsonl.lines().count(),
        results_jsonl.lines().count()
    );
}

fn replay_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/replay_trading")
}

const REPLAY_BLESS_ENV: &str = "STARFALL_REPLAY_BLESS";

fn compare_or_bless_golden(fixture_dir: &Path, name: &str, generated: &[u8]) {
    let golden_path = fixture_dir.join(name);

    if std::env::var(REPLAY_BLESS_ENV).as_deref() == Ok("1") {
        fs::write(&golden_path, generated)
            .unwrap_or_else(|e| panic!("{} bless 쓰기 실패: {e}", golden_path.display()));
        println!(
            "[bless] {} 를 방금 만든 산출물로 덮어썼다 ({} 바이트)",
            golden_path.display(),
            generated.len()
        );
        return;
    }

    let golden = fs::read(&golden_path).unwrap_or_else(|e| {
        panic!(
            "골든 파일을 읽을 수 없다: {}({e}) — 처음 만드는 경우라면 \
             `{REPLAY_BLESS_ENV}=1 cargo test -p starfall-sim --test trading_replay \
             sc40_two_process_replay_with_trading_is_byte_identical` 로 한 번 만든다",
            golden_path.display()
        )
    });

    if golden != generated {
        let first_diff = golden
            .iter()
            .zip(generated.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| golden.len().min(generated.len()));
        panic!(
            "{} 이 추적된 골든과 바이트 단위로 다르다(골든 {} 바이트, 방금 재생 {} 바이트, \
             첫 차이 오프셋 {first_diff}). 판매 판정 코드 변경이 결과를 바꿨을 수 있다는 \
             신호다 — 조용히 bless 하지 말고 원인을 먼저 확인하라.",
            golden_path.display(),
            golden.len(),
            generated.len()
        );
    }
    println!(
        "[golden] {} 이 추적된 골든과 바이트 단위로 일치한다 ({} 바이트)",
        golden_path.display(),
        generated.len()
    );
}

/// 자식(`trading_replay_worker`)을 별도 프로세스로 띄워 끝까지 기다린다. 성공 경로에서도 자식이 실제로
/// 돌았다는 흔적을 남긴다: 자식 stdout 의 `test result: ok. 1 passed` 를 단언하고(이름이
/// 틀리면 libtest 가 0 개를 돌려 여기서 깨진다), 회차·자식 pid·부모 pid 와 그 결과 줄을
/// 찍는다. 기대 횟수는 여기서 요구하지 않는다 — 그건 계약·게이트의 일이다.
fn spawn_replay_child(run: u32, inputs_path: &Path, out_dir: &Path) -> std::process::Output {
    let exe = std::env::current_exe().expect("현재 테스트 바이너리 경로");
    let child = Command::new(exe)
        .args([
            "trading_replay_worker",
            "--exact",
            "--ignored",
            "--test-threads=1",
        ])
        .env("STARFALL_REPLAY_INPUTS", inputs_path)
        .env("STARFALL_REPLAY_OUT_DIR", out_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("자식 프로세스 실행 실패");
    let child_pid = child.id();
    let output = child.wait_with_output().expect("자식 프로세스 대기 실패");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "재생 워커({run}회차) 실패:
stdout: {stdout}
stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("test result: ok. 1 passed"),
        "재생 워커({run}회차)가 테스트를 1건 돌렸다는 흔적이 없다(자식 이름 오타·필터 불일치?):
stdout: {stdout}"
    );
    let result_line = stdout
        .lines()
        .find(|line| line.starts_with("test result:"))
        .unwrap_or_default();
    assert_ne!(
        child_pid,
        std::process::id(),
        "자식 pid 가 부모와 같다 — 별도 프로세스가 아니다"
    );
    println!(
        "[CHILD] worker=trading_replay_worker run={run} child_pid={child_pid} parent_pid={} :: {result_line}",
        std::process::id()
    );
    output
}

/// SC-40 / AC-8(a) — 판매 명령이 섞인 입력열을 서로 다른 프로세스에서 2회 돌려
/// `snapshots.jsonl`·`events.jsonl`(`MINERAL_SOLD` 포함)·`results.jsonl`(거부
/// 사유 포함) 셋 다 바이트 단위로 같음을 확인한다.
#[test]
fn sc40_two_process_replay_with_trading_is_byte_identical() {
    let fixture_dir = replay_fixture_dir();

    let scratch = std::env::temp_dir().join(format!("starfall-sc40-{}", std::process::id()));
    let inputs_path = scratch.join("inputs.jsonl");
    write_inputs_jsonl(&inputs_path, &maneuver_plan());

    let out1 = scratch.join("run1");
    let out2 = scratch.join("run2");

    spawn_replay_child(1, &inputs_path, &out1);
    spawn_replay_child(2, &inputs_path, &out2);

    let snapshots1 = fs::read(out1.join("snapshots.jsonl")).expect("run1 snapshots.jsonl");
    let snapshots2 = fs::read(out2.join("snapshots.jsonl")).expect("run2 snapshots.jsonl");
    let events1 = fs::read(out1.join("events.jsonl")).expect("run1 events.jsonl");
    let events2 = fs::read(out2.join("events.jsonl")).expect("run2 events.jsonl");
    let results1 = fs::read(out1.join("results.jsonl")).expect("run1 results.jsonl");
    let results2 = fs::read(out2.join("results.jsonl")).expect("run2 results.jsonl");
    let initial1 = fs::read(out1.join("initial.json")).expect("run1 initial.json");
    let initial2 = fs::read(out2.join("initial.json")).expect("run2 initial.json");

    assert_eq!(
        snapshots1, snapshots2,
        "서로 다른 프로세스 2회 실행의 snapshots.jsonl 이 바이트 단위로 달랐다 — 결정성 위반(I-37)"
    );
    assert_eq!(
        events1, events2,
        "서로 다른 프로세스 2회 실행의 events.jsonl 이 바이트 단위로 달랐다 — MINERAL_SOLD 를 \
         포함한 거래 판정이 결정적이지 않다는 신호다"
    );
    assert_eq!(
        results1, results2,
        "서로 다른 프로세스 2회 실행의 results.jsonl 이 바이트 단위로 달랐다 — 거부 사유 판정이 \
         결정적이지 않다는 신호다"
    );
    assert_eq!(initial1, initial2, "tick 0 조립이 두 프로세스에서 달랐다");

    // qa3 ⊘ — 세 조건을 같은 실행에서 전부 확인한다: MINERAL_SOLD ≥ 1, 거부
    // 사유별 ≥ 1, 회복 배수 통과 ≥ 1(자명 통과를 배제한다).
    let events_text = String::from_utf8_lossy(&events1);
    let sold_count = events_text
        .lines()
        .flat_map(|line| line.matches("\"MINERAL_SOLD\""))
        .count();
    assert!(
        sold_count >= 1,
        "⊘: MINERAL_SOLD 가 ≥1건 나야 한다 — 냈다: {sold_count}건"
    );

    let results_text = String::from_utf8_lossy(&results1);
    let reject_count = |label: &str| -> usize {
        results_text
            .lines()
            .filter(|line| line.contains(&format!("\"{label}\"")))
            .count()
    };
    // qa3 2026-10-07 지적 — 계약 표가 요구하는 네 사유(쿨다운·도킹 밖·속도·보유
    // 부족) 전부를 같은 실행에서 ≥1건씩 확인한다.
    let insufficient_quantity_count = reject_count("INSUFFICIENT_QUANTITY");
    let cooldown_active_count = reject_count("COOLDOWN_ACTIVE");
    let target_out_of_range_count = reject_count("TARGET_OUT_OF_RANGE");
    let ship_too_fast_count = reject_count("SHIP_TOO_FAST");
    assert!(
        insufficient_quantity_count >= 1,
        "⊘: 거부 사유(INSUFFICIENT_QUANTITY)가 ≥1건 재생에 있어야 한다 — 냈다: \
         {insufficient_quantity_count}건"
    );
    assert!(
        cooldown_active_count >= 1,
        "⊘: 거부 사유(COOLDOWN_ACTIVE)가 ≥1건 재생에 있어야 한다 — 냈다: \
         {cooldown_active_count}건"
    );
    assert!(
        target_out_of_range_count >= 1,
        "⊘: 거부 사유(TARGET_OUT_OF_RANGE)가 ≥1건 재생에 있어야 한다 — 냈다: \
         {target_out_of_range_count}건"
    );
    assert!(
        ship_too_fast_count >= 1,
        "⊘: 거부 사유(SHIP_TOO_FAST)가 ≥1건 재생에 있어야 한다 — 냈다: \
         {ship_too_fast_count}건"
    );

    // 재생 길이 자체가 회복 배수를 ≥2 번 지나는 설계인지 — `TOTAL_TICKS`가 상수라
    // clippy는 "항상 참"이라고 보지만, 이 상수를 사람이 줄이면 바로 깨지게 해 둔다
    // (⊘ 자체를 테스트 설계로 고정).
    #[allow(clippy::assertions_on_constants)]
    const _: () = assert!(
        TOTAL_TICKS >= 2 * 60,
        "⊘: 회복 배수(60 tick) 를 ≥2 번 지나야 한다"
    );
    let market_state_recovery_broadcast_ticks = TOTAL_TICKS / 60;
    println!(
        "[SC-40] MINERAL_SOLD {sold_count}건, 거부 사유: INSUFFICIENT_QUANTITY \
         {insufficient_quantity_count}·COOLDOWN_ACTIVE {cooldown_active_count}·\
         TARGET_OUT_OF_RANGE {target_out_of_range_count}·SHIP_TOO_FAST \
         {ship_too_fast_count}, 회복 배수 통과 {market_state_recovery_broadcast_ticks}회, \
         snapshots {} 줄, events {} 줄, results {} 줄 (두 프로세스 동일)",
        String::from_utf8_lossy(&snapshots1).lines().count(),
        events_text.lines().count(),
        results_text.lines().count()
    );

    compare_or_bless_golden(
        &fixture_dir,
        "inputs.jsonl",
        &fs::read(&inputs_path).expect("스크래치 inputs.jsonl 재읽기"),
    );
    compare_or_bless_golden(&fixture_dir, "initial.json", &initial1);
    compare_or_bless_golden(&fixture_dir, "snapshots.jsonl", &snapshots1);
    compare_or_bless_golden(&fixture_dir, "events.jsonl", &events1);
    compare_or_bless_golden(&fixture_dir, "results.jsonl", &results1);

    let _ = fs::remove_dir_all(&scratch);
}
