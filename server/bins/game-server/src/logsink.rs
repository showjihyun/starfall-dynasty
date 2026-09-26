//! 로그 싱크 — **서버는 로그를 읽는 쪽 때문에 멈추지 않는다.**
//!
//! # 왜 이 모듈이 있는가 (p1-01 라운드 2, S-B·S-C)
//!
//! `tracing_subscriber::fmt()` 의 기본 writer 는 `std::io::stdout()` 이고, 그 쓰기는
//! **이벤트를 낸 스레드에서 동기로** 일어난다. stdout 이 파이프이고 읽는 쪽이 드레인하지
//! 않으면(예: `subprocess.PIPE` 로 띄워 두고 종료 후에만 `read()` 하는 드라이버) 파이프가
//! 차는 순간 그 `tracing::…!` 호출이 **그 자리에서 블로킹**된다. 실측(2026-09-21):
//!
//! | 관측 | 값 |
//! |---|---|
//! | 파이프 용량 | **4096 B** (Python `subprocess` 의 익명 파이프) |
//! | 멈춘 스레드 | tokio 워커 **12개** 전부 (`Wait, Unknown`) |
//! | 프로세스 | 살아 있음. CPU 누적 **0.28초** — 스핀이 아니라 정지 |
//!
//! 그래서 이렇게 된다:
//!
//! 1. `/ws` 핸들러가 로그 한 줄에서 멈춘다 → TCP 는 붙는데 `SESSION_READY` 가 없다.
//! 2. `serve_session` 이 종료 로그에서 멈춘다 → `submit.close()` 에 **도달하지 못한다**
//!    → 세션이 영원히 안 닫히고(`ws_connections` 가 거짓말), 함선이 `ACTIVE` 로 잔존하며,
//!    송신 태스크가 다시 깨지 않아 `IDLE_TIMEOUT` 도 발동하지 않는다.
//! 3. 워커가 다 잠기면 HTTP(`/healthz`·`/debug/stats`)도, `with_graceful_shutdown` 이
//!    기다리는 stdin `shutdown` 퓨처도 **폴링되지 않는다** → 정상 종료가 안 먹는다.
//!
//! **로그는 진단이지 세계의 사실이 아니다.** 진단을 쓰다가 세계가 멈추면 안 된다.
//!
//! # 어떻게 막는가
//!
//! 이벤트를 내는 스레드는 포맷된 줄을 **유계 큐에 넣기만** 한다(`try_send`). 실제 쓰기는
//! 전용 OS 스레드가 한다 — 그 스레드가 파이프에서 막혀도 tokio 워커도 tick 스레드도
//! 영향받지 않는다. 큐가 차면 줄을 **버리고 센다**. 버린 사실은 조용히 넘어가지 않고,
//! 소비자가 다시 읽기 시작하는 순간 로그 스트림에 그대로 찍힌다.
//!
//! 버리는 쪽을 택한 이유: 대안은 막는 것이고, 막는 것이 바로 이 결함이었다.

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};

use tracing_subscriber::fmt::MakeWriter;

/// 큐에 담아 둘 수 있는 로그 줄 수.
///
/// 한 줄이 평균 200 B 정도이므로 최악 약 800 KB 다. 파이프 용량(4 KiB)의 200배이니
/// **잠깐 느린** 소비자는 한 줄도 잃지 않고, **멈춘** 소비자는 메모리를 무한정 먹는
/// 대신 줄을 잃는다.
const QUEUE_LINES: usize = 4096;

/// 종료할 때 싱크가 비워지기를 기다리는 최대 시간. 여기서 무한정 기다리면 "로그 때문에
/// 서버가 안 내려간다"가 되어 고치려던 결함으로 되돌아간다.
const FLUSH_DEADLINE: Duration = Duration::from_secs(2);

/// 드레인 스레드가 "더 줄이 없는가"를 다시 확인하는 주기 (PR #1 리뷰 결함 4).
///
/// 채널 disconnect 로는 종료를 알 수 없다 — 전역 tracing subscriber 에 설치된
/// `NonBlockingStdout` 복제본(`:make_writer` 가 아니라 subscriber 가 **소유한** 값)이
/// 자기 `tx` 를 프로세스가 죽을 때까지 들고 있어서, `FlushGuard::drop` 이 자기 몫의
/// `tx` 하나만 놓아도 채널은 절대 닫히지 않는다. 그래서 "끝"은 채널이 아니라
/// `Shared::shutdown` 플래그로 판단한다.
const DRAIN_POLL: Duration = Duration::from_millis(20);

#[derive(Debug)]
struct Shared {
    /// 큐가 가득 차 버린 줄 수.
    dropped: AtomicU64,
    /// `true` 가 되면 드레인 스레드는 큐에 남은 줄을 마저 비우고 끝난다(결함 4 수정).
    /// `FlushGuard::drop` 이 세운다.
    shutdown: AtomicBool,
}

/// `tracing_subscriber` 에 넘기는 writer 팩토리.
#[derive(Debug, Clone)]
pub struct NonBlockingStdout {
    tx: SyncSender<Vec<u8>>,
    shared: Arc<Shared>,
}

/// 프로세스 종료 시 싱크를 비우는 가드. **`main` 이 끝날 때까지 들고 있어야 한다** —
/// 떨어뜨리면 그 시점에 드레인 스레드가 끝난다.
#[derive(Debug)]
pub struct FlushGuard {
    tx: Option<SyncSender<Vec<u8>>>,
    drain: Option<std::thread::JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl Drop for FlushGuard {
    fn drop(&mut self) {
        // 송신단을 놓는다 — 하지만 **이것만으로는 드레인 스레드가 끝나지 않는다**
        // (결함 4). 전역 tracing subscriber 에 설치된 `NonBlockingStdout` 이 `tx`
        // 복제본을 하나 더 들고 있고, 그건 여기서 못 건드린다 — 그래서 `shutdown`
        // 플래그로 직접 신호를 보낸다(`drain_loop` 참고). 남는 tx 를 여기서 놓는 것도
        // 여전히 의미가 있다: 이 값이 유일한 sender 였던 경우(테스트 등) 채널이 실제로
        // 닫혀 드레인이 더 빨리 끝난다.
        drop(self.tx.take());
        self.shared.shutdown.store(true, Ordering::SeqCst);
        let Some(drain) = self.drain.take() else {
            return;
        };
        // **유계 대기**. 소비자가 멈춰 있으면 드레인 스레드는 영영 끝나지 않는다 —
        // 그때는 남은 줄을 포기하고 프로세스를 내보낸다.
        let deadline = Instant::now() + FLUSH_DEADLINE;
        while !drain.is_finished() {
            if Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = drain.join();
    }
}

/// 비블로킹 stdout 싱크를 만든다. 반환된 가드는 `main` 이 끝날 때까지 살아 있어야 한다.
///
/// 드레인 스레드를 만들지 못하면 `None` 을 돌려준다 — 호출자는 기본(동기) writer 로
/// 돌아가야 한다. 로그가 안 나오는 서버보다는 (이 결함이 있더라도) 나오는 서버가 낫다.
#[must_use]
pub fn non_blocking_stdout() -> Option<(NonBlockingStdout, FlushGuard)> {
    let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE_LINES);
    let shared = Arc::new(Shared {
        dropped: AtomicU64::new(0),
        shutdown: AtomicBool::new(false),
    });
    let drain_shared = Arc::clone(&shared);
    let drain = std::thread::Builder::new()
        .name("starfall-log".to_owned())
        .spawn(move || drain_loop(&rx, &drain_shared))
        .ok()?;

    Some((
        NonBlockingStdout {
            tx: tx.clone(),
            shared: Arc::clone(&shared),
        },
        FlushGuard {
            tx: Some(tx),
            drain: Some(drain),
            shared,
        },
    ))
}

/// 전용 스레드. **여기서만 stdout 에 실제로 쓴다** — 그래서 여기서 막혀도 서버는 돈다.
fn drain_loop(rx: &Receiver<Vec<u8>>, shared: &Shared) {
    drain_into(rx, shared, |line| {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(line);
        let _ = out.flush();
    });
}

/// `drain_loop` 의 순수 로직 — 실제 stdout 대신 임의의 sink 로 테스트할 수 있게
/// 분리한다(결함 4 수정, 유닛 테스트 참고).
///
/// **채널 disconnect 를 종료 신호로 쓰지 않는다.** 전역 subscriber 가 든
/// `NonBlockingStdout` 복제본이 자기 `tx` 를 프로세스가 죽을 때까지 들고 있어서
/// disconnect 는 (정상적인 운영에서는) 일어나지 않는다 — 그래서 `shared.shutdown` 을
/// 짧은 주기로 폴링한다. `shutdown` 이 서기 전에 큐에 들어온 줄은 반드시 다 내보낸
/// 뒤에야 끝난다: 루프를 빠져나온 뒤 `try_recv` 로 마저 비운다.
fn drain_into<F: FnMut(&[u8])>(rx: &Receiver<Vec<u8>>, shared: &Shared, mut sink: F) {
    let mut reported = 0u64;
    loop {
        match rx.recv_timeout(DRAIN_POLL) {
            Ok(line) => write_line(&line, shared, &mut reported, &mut sink),
            Err(RecvTimeoutError::Timeout) => {
                if shared.shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
            // 정상 운영에서는 도달하지 않는다(위 문서 참고) — 방어적으로만 처리한다.
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // 종료 신호 이후에도 큐에 남아 있을 수 있는 줄을 전부 비운다 — "빨리 끝나는 것"이
    // 아니라 "남은 줄을 전부 내보낸 뒤 끝나는 것"이 요구사항이다.
    while let Ok(line) = rx.try_recv() {
        write_line(&line, shared, &mut reported, &mut sink);
    }
}

fn write_line<F: FnMut(&[u8])>(line: &[u8], shared: &Shared, reported: &mut u64, sink: &mut F) {
    sink(line);

    // 버린 줄이 있었다면 **그 사실을 로그 스트림 안에** 남긴다. 조용히 사라지면
    // 리포트를 읽는 쪽이 "로그가 끊겼다"를 다시 서버 정지로 오해한다.
    let dropped = shared.dropped.load(Ordering::Relaxed);
    if dropped > *reported {
        let message = format!(
            "[로그 싱크] 소비자가 느려 {}줄을 버렸다 (누적 {dropped}) — 서버는 계속 돈다\n",
            dropped - *reported
        );
        sink(message.as_bytes());
        *reported = dropped;
    }
}

/// 이벤트 한 건이 쓰이는 임시 버퍼. drop 될 때 큐로 넘어간다.
#[derive(Debug)]
pub struct QueuedLine {
    buffer: Vec<u8>,
    tx: SyncSender<Vec<u8>>,
    shared: Arc<Shared>,
}

impl QueuedLine {
    fn submit(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.buffer);
        match self.tx.try_send(line) {
            Ok(()) => {}
            // **막지 않는다.** 버리고 센다 — 이것이 이 모듈의 존재 이유다.
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.shared.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl Write for QueuedLine {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.submit();
        Ok(())
    }
}

impl Drop for QueuedLine {
    fn drop(&mut self) {
        self.submit();
    }
}

impl<'a> MakeWriter<'a> for NonBlockingStdout {
    type Writer = QueuedLine;

    fn make_writer(&'a self) -> Self::Writer {
        QueuedLine {
            buffer: Vec::with_capacity(256),
            tx: self.tx.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// 큐가 가득 차도 **쓰는 쪽은 막히지 않는다** — 이 파일의 존재 이유 그 자체다.
    ///
    /// 드레인 스레드를 흉내 내지 않고(= 아무도 읽지 않고) 용량의 두 배를 밀어 넣는다.
    /// 동기 writer 였다면 여기서 영원히 멈춘다.
    #[test]
    fn a_full_queue_drops_lines_instead_of_blocking() {
        let (tx, rx) = sync_channel::<Vec<u8>>(4);
        let shared = Arc::new(Shared {
            dropped: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
        });
        let sink = NonBlockingStdout {
            tx,
            shared: Arc::clone(&shared),
        };

        for n in 0..16 {
            let mut line = sink.make_writer();
            writeln!(line, "line {n}").unwrap();
            // drop → submit
        }

        assert_eq!(shared.dropped.load(Ordering::Relaxed), 12, "4칸만 들어간다");
        let mut received = 0;
        while rx.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 4);
    }

    /// 여유가 있으면 한 줄도 버리지 않고, 포맷된 바이트가 그대로 넘어간다.
    #[test]
    fn lines_reach_the_queue_untouched_when_there_is_room() {
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let shared = Arc::new(Shared {
            dropped: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
        });
        let sink = NonBlockingStdout { tx, shared };

        {
            let mut line = sink.make_writer();
            // tracing 의 fmt 레이어는 한 이벤트를 여러 번 나눠 쓴다.
            line.write_all(b"INFO ").unwrap();
            line.write_all(b"hello\n").unwrap();
        }

        assert_eq!(rx.try_recv().unwrap(), b"INFO hello\n");
    }

    /// 가드를 떨어뜨리면 드레인 스레드가 큐를 비우고 끝난다 (종료 시 마지막 줄 보존).
    #[test]
    fn dropping_the_guard_flushes_and_stops_the_drain_thread() {
        let (sink, guard) = non_blocking_stdout().expect("드레인 스레드");
        {
            let mut line = sink.make_writer();
            writeln!(line, "[테스트] logsink flush 경로").unwrap();
        }
        drop(sink);
        drop(guard); // 여기서 막히면 테스트가 걸린다 — 유계 대기가 그것을 막는다.
    }

    /// PR #1 리뷰 결함 4 — 전역 subscriber 가 든 `NonBlockingStdout` 복제본은
    /// `FlushGuard::drop`이 절대 놓지 않는다(자기 `tx` 만 놓는다, `:79` 참고). 그래서
    /// 지금 코드는 `rx.recv()` 가 `Disconnected` 를 볼 일이 없어 **항상** `FLUSH_DEADLINE`
    /// (2초)까지 기다린다 — stdin `shutdown` 한 번마다 2초 지연.
    ///
    /// 이 테스트는 그 조건을 그대로 재현한다: `sink`(전역 구독자가 들고 있을 원본
    /// `NonBlockingStdout`, `tx` 보유)를 **드롭하지 않고** `guard` 만 드롭한다.
    ///
    /// 시간 단언은 실제 관측(§7a — "드레인이 실제로 끝났는가")의 **보조**일 뿐이다.
    /// 주 단언은 아래 `drain_into_...` 유닛 테스트가 `is_finished()`(폴링, 시간이 아닌
    /// 완료 여부)로 진다.
    #[test]
    fn dropping_the_guard_does_not_wait_for_a_surviving_sender_clone() {
        let (sink, guard) = non_blocking_stdout().expect("드레인 스레드");
        {
            let mut line = sink.make_writer();
            writeln!(line, "[테스트] 살아남은 sender 아래 flush").unwrap();
        }
        // `sink` 를 일부러 드롭하지 않는다 — 전역 tracing subscriber 가 이 값을 영구히
        // 들고 있는 상황을 흉내 낸다. `drop(sink)` 를 넣으면 이 테스트는 결함을 재현하지
        // 못한다(그러면 유일한 sender 가 없어져 `rx.recv()` 가 정상적으로 `Disconnected`
        // 를 본다).
        let start = Instant::now();
        drop(guard);
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(500),
            "guard 가 살아있는 sender clone 때문에 FLUSH_DEADLINE(2s) 까지 기다렸다: {elapsed:?}"
        );
        drop(sink);
    }

    /// PR #1 리뷰 결함 4의 **주 단언**. `drain_into` 를 직접 돌려서 (1) `shutdown` 신호
    /// 뒤 살아있는 sender 가 있어도 실제로 끝나는지(`is_finished()` 폴링 — 시간이 아니라
    /// 완료 여부), (2) 신호 전에 큐에 있던 줄이 **전부** sink 에 도달했는지(버려지지
    /// 않았는지)를 함께 본다. "빨리 끝난다"만으로는 부족하다 — "남은 줄을 전부 내보낸
    /// 뒤 끝난다"가 요구사항이다.
    #[test]
    fn drain_into_flushes_every_queued_line_then_stops_even_with_a_live_sender() {
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let shared = Arc::new(Shared {
            dropped: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
        });

        tx.send(b"line-1\n".to_vec()).unwrap();
        tx.send(b"line-2\n".to_vec()).unwrap();
        tx.send(b"line-3\n".to_vec()).unwrap();

        // 전역 subscriber 가 들고 있을 복제본을 흉내 낸다 — drain_into 는 이게 살아
        // 있어도 shutdown 플래그만으로 끝나야 한다.
        let _surviving_clone = tx.clone();

        let collected = Arc::new(Mutex::new(Vec::<u8>::new()));
        let collected_in_drain = Arc::clone(&collected);
        let shared_in_drain = Arc::clone(&shared);
        let drain = std::thread::spawn(move || {
            drain_into(&rx, &shared_in_drain, |line: &[u8]| {
                collected_in_drain.lock().unwrap().extend_from_slice(line);
            });
        });

        // 드레인 스레드가 큐에 있던 3줄을 소비할 시간을 준 뒤 종료 신호를 보낸다.
        std::thread::sleep(Duration::from_millis(100));
        shared.shutdown.store(true, Ordering::SeqCst);

        // **주 단언**: 실제로 끝났는가(시간이 아니라 상태) — 관대한 상한(1초 ≫
        // `DRAIN_POLL` 20ms)은 "테스트가 영원히 안 걸리게"일 뿐, 빠름을 재는 게 아니다.
        let deadline = Instant::now() + Duration::from_secs(1);
        while !drain.is_finished() {
            assert!(
                Instant::now() < deadline,
                "drain_into 가 shutdown 신호 뒤에도 끝나지 않았다 — 살아있는 sender 가 \
                 여전히 완료를 막고 있다(결함 4)"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        drain.join().unwrap();

        assert_eq!(
            &*collected.lock().unwrap(),
            b"line-1\nline-2\nline-3\n",
            "shutdown 신호 전에 큐에 있던 줄이 전부 나가지 않고 잘렸다"
        );
    }
}
