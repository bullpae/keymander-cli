//! 마우스 워커 — 키 홀드 상태를 연속 포인터 이동으로 변환 (플랫폼 공용)
//!
//! 엔진의 MouseEngage/MouseRelease 결정으로 방향 플래그를 세우고,
//! 전용 스레드가 125Hz(8ms) 틱으로 가속 커브를 적용한 상대 이동을
//! [`MouseSink`]에 넘긴다. 주입(SendInput/CGEvent)은 어댑터 몫이다.
//!
//! 가속: 시작 180px/s → 500ms에 걸쳐 1300px/s (QMK kinetic 모드와 유사).
//! 첫 몇 픽셀은 정밀 조준, 길게 누르면 화면을 가로지른다.
//! 이동이 완전히 멈추면 램프가 초기화된다.

use super::MouseBind;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// 틱 간격 — 125Hz
const TICK: Duration = Duration::from_millis(8);
/// 시작 속도 (px/s)
const MIN_SPEED: f32 = 180.0;
/// 최대 속도 (px/s)
const MAX_SPEED: f32 = 1300.0;
/// 최대 속도 도달 시간 (ms)
const RAMP_MS: f32 = 500.0;
/// 저속 정밀 모드 배율 (mouse:slow 홀드)
const SLOW_FACTOR: f32 = 0.25;
/// 휠 속도 (노치/초)
const WHEEL_NOTCHES_PER_SEC: f32 = 6.0;

/// 상대 이동/휠 주입 — 플랫폼 어댑터가 구현
pub trait MouseSink: Send + 'static {
    /// 포인터 상대 이동 (픽셀)
    fn move_rel(&mut self, dx: i32, dy: i32);
    /// 휠 스크롤 (+1 = 위로 1노치)
    fn wheel(&mut self, notches: i32);
}

#[derive(Default)]
struct MotionState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    wheel_up: bool,
    wheel_down: bool,
    slow: bool,
    /// 워커 종료 신호
    shutdown: bool,
}

impl MotionState {
    fn active(&self) -> bool {
        self.up || self.down || self.left || self.right || self.wheel_up || self.wheel_down
    }
}

/// 마우스 모션 워커 — 버튼은 다루지 않는다 (버튼 down/up은 어댑터가
/// 키 이벤트 시점에 직접 주입해야 드래그 지연이 없다)
pub struct MouseWorker {
    state: Arc<(Mutex<MotionState>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MouseWorker {
    pub fn start<S: MouseSink>(mut sink: S) -> Self {
        let state = Arc::new((Mutex::new(MotionState::default()), Condvar::new()));
        let shared = state.clone();

        let thread = std::thread::spawn(move || {
            let (lock, cv) = &*shared;
            let mut ramp_start: Option<Instant> = None;
            // 픽셀 미만 이동량 누적 (틱당 소수 픽셀을 버리지 않는다)
            let mut acc_x = 0f32;
            let mut acc_y = 0f32;
            let mut acc_wheel = 0f32;

            loop {
                let (u, d, l, r, wu, wd, slow) = {
                    let mut guard = match lock.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    while !guard.active() && !guard.shutdown {
                        // 완전 정지 — 램프/누적 초기화 후 대기
                        ramp_start = None;
                        acc_x = 0.0;
                        acc_y = 0.0;
                        acc_wheel = 0.0;
                        guard = match cv.wait(guard) {
                            Ok(g) => g,
                            Err(_) => return,
                        };
                    }
                    if guard.shutdown {
                        return;
                    }
                    (
                        guard.up,
                        guard.down,
                        guard.left,
                        guard.right,
                        guard.wheel_up,
                        guard.wheel_down,
                        guard.slow,
                    )
                };

                // ── 이동 속도: 시간 기반 가속 램프 ──
                let now = Instant::now();
                let start = *ramp_start.get_or_insert(now);
                let t_ms = now.duration_since(start).as_secs_f32() * 1000.0;
                let mut speed = MIN_SPEED + (MAX_SPEED - MIN_SPEED) * (t_ms / RAMP_MS).min(1.0);
                if slow {
                    speed *= SLOW_FACTOR;
                }

                let mut vx = (r as i8 - l as i8) as f32;
                let mut vy = (d as i8 - u as i8) as f32;
                if vx != 0.0 && vy != 0.0 {
                    // 대각선 정규화 — 축 이동과 같은 체감 속도
                    let inv = std::f32::consts::FRAC_1_SQRT_2;
                    vx *= inv;
                    vy *= inv;
                }

                let dt = TICK.as_secs_f32();
                acc_x += vx * speed * dt;
                acc_y += vy * speed * dt;
                let dx = acc_x as i32;
                let dy = acc_y as i32;
                acc_x -= dx as f32;
                acc_y -= dy as f32;
                if dx != 0 || dy != 0 {
                    sink.move_rel(dx, dy);
                }

                // ── 휠: 고정 속도 노치 누적 ──
                let wv = (wu as i8 - wd as i8) as f32;
                if wv != 0.0 {
                    acc_wheel += wv * WHEEL_NOTCHES_PER_SEC * dt;
                    let notches = acc_wheel as i32;
                    if notches != 0 {
                        acc_wheel -= notches as f32;
                        sink.wheel(notches);
                    }
                }

                std::thread::sleep(TICK);
            }
        });

        Self {
            state,
            thread: Some(thread),
        }
    }

    /// 이동/휠/저속 바인딩 시작. 버튼 바인딩은 무시한다 (어댑터 직접 처리).
    pub fn engage(&self, bind: MouseBind) {
        self.set(bind, true);
    }

    /// 이동/휠/저속 바인딩 정지
    pub fn release(&self, bind: MouseBind) {
        self.set(bind, false);
    }

    /// 모든 모션 정지 (레이어 트리거 해제, keymap 토글, 종료 시)
    pub fn stop_all(&self) {
        let (lock, cv) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            *guard = MotionState {
                shutdown: guard.shutdown,
                ..MotionState::default()
            };
            cv.notify_one();
        }
    }

    fn set(&self, bind: MouseBind, on: bool) {
        let (lock, cv) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            match bind {
                MouseBind::MoveUp => guard.up = on,
                MouseBind::MoveDown => guard.down = on,
                MouseBind::MoveLeft => guard.left = on,
                MouseBind::MoveRight => guard.right = on,
                MouseBind::WheelUp => guard.wheel_up = on,
                MouseBind::WheelDown => guard.wheel_down = on,
                MouseBind::Slow => guard.slow = on,
                MouseBind::BtnLeft | MouseBind::BtnRight | MouseBind::BtnMiddle => return,
            }
            cv.notify_one();
        }
    }
}

impl Drop for MouseWorker {
    fn drop(&mut self) {
        let (lock, cv) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            guard.shutdown = true;
            cv.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct RecordingSink(mpsc::Sender<(i32, i32)>);
    impl MouseSink for RecordingSink {
        fn move_rel(&mut self, dx: i32, dy: i32) {
            let _ = self.0.send((dx, dy));
        }
        fn wheel(&mut self, _notches: i32) {}
    }

    #[test]
    fn engage_moves_release_stops() {
        let (tx, rx) = mpsc::channel();
        let worker = MouseWorker::start(RecordingSink(tx));

        worker.engage(MouseBind::MoveRight);
        // 수 틱 안에 오른쪽 이동이 나와야 한다
        let (dx, dy) = rx
            .recv_timeout(Duration::from_millis(500))
            .expect("이동 발생");
        assert!(dx > 0, "오른쪽 이동 기대: dx={dx}");
        assert_eq!(dy, 0);

        worker.release(MouseBind::MoveRight);
        // 정지 후 잔여 틱을 비우고 나면 더 이상 이동이 없어야 한다
        std::thread::sleep(Duration::from_millis(50));
        while rx.try_recv().is_ok() {}
        std::thread::sleep(Duration::from_millis(100));
        assert!(rx.try_recv().is_err(), "정지 후 이동 없음");
    }

    #[test]
    fn stop_all_clears_motion() {
        let (tx, rx) = mpsc::channel();
        let worker = MouseWorker::start(RecordingSink(tx));
        worker.engage(MouseBind::MoveLeft);
        worker.engage(MouseBind::MoveUp);
        rx.recv_timeout(Duration::from_millis(500))
            .expect("이동 발생");

        worker.stop_all();
        std::thread::sleep(Duration::from_millis(50));
        while rx.try_recv().is_ok() {}
        std::thread::sleep(Duration::from_millis(100));
        assert!(rx.try_recv().is_err(), "stop_all 후 이동 없음");
    }

    #[test]
    fn slow_mode_reduces_step() {
        let (tx, rx) = mpsc::channel();
        let worker = MouseWorker::start(RecordingSink(tx));

        // slow 모드에서 초기 틱 이동량은 일반 모드보다 작아야 한다
        worker.engage(MouseBind::Slow);
        worker.engage(MouseBind::MoveRight);
        let mut slow_first = 0;
        for _ in 0..3 {
            let (dx, _) = rx.recv_timeout(Duration::from_millis(500)).expect("이동");
            slow_first += dx;
        }
        worker.stop_all();
        // slow: 180*0.25 = 45px/s → 8ms 틱당 0.36px → 3틱에 1px 안팎
        assert!(slow_first <= 3, "저속 3틱 누적 {slow_first}px");
    }
}
