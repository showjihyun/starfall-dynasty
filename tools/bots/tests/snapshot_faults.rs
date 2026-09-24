//! **스냅샷 계측기의 자체 검증 (SC-85).**
//!
//! p0-02에서 이 형태의 테스트가 라운드 1을 구했다 — 거짓 FAIL 2건과 미검출 위험 1건을
//! 서버가 아니라 **하네스**에서 찾았다. p1-01은 계측 축이 하나 더 늘었으므로(스냅샷) 같은
//! 방식으로 **고장을 주입해 빨간불이 켜지는지** 본다.
//!
//! 정상 경로가 통과하는 것은 증거가 아니다. **틀렸을 때 잡히는가**가 증거다.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

use starfall_bots::conn::{Behavior, BotSpec, Clock, ConnectionOutcome, run_connection};
use starfall_bots::snapshot::SNAPSHOT_CSV_HEADER;
use starfall_bots::token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    /// ① 스냅샷 1건을 빠뜨린다 → 간격이 2배가 되어 **간격 위반**으로 잡혀야 한다.
    DropOne,
    /// ② `ships` 를 `ship_id` 내림차순으로 보낸다.
    DescendingShips,
    /// ③ `ack_input_seq` 를 한 번 되돌린다.
    AckRewind,
    /// ⑤ 프레임 바이트를 부풀린다(패딩 필드) → 대역폭 계측이 따라 커져야 한다.
    InflateBytes,
    /// ⑥ `presence` 를 `LINGERING` 으로 고정한다.
    AlwaysLingering,
    /// ⑦ 스냅샷 간격을 선언값(2)이 아니라 3 tick 으로 보낸다.
    WrongInterval,
    /// `controlled_ship_id` 를 배열에 없는 값으로 보낸다.
    ControlledMissing,
}

const SHIP_A: &str = "01a0c000-0000-7000-8000-00000000000a";
const SHIP_B: &str = "01a0c000-0000-7000-8000-00000000000b";
const ACTOR: &str = "01a0b1c2-b010-7000-8000-000000000000";

struct Fake {
    addr: std::net::SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    sent: Arc<Mutex<u32>>,
}

fn ship(id: &str, actor: &str, x: i64, presence: &str) -> serde_json::Value {
    serde_json::json!({
        "ship_id": id, "actor_id": actor, "ship_class_id": "scout-s01", "presence": presence,
        "position_x_mm": x, "position_y_mm": 0, "position_z_mm": 1234,
        "velocity_x_mm_s": 0, "velocity_y_mm_s": 0, "velocity_z_mm_s": 35000,
        "orientation_x_micro": 0, "orientation_y_micro": 0, "orientation_z_micro": 0,
        "orientation_w_micro": 1_000_000,
        "angular_velocity_x_mdeg_s": 0, "angular_velocity_y_mdeg_s": 0,
        "angular_velocity_z_mdeg_s": 0, "angular_velocity_roll_mdeg_s": 0
    })
}

async fn spawn_fake(fault: Fault, snapshots: u32) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let sent = Arc::new(Mutex::new(0u32));
    let sent_c = Arc::clone(&sent);

    let handle = tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        #[allow(clippy::result_large_err)]
        let cb = |_req: &Request, resp: Response| -> Result<Response, ErrorResponse> { Ok(resp) };
        let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, cb).await else {
            return;
        };

        let ready = serde_json::json!({
            "message_id": uuid::Uuid::now_v7(), "message_type": "SESSION_READY",
            "schema_version": 1, "tick": 100, "correlation_id": uuid::Uuid::now_v7(),
            "payload": {
                "session_id": uuid::Uuid::now_v7(),
                "world_id": "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
                "actor_id": ACTOR, "tick_hz": 20, "server_version": "fake-p1"
            }
        });
        if ws.send(Message::text(ready.to_string())).await.is_err() {
            return;
        }

        let mut tick = 102u64;
        let ack = 1u64;
        for i in 0..snapshots {
            // ① 3번째 스냅샷을 빠뜨린다 — tick 은 정상적으로 흘러간 척한다.
            let skip = fault == Fault::DropOne && i == 3;
            let step = if fault == Fault::WrongInterval { 3 } else { 2 };

            let presence = if fault == Fault::AlwaysLingering {
                "LINGERING"
            } else {
                "ACTIVE"
            };
            let mut ships = vec![
                ship(SHIP_A, ACTOR, 1000 + i64::from(i), presence),
                ship(
                    SHIP_B,
                    "01a0b1c2-b010-7000-8000-000000000001",
                    5000,
                    presence,
                ),
            ];
            if fault == Fault::DescendingShips {
                ships.reverse();
            }

            // ③ 5번째에서 ack 을 되돌린다.
            let this_ack = if fault == Fault::AckRewind && i == 5 {
                1
            } else {
                u64::from(i) + 1
            };
            let _ = ack;

            let controlled = if fault == Fault::ControlledMissing {
                "01a0c000-0000-7000-8000-0000000000ff"
            } else {
                SHIP_A
            };

            let mut payload = serde_json::json!({
                "star_system_id": "cradle",
                "soft_boundary_radius_mm": 10_000_000i64,
                "hard_boundary_radius_mm": 12_000_000i64,
                "snapshot_interval_ticks": 2,
                "controlled_ship_id": controlled,
                "ack_input_seq": this_ack,
                "ships": ships
            });
            // ⑤ 바이트만 부풀린다. 봇의 strict 타입이 거부하지 않도록 **프레임 밖**이 아니라
            //    공백으로 늘린다(같은 JSON, 더 많은 바이트).
            let msg = serde_json::json!({
                "message_id": uuid::Uuid::now_v7(), "message_type": "WORLD_SNAPSHOT",
                "schema_version": 1, "tick": tick, "correlation_id": null,
                "payload": payload.take()
            });
            let text = if fault == Fault::InflateBytes {
                serde_json::to_string_pretty(&msg).unwrap_or_default()
            } else {
                msg.to_string()
            };

            tick += step;
            if skip {
                continue;
            }
            if ws.send(Message::text(text)).await.is_err() {
                return;
            }
            if let Ok(mut g) = sent_c.lock() {
                *g += 1;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }

        // 남은 프레임을 읽어 주고 Close 에 응답한다.
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Close(_) = msg {
                let _ = ws.send(Message::Close(None)).await;
                return;
            }
        }
    });

    Fake { addr, handle, sent }
}

async fn fly_against(fake: &Fake, secs: u64) -> ConnectionOutcome {
    let (_subject, tok) = token::identity("dev_only_not_a_secret", "bot-000");
    run_connection(BotSpec {
        label: "bot-000".to_owned(),
        url: format!("ws://{}/ws", fake.addr),
        token: tok,
        behavior: Behavior::Fly {
            interval: Duration::from_millis(50),
            duration: Duration::from_secs(secs),
            thrust: (0, 0, 1000),
            brake_last: Duration::ZERO,
            phase: Duration::ZERO,
        },
        clock: Clock::start(),
        live_corr: None,
    })
    .await
}

/// 정상 경로 — 게이트가 전부 초록이고 CSV·지표가 채워진다.
#[tokio::test]
async fn healthy_snapshots_pass_every_gate() {
    let fake = spawn_fake(Fault::None, 8).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();

    assert_eq!(s.observer_actor_id.as_deref(), Some(ACTOR));
    assert_eq!(s.controlled_ship_id.as_deref(), Some(SHIP_A));
    assert!(
        s.snapshots_received >= 6,
        "받은 스냅샷 {}",
        s.snapshots_received
    );
    assert_eq!(s.interval_violations, 0);
    assert_eq!(
        s.first_snapshots_excluded, 1,
        "첫 스냅샷은 정확히 1건 제외된다"
    );
    assert_eq!(s.intervals_checked, s.snapshots_received - 1);
    assert_eq!(s.ship_order_violations, 0);
    assert_eq!(s.controlled_ship_missing, 0);
    assert_eq!(s.ack_regressions, 0);
    assert!(s.snapshot_bytes_received > 0);
    assert!(out.snapshots.gates_ok());
    // designer 지표가 실제로 계산된다
    assert!(
        s.own_path_length_m > 0.0,
        "경로 길이가 0이면 관측이 안 된 것이다"
    );
    assert!(s.nearest_ship_min_m.is_some());
    fake.handle.abort();
}

/// **① 스냅샷 1건 누락 → 간격 위반으로 잡힌다.**
#[tokio::test]
async fn dropped_snapshot_shows_up_as_an_interval_violation() {
    let fake = spawn_fake(Fault::DropOne, 8).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();

    assert!(s.interval_violations >= 1, "누락이 잡히지 않았다: {s:?}");
    assert!(!out.snapshots.gates_ok(), "누락이 있는데 게이트가 통과했다");
    // 서버 메트릭과의 대조(SC-70)를 위해 **받은 수 자체**도 남는다.
    let sent = *fake.sent.lock().expect("lock");
    assert_eq!(
        s.snapshots_received,
        u64::from(sent),
        "봇이 센 수신 수가 가짜 서버의 송신 수와 같아야 한다"
    );
    fake.handle.abort();
}

/// ② `ship_id` 내림차순 → 정렬 위반.
#[tokio::test]
async fn descending_ship_order_is_detected() {
    let fake = spawn_fake(Fault::DescendingShips, 6).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();
    assert!(s.ship_order_violations >= 1, "정렬 위반이 잡히지 않았다");
    assert!(!out.snapshots.gates_ok());
    fake.handle.abort();
}

/// ③ `ack_input_seq` 후퇴 → 세션 내 단조 위반.
#[tokio::test]
async fn ack_rewind_is_detected() {
    let fake = spawn_fake(Fault::AckRewind, 8).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();
    assert_eq!(s.ack_regressions, 1, "ack 후퇴가 정확히 1건 잡혀야 한다");
    assert!(!out.snapshots.gates_ok());
    fake.handle.abort();
}

/// ⑤ 바이트 부풀리기 → 대역폭 계측이 따라 커진다(서버 메트릭과 **독립**임의 증거).
#[tokio::test]
async fn byte_accounting_follows_the_wire_not_a_metric() {
    let plain = spawn_fake(Fault::None, 6).await;
    let a = fly_against(&plain, 1).await.snapshots.finish();
    plain.handle.abort();

    let fat = spawn_fake(Fault::InflateBytes, 6).await;
    let b = fly_against(&fat, 1).await.snapshots.finish();
    fat.handle.abort();

    assert_eq!(
        a.snapshots_received, b.snapshots_received,
        "건수는 같아야 한다"
    );
    assert!(
        b.snapshot_bytes_received > a.snapshot_bytes_received,
        "바이트가 늘었는데 계측이 따라오지 않았다: {} vs {}",
        a.snapshot_bytes_received,
        b.snapshot_bytes_received
    );
}

/// ⑥ `presence` 고정 → 전이가 없다는 것이 드러난다.
#[tokio::test]
async fn presence_values_are_recorded_so_a_frozen_value_is_visible() {
    let fake = spawn_fake(Fault::AlwaysLingering, 6).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();
    assert!(s.lingering_seen);
    assert_eq!(
        s.presence_values.get("ACTIVE"),
        None,
        "ACTIVE 가 한 번도 없다"
    );
    assert!(s.presence_values.get("LINGERING").copied().unwrap_or(0) > 0);
    fake.handle.abort();
}

/// ⑦ 선언한 간격과 실제 간격이 다르면 잡힌다.
#[tokio::test]
async fn interval_mismatch_between_declared_and_actual_is_detected() {
    let fake = spawn_fake(Fault::WrongInterval, 6).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();
    assert_eq!(s.snapshot_interval_ticks, Some(2), "서버가 선언한 값은 2다");
    assert!(
        s.interval_violations >= 1,
        "실제 3 tick 간격이 잡히지 않았다"
    );
    fake.handle.abort();
}

/// `controlled_ship_id` 가 배열에 없으면 잡힌다.
#[tokio::test]
async fn missing_controlled_ship_is_detected() {
    let fake = spawn_fake(Fault::ControlledMissing, 6).await;
    let out = fly_against(&fake, 1).await;
    let s = out.snapshots.finish();
    assert!(s.controlled_ship_missing >= 1);
    assert!(!out.snapshots.gates_ok());
    fake.handle.abort();
}

/// ⑨ **CSV 왕복**: 기록한 행을 다시 읽으면 와이어의 양자화 정수가 그대로다.
///
/// 실수로 바꾸면 SC-63("정수로 완전히 같다")을 잴 수 없고, 1 mm 차이가 조용히 사라진다.
#[tokio::test]
async fn csv_preserves_the_exact_quantised_integers() {
    let fake = spawn_fake(Fault::None, 6).await;
    let out = fly_against(&fake, 1).await;

    let rows = out.snapshots.csv_rows();
    assert!(!rows.is_empty(), "CSV 행이 비었다");
    assert_eq!(
        SNAPSHOT_CSV_HEADER,
        "tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg",
        "헤더가 client 와 어긋나면 조인이 0행이 된다"
    );
    for row in rows {
        let f: Vec<&str> = row.split(',').collect();
        assert_eq!(f.len(), 12, "컬럼 수: {row}");
        // R23: 뒤 두 열은 render_offset_mm / render_offset_deg 다. 봇은 평활화가 없으므로
        // 상수 0 이고, **그 0 이 실제로 0 인지**를 여기서 고정한다 — 열만 늘려 놓고
        // 아무 값이나 흘려보내면 client 와 같은 헤더라는 계약이 형식만 남는다.
        assert_eq!(
            f[10], "0",
            "봇은 평활화가 없으므로 render_offset_mm 이 0 이어야 한다: {row}"
        );
        assert_eq!(
            f[11], "0",
            "봇은 평활화가 없으므로 render_offset_deg 가 0 이어야 한다: {row}"
        );
        // 정수 컬럼이 실수 표기로 새지 않는다.
        for col in &f[4..12] {
            assert!(
                !col.contains('.') && !col.contains('e'),
                "양자화 정수가 실수로 기록됐다: {row}"
            );
        }
        assert_eq!(f[1], ACTOR, "observer_actor_id 가 관측자 자신이어야 한다");
        assert_eq!(f[6], "1234", "position_z_mm 가 와이어 값 그대로여야 한다");
    }

    // 같은 함선의 시계열을 되읽어 이동을 복원할 수 있다.
    let ship_a: uuid::Uuid = SHIP_A.parse().expect("uuid");
    let samples = out.snapshots.ship_positions(ship_a);
    assert!(samples.len() >= 5, "표본 {}", samples.len());
    assert!(
        samples.windows(2).all(|w| w[1].tick > w[0].tick),
        "tick 이 증가해야 한다"
    );
    fake.handle.abort();
}
