//! AC-8 / SC-34 — 결정성 재생.
//!
//! 고정된 초기 상태 + **파일에 기록된 입력열**(함선 3척, 600 tick, 추력·선회·브레이크·경계
//! 접촉·잔류를 포함)을 **서로 다른 프로세스에서 2회** 돌려 매 tick의 `WORLD_SNAPSHOT`
//! payload 를 직렬화해 저장하고, 두 산출물이 **바이트 단위로 동일**함을 단언한다(I-37).
//! 근사 비교는 쓰지 않는다 — 허용 오차는 이 테스트가 잡아야 할 차이를 정확히 덮는다.
//!
//! # 세 파일의 역할 (`01_architect_tasks.md` "S6 산출물 형식" 확정)
//!
//! - `initial.json` — tick 0 의 함선 3척 상태 + `star_system_id`. **C4(client)가 예측
//!   코어의 시작값으로 그대로 읽는다.**
//! - `inputs.jsonl` — `{"tick", "session", "payload"}` 한 줄 = 한 입력. **정적 파일이다.**
//!   [`maneuver_plan`] 이 만드는 것은 이 파일의 내용이고, 재생 워커는 그 내용을 **파일에서
//!   읽어** 시뮬레이션에 넣는다 — 워커가 스스로 입력을 발명하면 "C4 가 쓸 자산"이 되지
//!   못한다(태스크 문서 경고). 입력 없는 tick 은 줄이 없다 — 이월 경로를 타게 한다.
//! - `snapshots.jsonl` — 한 줄 = 그 tick 의 `WORLD_SNAPSHOT` **payload**(첫 함선의 세션
//!   기준 하나만). **이 파일이 두 프로세스 실행 사이에서 비교되는 대상이다.**
//!
//! 세션 닫힘(잔류 시작)은 `inputs.jsonl` 의 표현 범위 밖이다(그 형식은 `SET_SHIP_CONTROL`
//! payload 한 종류만 나른다 — C4 의 클라이언트 예측은 연결 개념을 모른다). 그래서
//! `CHARLIE_SESSION` 의 `CloseSession` 은 재생 워커 코드에 고정된 tick 으로 박아 둔다.
//!
//! # 골든 파일 — 추적되고, 대조되고, bless 로만 갱신된다 (ADR-0010 §3.1, architect 결정)
//!
//! `tests/data/replay/` 의 세 파일은 **git에 추적된다.** "같은 비트"의 전제는 두 언어가
//! *같은 절차를 도는* 것이 아니라 **같은 파일을 여는** 것이다 — 추적하지 않으면 기준이
//! "마지막으로 로컬에서 `cargo test` 를 돌린 사람의 파일"이 되고, C#(client) 쪽이
//! 재생 결과를 초과했을 때 "두 구현이 다르다"와 "두 사람이 다른 파일을 봤다"를 구분할
//! 수 없다.
//!
//! 추적하는 순간 "테스트만 돌려도 골든이 조용히 바뀐다"가 새 위험이 된다. 그래서 이
//! 테스트는 골든을 **무조건 덮어쓰지 않고 대조한다**([`compare_or_bless_golden`]):
//! `inputs.jsonl` 은 [`maneuver_plan`](정본)의 직렬화 결과와, `initial.json`·
//! `snapshots.jsonl` 은 run1 산출물과 각각 바이트 단위로 비교하고, 다르면 실패시키며
//! 재생성 방법을 출력한다. 의도적으로 물리·이월 규칙을 바꿔 golden 을 갱신해야 할
//! 때만 `STARFALL_REPLAY_BLESS=1` 환경 변수를 주고 이 테스트를 다시 돌린다 — 그때는
//! 대조 대신 덮어쓴다. **조용히 bless 하지 않는다**: golden 이 달라졌다는 것은 최근
//! 변경이 재생 결과(=물리)를 건드렸다는 신호이므로, 의도한 변경인지 먼저 확인한 뒤에만
//! bless 한다.
//!
//! # 자식 프로세스 재실행 패턴
//!
//! `current_exe()` 로 이 테스트 바이너리 자신을 다시 실행한다. 평범한 `#[test]` 로는
//! "진짜 다른 프로세스" 를 못 만들므로, [`replay_worker`] 를 `#[ignore]` 로 감춰 두고
//! 부모가 `--exact replay_worker --ignored` 로 정확히 그것만 골라 두 번 실행한다. 환경
//! 변수로 입력·출력 경로를 넘긴다 — 새 바이너리도 새 크레이트도 아니다(표준 라이브러리
//! `std::process::Command`/`std::env::current_exe` 뿐이다, `Cargo.toml` 참고).
//!
//! # 왜 `data/` 의 실제 스폰 지점을 쓰지 않는가
//!
//! 실제 스폰 링(반경 ~2,500 m)에서 hard 경계(12,000 m)까지는 600 tick(30 s)·최고
//! 속도(140 m/s) 로도 못 미친다(약 3,920 m 이동 — server 실측, `03_server_impl.md`
//! 기록). **G-g** 가 이미 이 테스트를 메모리(`Simulation::new(world, 0)`) 취급으로
//! 지정했으므로, 이 파일의 [`world`] 는 `data/` 를 읽지 않는 합성 상수다 — 스폰 지점을
//! 경계 바로 안쪽에 두어 600 tick 예산 안에서 경계 접촉을 실제로 겪게 한다.
//!
//! 함선 3척은 **단일 스폰 지점**(`[11_700, 0, 0]`)만 등록해 스폰 해시(`actor_id` 의존)를
//! 완전히 비켜 간다 — `point_count == 1` 이면 `index0 = hash % 1 = 0` 이 언제나 성립해
//! `choose_spawn_point` 의 "전부 점유" 오버플로 분기(ADR-0010 §4)로 3척을 예측 가능한
//! 간격으로 늘어세운다(11,700 / 11,900 / 12,000 m — 계산은 아래 상수 주석 참고). 이
//! 오버플로 경로 자체가 스폰 알고리즘의 한 분기이므로, 덤으로 그 분기도 이 재생이
//! 덮는다.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // 테스트 전용 헬퍼·하네스

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use starfall_contracts::events::SessionCloseReason;
use starfall_contracts::primitives::{
    ConstSchemaVersion, ControlAxisMilli, DataId, GameCalendar, GameTime, InputSeq,
    QuaternionComponentMicro, ServerVersion, UuidV7,
};
use starfall_contracts::{SetShipControlCommand, SetShipControlPayload, ShipState};
use starfall_sim::world::{
    BoundaryConstants, Quat, ShipClassConstants, Vec3, facing_toward_origin,
};
use starfall_sim::{
    IdSource, InboundCommand, ServerMessage, ShipClassData, Simulation, Submission, WorldConstants,
};

// -----------------------------------------------------------------------------------------
// 고정 식별자 — 두 자식 프로세스가 100% 같은 값을 쓴다(하드코딩, 파일에서 읽지 않는다).
// -----------------------------------------------------------------------------------------

/// 결정적 UUIDv7 (테스트 전용). `simulation.rs` 단위 테스트와 같은 패턴이지만, 이 파일은
/// 별도 통합 테스트 바이너리라 그 비공개 헬퍼를 가져올 수 없어 다시 정의한다.
fn id(n: u64) -> UuidV7 {
    let text = format!(
        "01a0b1c2-0000-7{:03x}-8{:03x}-{:012x}",
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
fn actor_b() -> UuidV7 {
    id(20)
}
fn session_b() -> UuidV7 {
    id(21)
}
fn actor_c() -> UuidV7 {
    id(30)
}
fn session_c() -> UuidV7 {
    id(31)
}

/// 세션 C(잔류 담당)를 닫는 tick. `inputs.jsonl` 은 `SET_SHIP_CONTROL` 만 나르므로 이
/// 세션-닫힘은 파일 밖, 워커에 고정으로 박는다(모듈 문서 참고).
const CHARLIE_CLOSE_TICK: u64 = 50;

/// 재생 길이 (AC-8: "함선 3척, 600 tick").
const TOTAL_TICKS: u64 = 600;

// -----------------------------------------------------------------------------------------
// 월드 — 합성 상수(모듈 문서 "왜 data/ 를 쓰지 않는가" 참고).
// -----------------------------------------------------------------------------------------

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

/// 스폰 지점 하나 — 경계 바로 안쪽(모듈 문서 참고). `radial_offset_step_m = 100.0` 과
/// 함께 세 함선을 11,700 / 11,900 / 12,000 m 에 정확히 늘어세운다(`choose_spawn_point`
/// 의 "전부 점유" 분기, `existing.len() + 1` 시도수 해석 — `spawn.rs` 판단 근거 그대로).
const SPAWN_POINT_M: [f64; 3] = [11_700.0, 0.0, 0.0];

fn world() -> WorldConstants {
    WorldConstants {
        world_id: id(1),
        calendar: GameCalendar::new(20, GameTime::parse("3800-01-01T00:00:00Z").unwrap(), 60)
            .unwrap(),
        server_version: ServerVersion::parse("0.1.0").unwrap(),
        star_system_id: DataId::parse("cradle-replay").unwrap(),
        boundary: BoundaryConstants {
            soft_boundary_radius_m: 10_000.0,
            hard_boundary_radius_m: 12_000.0,
            boundary_pull_mps2: 25.0,
        },
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
        carry_forward_max_ticks: 10_000, // 이 재생에서 이월이 만료되지 않게(휴면 입력은 세션 닫힘으로만 겪는다)
        rate_limit_per_tick_cap: 5,
        max_entities_per_snapshot: 64,
    }
}

/// 시뮬레이션 id 공급자(운영의 `Uuid::now_v7()` 대신). 두 자식이 `SeqIds(0)` 로 똑같이
/// 시작하므로 이벤트·메시지 id 열도 바이트 단위로 같다.
struct SeqIds(u64);
impl IdSource for SeqIds {
    fn next_id(&mut self) -> UuidV7 {
        self.0 += 1;
        id(1_000 + self.0)
    }
}

// -----------------------------------------------------------------------------------------
// inputs.jsonl 의 한 줄 — "S6 산출물 형식" 표 그대로.
// -----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InputLine {
    tick: u64,
    session: UuidV7,
    payload: SetShipControlPayload,
}

/// `initial.json` 의 모양 — "S6 산출물 형식" 표: tick 0 의 함선 3척 + `star_system_id` +
/// 시작 tick. 함선 필드는 `ShipState`(계약 타입, 와이어와 같은 표현) 그대로 재사용한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitialWorld {
    tick: u64,
    star_system_id: DataId,
    ships: Vec<ShipState>,
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

fn control_payload(input_seq: u32, thrust_z: f64, aim: Quat, brake: bool) -> SetShipControlPayload {
    control_payload_with_roll(input_seq, thrust_z, 0.0, aim, brake)
}

/// [`control_payload`] + 수동 롤(`-1.0..=1.0`). alpha 의 선회 구간(tick 450~530)에서
/// `ω_aim` 이 아직 0이 아닌 동안 롤을 섞어 **AC-12(e)가 요구하는 "ω_aim·ω_roll 동시에
/// 0이 아님" tick** 을 만든다(S8) — 롤 축은 자세 제어기와 권한이 분리돼 있어
/// (`integrate.rs` 2단계 "롤 축 권한 분리") 이 추가가 선회·추력 경로에 간섭하지 않는다.
fn control_payload_with_roll(
    input_seq: u32,
    thrust_z: f64,
    roll: f64,
    aim: Quat,
    brake: bool,
) -> SetShipControlPayload {
    SetShipControlPayload {
        input_seq: InputSeq::new(input_seq).expect("1 이상"),
        thrust_x_milli: milli(0.0),
        thrust_y_milli: milli(0.0),
        thrust_z_milli: milli(thrust_z),
        roll_milli: milli(roll),
        aim_x_micro: micro(aim.x),
        aim_y_micro: micro(aim.y),
        aim_z_micro: micro(aim.z),
        aim_w_micro: micro(aim.w),
        brake,
        flight_assist: true,
    }
}

/// 고정 입력열 — thrust(전/후진) · turn · brake · 경계 접촉 · 잔류를 600 tick 안에서
/// 모두 겪는다(AC-8 요구). 각 함선의 스폰 자세는 `facing_toward_origin` 을 **그대로
/// 호출해** 구한다 — 값을 손으로 베끼면 그 자체가 오탈자 위험이다.
// 아래는 각 `push` 가 그 tick·함선의 의도를 설명하는 주석과 짝지어져 있다 — `vec![]` 로
// 합치면 그 설명이 값과 떨어져 읽기 어려워진다(clippy 는 순수 리터럴 나열을 가정한다).
#[allow(clippy::vec_init_then_push)]
fn maneuver_plan() -> Vec<InputLine> {
    // 함선은 언제나 spawn 시 원점을 바라본다(`facing_toward_origin`) — 그래서 "전진"은
    // 언제나 원점 쪽이다. **경계를 실제로 뚫으려면 원점 반대를 바라봐야 한다** — 후진
    // (`reverse_thrust_mps2 = 18`)은 경계 당김(`boundary_pull_mps2 = 25`)보다 약해
    // 뚫지 못한다(server 실측, 이 파일 초판에서 실제로 겪은 판단 착오 — 아래 두 상수는
    // 그래서 "바깥을 본 채 전진"으로 경계를 넘는다: `main_thrust_mps2 = 35 > 25`).
    let facing_a = facing_toward_origin(Vec3::new(
        SPAWN_POINT_M[0],
        SPAWN_POINT_M[1],
        SPAWN_POINT_M[2],
    ));
    let facing_b = facing_toward_origin(Vec3::new(11_900.0, 0.0, 0.0));
    let facing_c = facing_toward_origin(Vec3::new(12_000.0, 0.0, 0.0));
    // 원점 반대(세계 +X) 를 보는 고정 쿼터니언. `f0=(0,0,1)` 을 `(1,0,0)` 으로 보내는
    // 최소 회전 — ADR-0010 §4 의 수식을 그대로 손으로 적용했다(대척점이 아니라 직교라
    // 그 분기는 필요 없다).
    let half_sqrt2 = 0.5_f64.sqrt();
    let away_from_origin = Quat::new(0.0, half_sqrt2, 0.0, half_sqrt2);

    let mut lines = Vec::new();

    // ---- alpha(A, 관찰자) — 경계 접촉(실제로 hard 경계를 넘겨 클램프를 겪는다) + 브레이크
    //      + 선회(180°, away → toward) + 추력 ----
    lines.push(InputLine {
        tick: 1,
        session: session_a(),
        // 바깥을 보고 전진 = main_thrust(35) - pull(25) = 순 +10 m/s² 바깥 — **일단 spawn
        // 자세(원점 쪽)에서 away 로 180° 선회를 먼저 마쳐야** 그 순추력이 붙는다(server
        // 실측: 선회 중엔 국소 +Z 가 아직 원점 쪽에 가까워 초반 tick은 오히려 안쪽으로
        // 민다). 그래서 이 구간을 380 tick(~19 s) 으로 넉넉히 잡는다 — 선회(약 50 tick)
        // + 나머지 가속 구간으로 hard 경계(12,000 m)를 실제로 넘겨 클램프(12단계)에
        // 여러 번 걸리는 것을 server 실측으로 확인했다.
        payload: control_payload(1, 1.0, away_from_origin, false),
    });
    lines.push(InputLine {
        tick: 380,
        session: session_a(),
        payload: control_payload(2, 0.0, away_from_origin, true), // 경계 밖/위에서 브레이크
    });
    lines.push(InputLine {
        tick: 450,
        session: session_a(),
        // 180° 선회 시작(away → toward, 쿼터니언 내적 0 — 최대 자세 오차). 추력 없이
        // 선회만 한다.
        payload: control_payload(3, 0.0, facing_a, false),
    });
    lines.push(InputLine {
        tick: 460,
        session: session_a(),
        // S8(AC-12(e)) — 선회가 아직 끝나지 않아 ω_aim ≠ 0 인 동안 수동 롤을 섞는다.
        // 각속도를 하나로 합쳐 보내고 받는 쪽에서 분해하는 손실 구현이라면 이 tick에서
        // `ω_aim`·`ω_roll` 을 각각 되돌리지 못해 AC-12(e)가 실패한다(ADR-0010 §1.1).
        payload: control_payload_with_roll(4, 0.0, 1.0, facing_a, false),
    });
    lines.push(InputLine {
        tick: 500,
        session: session_a(),
        // 롤을 다시 0으로 — 530의 전진 재개 전에 정리한다.
        payload: control_payload(5, 0.0, facing_a, false),
    });
    lines.push(InputLine {
        tick: 530,
        session: session_a(),
        // 선회를 마쳤을 새 방향(원점 쪽)으로 전진 — 경계를 벗어나 복귀를 시작한다(600
        // tick 예산 안에 완전히 복귀할 필요는 없다 — 이 재생의 목적은 코드 경로 커버리지다).
        payload: control_payload(6, 1.0, facing_a, false),
    });

    // ---- bravo(B) — 추력 전진(원점 쪽) + 브레이크 + 후진(추력 양방향 커버) ----
    lines.push(InputLine {
        tick: 1,
        session: session_b(),
        payload: control_payload(1, 1.0, facing_b, false),
    });
    lines.push(InputLine {
        tick: 150,
        session: session_b(),
        payload: control_payload(2, 0.0, facing_b, true),
    });
    lines.push(InputLine {
        tick: 170,
        session: session_b(),
        // 후진 — 원점을 본 채 후진이므로 다시 바깥쪽으로(반대 방향 추력 값 커버).
        payload: control_payload(3, -1.0, facing_b, false),
    });

    // ---- charlie(C) — 짧은 조작 뒤 세션이 닫혀(워커에 고정) 잔류로 들어간다 ----
    lines.push(InputLine {
        tick: 1,
        session: session_c(),
        payload: control_payload(1, 0.5, facing_c, false),
    });

    lines.sort_by_key(|line| (line.tick, line.session));
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

// -----------------------------------------------------------------------------------------
// 재생 워커 — 자식 프로세스가 `--exact replay_worker --ignored` 로 실행한다.
// -----------------------------------------------------------------------------------------

/// 이 테스트는 **직접 실행되지 않는다**(`#[ignore]`). [`spawn_replay_child`] 가
/// `current_exe()` 재실행으로 골라 부른다. 입출력 경로는 환경 변수로 받는다.
#[test]
#[ignore = "부모(ac8_...) 가 자식 프로세스로만 실행한다 — 직접 실행하려면 STARFALL_REPLAY_INPUTS/STARFALL_REPLAY_OUT_DIR 를 채워야 한다"]
fn replay_worker() {
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
            submissions.push(Submission::OpenSession {
                seq: next_seq(),
                session_id: session_c(),
                actor_id: actor_c(),
            });
        }

        if tick == CHARLIE_CLOSE_TICK {
            submissions.push(Submission::CloseSession {
                seq: next_seq(),
                session_id: session_c(),
                reason: SessionCloseReason::ClientClosed,
            });
        }

        for (index, line) in inputs
            .iter()
            .enumerate()
            .filter(|(_, line)| line.tick == tick)
        {
            submissions.push(Submission::Command {
                seq: next_seq(),
                session_id: line.session,
                command: InboundCommand::SetShipControl(SetShipControlCommand {
                    command_id: id(90_000 + index as u64),
                    command_type: starfall_contracts::commands::SetShipControlType::SetShipControl,
                    schema_version: ConstSchemaVersion,
                    client_sent_at: None,
                    payload: line.payload,
                }),
            });
        }

        let outcome = sim.step(submissions, &mut ids);

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

    println!(
        "[replay_worker] tick 0..{TOTAL_TICKS} 실행 완료, snapshots.jsonl {} 줄 / {} 바이트",
        snapshots_jsonl.lines().count(),
        snapshots_jsonl.len()
    );
}

// -----------------------------------------------------------------------------------------
// 부모 — 자식 2회 실행 + 바이트 비교 (SC-34).
// -----------------------------------------------------------------------------------------

fn replay_fixture_dir() -> PathBuf {
    // `tests/` 와 나란히 있는 `tests/data/replay/` — CARGO_MANIFEST_DIR 은 `crates/sim`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/replay")
}

/// 이 값이 설정돼 있으면(정확히 `"1"`) [`compare_or_bless_golden`] 이 대조 대신 추적된
/// 골든을 덮어쓴다. 기본은 대조다 — 추적하는 순간의 새 위험("테스트만 돌려도 골든이
/// 조용히 바뀐다")을 막기 위해서다(ADR-0010 §3.1, architect 결정).
const REPLAY_BLESS_ENV: &str = "STARFALL_REPLAY_BLESS";

/// `fixture_dir/name` 에 추적된 골든을 방금 만든 `generated` 와 바이트 단위로 대조한다.
/// `STARFALL_REPLAY_BLESS=1` 이면 대조 대신 덮어쓴다(의도적 재생성일 때만 켠다).
///
/// 실패 메시지에 바이트 전체를 찍지 않는다 — `snapshots.jsonl` 은 500 KB 를 넘는다.
/// 대신 길이와 첫 차이 오프셋만 알려준다.
fn compare_or_bless_golden(fixture_dir: &Path, name: &str, generated: &[u8]) {
    let golden_path = fixture_dir.join(name);

    if std::env::var(REPLAY_BLESS_ENV).as_deref() == Ok("1") {
        fs::write(&golden_path, generated)
            .unwrap_or_else(|e| panic!("{} bless 쓰기 실패: {e}", golden_path.display()));
        println!(
            "[bless] {} 를 방금 만든 산출물로 덮어썼다 ({} 바이트) — {REPLAY_BLESS_ENV} 를 \
             끄고 다시 돌려 golden 이 이제 대조를 통과하는지 확인하라",
            golden_path.display(),
            generated.len()
        );
        return;
    }

    let golden = fs::read(&golden_path).unwrap_or_else(|e| {
        panic!(
            "골든 파일을 읽을 수 없다: {}({e}) — 처음 만드는 경우라면 \
             `{REPLAY_BLESS_ENV}=1 cargo test -p starfall-sim --test determinism \
             ac8_two_process_replay_produces_byte_identical_snapshots` 로 한 번 만든다",
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
             첫 차이 오프셋 {first_diff}). 최근 물리·이월 코드 변경이 재생 결과를 바꿨을 \
             수 있다는 신호다 — 조용히 bless 하지 말고 원인을 먼저 확인하라. 의도된 \
             변경이 맞다면 `{REPLAY_BLESS_ENV}=1 cargo test -p starfall-sim --test \
             determinism ac8_two_process_replay_produces_byte_identical_snapshots` 로 \
             재생성한다.",
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

/// `ShipState` JSON 하나의 `ω_aim` 세 필드(x/y/z) 중 하나라도 0이 아닌가 — AC-12(e) 커버리지
/// 확인용(S8).
fn ship_has_nonzero_aim(ship: &serde_json::Value) -> bool {
    [
        "angular_velocity_x_mdeg_s",
        "angular_velocity_y_mdeg_s",
        "angular_velocity_z_mdeg_s",
    ]
    .iter()
    .any(|field| ship[field].as_i64() != Some(0))
}

fn spawn_replay_child(inputs_path: &Path, out_dir: &Path) -> std::process::Output {
    let exe = std::env::current_exe().expect("현재 테스트 바이너리 경로");
    Command::new(exe)
        .args(["replay_worker", "--exact", "--ignored", "--test-threads=1"])
        .env("STARFALL_REPLAY_INPUTS", inputs_path)
        .env("STARFALL_REPLAY_OUT_DIR", out_dir)
        .output()
        .expect("자식 프로세스 실행 실패")
}

/// AC-8 / SC-34 — 같은 입력열을 서로 다른 프로세스에서 2회 돌려 스냅샷 바이트가
/// 동일함을 확인한다. **근사 비교를 쓰지 않는다.**
#[test]
fn ac8_two_process_replay_produces_byte_identical_snapshots() {
    let fixture_dir = replay_fixture_dir();

    // `inputs.jsonl` 은 추적 대상(golden)이지 정본이 아니다 — [`maneuver_plan`] 이 정본,
    // 파일은 증거다. 그래서 추적된 위치가 아니라 스크래치에 쓴다(architect 결정,
    // ADR-0010 §3.1) — 이 테스트를 그냥 돌리는 것만으로 추적 파일이 바뀌면 안 된다.
    let scratch = std::env::temp_dir().join(format!("starfall-ac8-{}", std::process::id()));
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
    let initial1 = fs::read(out1.join("initial.json")).expect("run1 initial.json");
    let initial2 = fs::read(out2.join("initial.json")).expect("run2 initial.json");

    let tick_count = String::from_utf8_lossy(&snapshots1).lines().count();
    println!(
        "[AC-8/SC-34] 비교한 스냅샷 줄 수 = {tick_count}, snapshots.jsonl 총 바이트 = {} (두 프로세스 동일)",
        snapshots1.len()
    );

    assert_eq!(
        snapshots1, snapshots2,
        "서로 다른 프로세스 2회 실행의 snapshots.jsonl 이 바이트 단위로 달랐다 — 결정성 위반(I-37)"
    );
    assert_eq!(
        initial1, initial2,
        "서로 다른 프로세스 2회 실행의 initial.json 이 달랐다 — tick 0 조립도 결정적이어야 한다"
    );
    assert!(
        tick_count > 1,
        "스냅샷이 최소 여러 줄은 나와야 재생이 의미가 있다"
    );

    // AC-12(e) — 각속도 두 필드(ω_aim/ω_roll)를 각각 되돌리지 않으면 실패하는 케이스가
    // 이 재생 안에 있어야 한다(S8). 합에서 분해하는 손실 구현은 ω_aim·ω_roll 이 **동시에**
    // 0이 아닌 tick에서만 걸린다 — 둘 중 하나만 0이 아니면 "합으로 보내도 우연히 맞는다".
    let has_simultaneous_aim_and_roll = String::from_utf8_lossy(&snapshots1)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|payload| {
            payload["ships"].as_array().is_some_and(|ships| {
                ships.iter().any(|ship| {
                    ship["actor_id"].as_str() == Some(&actor_a().to_string())
                        && ship_has_nonzero_aim(ship)
                        && ship["angular_velocity_roll_mdeg_s"].as_i64() != Some(0)
                })
            })
        });
    assert!(
        has_simultaneous_aim_and_roll,
        "AC-12(e) 미충족: alpha 의 ω_aim 과 ω_roll 이 동시에 0이 아닌 스냅샷이 없다 \
         — 이 재생으로는 각속도를 합에서 분해하는 손실 구현도 통과해 버린다(S8)"
    );

    // C4(client)가 상대 경로로 그대로 읽는 세 파일은 이제 **추적 대상**이다(ADR-0010
    // §3.1) — 조용히 덮어쓰지 않고 대조한다. run1 = run2 는 이미 위에서 증명됐으니
    // run1 산출물을 대표로 쓴다. `inputs.jsonl` 은 `maneuver_plan()`(정본)을 방금
    // 스크래치에 직렬화한 결과와 대조한다.
    let inputs_generated =
        fs::read(&inputs_path).expect("스크래치에 방금 쓴 inputs.jsonl 을 다시 읽을 수 없다");
    compare_or_bless_golden(&fixture_dir, "inputs.jsonl", &inputs_generated);
    compare_or_bless_golden(&fixture_dir, "initial.json", &initial1);
    compare_or_bless_golden(&fixture_dir, "snapshots.jsonl", &snapshots1);

    let _ = fs::remove_dir_all(&scratch); // 정리 실패는 테스트 결과에 영향 없다.
}
