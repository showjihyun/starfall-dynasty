//! SC-33 / AC-7 — 채굴이 섞인 재생도 결정적이다(별도 golden, 이동 재생과 같은 파일이
//! 아니다).
//!
//! `determinism.rs`(AC-8/SC-34, "이동 golden")는 **채굴 명령을 전혀 쓰지 않는다** —
//! 그 파일 자체가 "채굴을 안 쓰면 한 바이트도 안 바뀐다"의 증거였다(S3b 절,
//! `03_server_impl.md`). 이 파일은 그 반대쪽을 재는 별도 재생이다: `MINE_RESOURCE`가
//! 섞인 입력열도 두 프로세스에서 바이트 단위로 같은 결과를 내는가 — sim이 시계를
//! 읽지 않고(I-58) 회복·소진·상한 판정이 전부 정수·결정적 비교이므로 그래야 한다.
//!
//! # 비교 대상 둘
//!
//! - `snapshots.jsonl` — 이동 재생과 같은 관례(`WORLD_SNAPSHOT.payload`, 세션 A 기준).
//! - `events.jsonl` — 이 tick이 발행한 **도메인 이벤트 전체**(`MINERAL_MINED` 포함,
//!   `sequence` 순으로 이미 정렬돼 있다)를 한 줄에 직렬화. 이동 재생에는 이 파일이
//!   없다 — 물리만 재는 파일이었기 때문이다. 채굴의 경제 결과(산출량·잔량 전후)가
//!   결정적인지는 스냅샷이 안 보여준다 — 그래서 따로 잰다. **계약 직렬화만 쓴다**
//!   ([`EventLine`]) — sim 내부 타입(`PendingEvent`/`DomainEventBody`)의 `Debug`
//!   표현을 처음엔 썼는데, 동작이 같아도 Rust 쪽 이름·필드 순서가 바뀌면 golden이
//!   깨지는 함정이었다(history golden에서 이미 겪은 것과 같은 종류 — team-lead
//!   지적, 2026-09-29 재정정). `event_type`(레지스트리 이름)과 payload 자체의
//!   `Serialize`(계약 wire 타입)만 쓰면 판정 로직이 실제로 안 바뀌는 한 이 파일도
//!   안 바뀐다.
//!
//! # 골든 경로 — 이동 golden과 **다른 디렉터리**
//!
//! `tests/data/replay_mining/`(이동은 `tests/data/replay/`) — team-lead 지시
//! (2026-09-29). 처음 만들 때 `STARFALL_REPLAY_BLESS=1`로 1회 생성했고, `events.jsonl`
//! 을 Debug에서 계약 직렬화로 바꾸며 **둘째 BLESS**를 했다(같은 날, team-lead 지시)
//! — 근거: 입력열(`inputs.jsonl`)·물리(`snapshots.jsonl`)는 그대로이고 `events.jsonl`
//! 의 **표현 형식만** 바뀌었다(내용은 같은 이벤트열 — 아래 게이트에 두 파일이 실제로
//! 불변이었음을 남긴다). 그 뒤로는 이동 golden과 같은 대조 규율 — 조용히 덮어쓰지
//! 않는다, ADR-0010 §3.1과 같은 정신).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // 테스트 전용 헬퍼·하네스

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use starfall_contracts::primitives::{
    ConstSchemaVersion, ControlAxisMilli, DataId, GameCalendar, GameTime, InputSeq,
    QuaternionComponentMicro, ServerVersion, UuidV7,
};
use starfall_contracts::{
    MineResourceCommand, MineResourcePayload, SetShipControlCommand, SetShipControlPayload,
    ShipState,
};
use starfall_sim::world::{
    BoundaryConstants, Quat, ShipClassConstants, Vec3, facing_toward_origin,
};
use starfall_sim::{
    DepositConstants, DomainEventBody, IdSource, InboundCommand, MineralConstants,
    MiningRuleConstants, PendingEvent, ServerMessage, ShipClassData, Simulation, Submission,
    WorldConstants,
};

fn id(n: u64) -> UuidV7 {
    let text = format!(
        "01a0b1c2-0001-7{:03x}-8{:03x}-{:012x}",
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

/// 재생 길이. 채굴 코드 경로(수락·소진·회복·쿨다운)를 겪기에 충분하고(쿨다운 60 tick
/// 이 두 번 지나가야 세 번째 채굴 시도를 볼 수 있다), 이동 재생(600 tick)만큼 길
/// 필요는 없다 — 이 재생의 목적은 물리 커버리지가 아니라 경제 결정성이다.
const TOTAL_TICKS: u64 = 200;

const SPAWN_POINT_M: [f64; 3] = [2500.0, 250.0, 0.0];

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
    WorldConstants {
        world_id: id(1),
        calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
            .unwrap(),
        server_version: ServerVersion::parse("0.1.0").unwrap(),
        star_system_id: DataId::parse("cradle-replay-mining").unwrap(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        },
        // 스폰 지점 하나 — 매장지와 정확히 같은 좌표(사거리 판정을 이 재생의 입력열
        // 설계에서 없애 채굴 판정 자체(쿨다운·소진·회복)에 집중한다. 사거리 판정은
        // `starfall-sim` 단위 테스트가 이미 촘촘히 덮는다, `simulation.rs::mining_tests`).
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
            DataId::parse("replay-mineral").unwrap(),
            MineralConstants {
                yield_per_extraction_kg: 25,
                regen_kg: 5,
                regen_interval_ticks: 10,
            },
        )]),
        deposits: BTreeMap::from([(
            DataId::parse("replay-deposit").unwrap(),
            DepositConstants {
                mineral_id: DataId::parse("replay-mineral").unwrap(),
                position_m: Vec3::new(SPAWN_POINT_M[0], SPAWN_POINT_M[1], SPAWN_POINT_M[2]),
                radius_m: 10.0,
                // 30kg. **실측(server, 2026-09-29)**: 애초 의도는 부분 산출·소진까지
                // 겪게 하는 것이었으나, regen_kg=5·regen_interval_ticks=10 에 쿨다운
                // 60 tick 을 그대로 곱하면 매번 정확히 30kg(=6구간×5)까지 완전히
                // 회복된 채로 다음 채굴을 맞는다 — 그래서 아래 계획의 네 번 모두
                // **가득 찬 상태에서 25kg 정량 수락**만 겪는다(부분 산출·소진은
                // `simulation.rs::mining_tests::partial_yield_when_remaining_is_less_than_extraction_yield`
                // 가 이미 단위 테스트로 덮는다 — 이 재생의 목적은 "경제 판정이 물리와
                // 같은 tick 에 섞여도 둘 다 결정적인가"이지 판정 경로 커버리지가
                // 아니다). 값은 그대로 두고 주석만 정정한다 — golden 이 이미 이 실제
                // 결과로 블레스돼 있다.
                initial_reserve_kg: 30,
            },
        )]),
        mining_rules: MiningRuleConstants {
            mining_range_from_surface_m: 150.0,
            max_ship_speed_mps: 10.0,
            cooldown_ticks: 60,
        },
    }
}

struct SeqIds(u64);
impl IdSource for SeqIds {
    fn next_id(&mut self) -> UuidV7 {
        self.0 += 1;
        id(1_000 + self.0)
    }
}

/// `inputs.jsonl` 의 한 줄 — 이동(`SET_SHIP_CONTROL`)과 채굴(`MINE_RESOURCE`)을 모두
/// 나른다(이동 재생의 `InputLine`은 후자를 모른다 — 그 파일은 채굴을 안 쓰므로 필요
/// 없었다).
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
}

impl InputLine {
    const fn tick(&self) -> u64 {
        match self {
            Self::SetShipControl { tick, .. } | Self::MineResource { tick, .. } => *tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitialWorld {
    tick: u64,
    star_system_id: DataId,
    ships: Vec<ShipState>,
}

/// `events.jsonl` 한 줄의 사건 하나. **계약 직렬화만 쓴다** — `PendingEvent`·
/// `DomainEventBody`(sim 내부 타입)는 `Serialize`가 없어서 이전 판은 `Debug` 표현을
/// 썼는데, team-lead 지적대로 그건 history golden과 같은 함정이다: 동작이 같아도
/// Rust 쪽 `derive`나 enum variant 이름이 바뀌면(리팩터링, 필드 재배열 등) golden이
/// 깨진다 — "물리·경제 판정이 바뀌었다"는 신호와 "누가 이름을 바꿨다"는 신호를
/// 구분하지 못하게 된다. 여기서는 `event_type`(레지스트리 이름, 안정적 문자열)과
/// **계약 payload 타입 자신의 `Serialize`**(`MineralMinedPayload` 등, `Serialize`가
/// derive된 wire 타입)만 쓴다 — payload 구조가 실제로 안 바뀌면 이 줄도 안 바뀐다.
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
    };
    EventLine {
        tick: event.tick.get(),
        sequence: event.sequence.get(),
        event_type: event.body.event_type(),
        payload,
    }
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

/// 채굴 넷(쿨다운 60 tick 간격, tick 1·61·121·181) + 그 사이 짧은 추력 한 번(경제·
/// 물리가 같은 재생에 섞여도 서로 결정성을 깨지 않는다는 것을 함께 본다). **실측**:
/// 네 번 모두 쿨다운(60 tick)이 회복 구간(6×10 tick)과 정확히 맞아떨어져 가득 찬
/// 상태(30kg)에서 25kg 정량 수락된다(위 `deposits` 필드 문서의 정정 참고) — 부분
/// 산출·소진 경로는 이 재생의 목적이 아니다(`mining_tests`의 단위 테스트가 그 경로를
/// 이미 덮는다).
#[allow(clippy::vec_init_then_push)]
fn maneuver_plan() -> Vec<InputLine> {
    let facing = facing_toward_origin(Vec3::new(
        SPAWN_POINT_M[0],
        SPAWN_POINT_M[1],
        SPAWN_POINT_M[2],
    ));
    let mut lines = Vec::new();

    lines.push(InputLine::MineResource {
        tick: 1,
        session: session_a(),
        deposit_id: DataId::parse("replay-deposit").unwrap(),
    });
    // 짧은 추력 — 물리도 같은 tick 대역에서 함께 진행된다는 것을 보인다. 사거리를
    // 벗어나지 않을 만큼 아주 짧게(다음 채굴도 계속 수락돼야 한다).
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
    lines.push(InputLine::MineResource {
        tick: 61,
        session: session_a(),
        deposit_id: DataId::parse("replay-deposit").unwrap(),
    });
    lines.push(InputLine::MineResource {
        tick: 121,
        session: session_a(),
        deposit_id: DataId::parse("replay-deposit").unwrap(),
    });
    lines.push(InputLine::MineResource {
        tick: 181,
        session: session_a(),
        deposit_id: DataId::parse("replay-deposit").unwrap(),
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
#[ignore = "부모(sc33_...) 가 자식 프로세스로만 실행한다"]
fn mining_replay_worker() {
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
    let mut initial_ships: Option<Vec<ShipState>> = None;

    for tick in 0..TOTAL_TICKS {
        let mut submissions = Vec::new();

        if tick == 0 {
            submissions.push(Submission::OpenSession {
                seq: next_seq(),
                session_id: session_a(),
                actor_id: actor_a(),
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
            };
            submissions.push(Submission::Command {
                seq: next_seq(),
                session_id: command.0,
                command: command.1,
            });
        }

        let outcome = sim.step(submissions, &mut ids);

        // 도메인 이벤트 전체(이미 sequence 순) — MINERAL_MINED 포함. **계약
        // 직렬화**([`event_line`]) — 위 `EventLine` 문서 참고. 빈 tick 은 줄을 안
        // 남긴다(이동 재생의 snapshots.jsonl 과 같은 관례).
        if !outcome.events.is_empty() {
            let lines: Vec<EventLine> = outcome.events.iter().map(event_line).collect();
            events_jsonl.push_str(&serde_json::to_string(&lines).expect("이벤트 직렬화"));
            events_jsonl.push('\n');
        }

        for outbound in &outcome.outbound {
            if outbound.session_id != session_a() {
                continue;
            }
            if let ServerMessage::WorldSnapshot(message) = &outbound.message {
                snapshots_jsonl
                    .push_str(&serde_json::to_string(&message.payload).expect("payload 직렬화"));
                snapshots_jsonl.push('\n');
                if tick == 0 {
                    initial_ships = Some(message.payload.ships.clone());
                }
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

    println!(
        "[mining_replay_worker] tick 0..{TOTAL_TICKS} 완료, snapshots {} 줄, events {} 줄",
        snapshots_jsonl.lines().count(),
        events_jsonl.lines().count()
    );
}

fn replay_fixture_dir() -> PathBuf {
    // 이동 golden(`tests/data/replay/`)과 **다른 디렉터리**(team-lead 지시, 2026-09-29).
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/replay_mining")
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
             `{REPLAY_BLESS_ENV}=1 cargo test -p starfall-sim --test mining_replay \
             sc33_two_process_replay_with_mining_is_byte_identical` 로 한 번 만든다",
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
             첫 차이 오프셋 {first_diff}). 채굴 판정 코드 변경이 결과를 바꿨을 수 있다는 \
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

fn spawn_replay_child(inputs_path: &Path, out_dir: &Path) -> std::process::Output {
    let exe = std::env::current_exe().expect("현재 테스트 바이너리 경로");
    Command::new(exe)
        .args([
            "mining_replay_worker",
            "--exact",
            "--ignored",
            "--test-threads=1",
        ])
        .env("STARFALL_REPLAY_INPUTS", inputs_path)
        .env("STARFALL_REPLAY_OUT_DIR", out_dir)
        .output()
        .expect("자식 프로세스 실행 실패")
}

/// SC-33 / AC-7 — 채굴 명령이 섞인 입력열을 서로 다른 프로세스에서 2회 돌려
/// `snapshots.jsonl`(물리)과 `events.jsonl`(도메인 이벤트, `MINERAL_MINED` 포함) 둘 다
/// 바이트 단위로 같음을 확인한다.
#[test]
fn sc33_two_process_replay_with_mining_is_byte_identical() {
    let fixture_dir = replay_fixture_dir();

    let scratch = std::env::temp_dir().join(format!("starfall-sc33-{}", std::process::id()));
    let inputs_path = scratch.join("inputs.jsonl");
    write_inputs_jsonl(&inputs_path, &maneuver_plan());

    let out1 = scratch.join("run1");
    let out2 = scratch.join("run2");

    let child1 = spawn_replay_child(&inputs_path, &out1);
    assert!(
        child1.status.success(),
        "재생 워커(1회차) 실패:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&child1.stdout),
        String::from_utf8_lossy(&child1.stderr)
    );
    let child2 = spawn_replay_child(&inputs_path, &out2);
    assert!(
        child2.status.success(),
        "재생 워커(2회차) 실패:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&child2.stdout),
        String::from_utf8_lossy(&child2.stderr)
    );

    let snapshots1 = fs::read(out1.join("snapshots.jsonl")).expect("run1 snapshots.jsonl");
    let snapshots2 = fs::read(out2.join("snapshots.jsonl")).expect("run2 snapshots.jsonl");
    let events1 = fs::read(out1.join("events.jsonl")).expect("run1 events.jsonl");
    let events2 = fs::read(out2.join("events.jsonl")).expect("run2 events.jsonl");
    let initial1 = fs::read(out1.join("initial.json")).expect("run1 initial.json");
    let initial2 = fs::read(out2.join("initial.json")).expect("run2 initial.json");

    assert_eq!(
        snapshots1, snapshots2,
        "서로 다른 프로세스 2회 실행의 snapshots.jsonl 이 바이트 단위로 달랐다 — 결정성 위반(I-37)"
    );
    assert_eq!(
        events1, events2,
        "서로 다른 프로세스 2회 실행의 events.jsonl 이 바이트 단위로 달랐다 — MINERAL_MINED \
         를 포함한 경제 판정이 결정적이지 않다는 신호다"
    );
    assert_eq!(initial1, initial2, "tick 0 조립이 두 프로세스에서 달랐다");

    // ⊘ — 실제로 채굴 이벤트가 났다는 것(자명하게 통과하는 "이벤트가 아예 없어서 같다"
    // 를 배제한다). 이제 JSON이라 `"event_type":"MINERAL_MINED"` 가 그대로 찍힌다
    // (계약 직렬화 — 레지스트리 이름 그대로, Rust variant 이름과 다르지 않다).
    let mined_count = String::from_utf8_lossy(&events1)
        .lines()
        .flat_map(|line| line.matches("\"MINERAL_MINED\""))
        .count();
    assert!(
        mined_count >= 2,
        "⊘: 이 재생이 실제로 MINERAL_MINED 를 ≥2건 내야 한다(수락 4건 실측 — 위 \
         `maneuver_plan` 문서 참고) — \
         냈다: {mined_count}건. events.jsonl: {}",
        String::from_utf8_lossy(&events1)
    );
    println!(
        "[SC-33] MINERAL_MINED {mined_count}건, snapshots {} 줄, events {} 줄 (두 프로세스 동일)",
        String::from_utf8_lossy(&snapshots1).lines().count(),
        String::from_utf8_lossy(&events1).lines().count()
    );

    compare_or_bless_golden(
        &fixture_dir,
        "inputs.jsonl",
        &fs::read(&inputs_path).expect("스크래치 inputs.jsonl 재읽기"),
    );
    compare_or_bless_golden(&fixture_dir, "initial.json", &initial1);
    compare_or_bless_golden(&fixture_dir, "snapshots.jsonl", &snapshots1);
    compare_or_bless_golden(&fixture_dir, "events.jsonl", &events1);

    let _ = fs::remove_dir_all(&scratch);
}
