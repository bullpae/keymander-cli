//! 클립보드 히스토리 — 다중 버퍼 붙여넣기 (docs/12 P1)
//!
//! 데몬이 상주하며 시스템 클립보드 변화를 수집해 링 버퍼에 쌓고, 레이어
//! 바인딩(`clip:N`)이 n번째 최근 항목을 현재 전경 앱에 붙여넣는다.
//!
//! 히스토리가 데몬에 사는 이유: 클립보드 감시는 상주 프로세스만 가능하고
//! 상주 프로세스는 데몬뿐이다 (docs/11 R3 배경).
//!
//! 프라이버시 (키 훅 프로그램의 의무):
//! - **메모리에만** 산다. 디스크 비저장.
//! - 1MB 초과 텍스트는 수집 제외 (메모리 보호).
//! - 내용은 로그에 절대 남기지 않는다 (keycode 불로깅 원칙과 동일).
//! - **비밀번호 제외**: macOS는 `org.nspasteboard.ConcealedType` 마크가 붙은
//!   항목(1Password 등 비번 관리자 관례)을 수집하지 않는다 (P1.1).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// 수집 상한 (텍스트 1건). 초과 시 수집하지 않는다.
const MAX_ITEM_BYTES: usize = 1024 * 1024;
/// 감시 폴링 주기. 유휴 비용은 무시 가능한 수준.
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// 붙여넣기 주입 후 원래 클립보드를 복원하기까지의 지연.
/// 대상 앱이 Cmd+V를 처리할 시간을 준다 (클립보드 관리자 표준 기법).
const RESTORE_DELAY: Duration = Duration::from_millis(300);

/// 히스토리 링 버퍼. 맨 앞(index 0)이 최신.
static HISTORY: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
/// 감시 스레드 중복 기동 방지.
static WATCHER_STARTED: AtomicBool = AtomicBool::new(false);

/// 현재 링 버퍼 상한 (기본 50). spawn_watcher에서 설정.
static CAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(50);

/// 새 클립보드 텍스트를 히스토리 맨 앞에 넣는다 (중복은 앞으로 이동, 상한 유지).
/// 수집 경로와 테스트에서 공용으로 쓴다.
fn record(text: String) {
    if text.is_empty() || text.len() > MAX_ITEM_BYTES {
        return;
    }
    let cap = CAP.load(Ordering::Relaxed).max(1);
    if let Ok(mut h) = HISTORY.lock() {
        // 같은 내용은 중복 생성 없이 맨 앞으로 이동 (dedupe)
        if let Some(pos) = h.iter().position(|e| e == &text) {
            h.remove(pos);
        }
        h.push_front(text);
        while h.len() > cap {
            h.pop_back();
        }
    }
}

/// n번째 최근 항목(1-기반)을 반환. 없으면 None.
fn slot(n: usize) -> Option<String> {
    if n == 0 {
        return None;
    }
    HISTORY.lock().ok()?.get(n - 1).cloned()
}

/// 현재 히스토리 길이 (진단/테스트용).
pub fn len() -> usize {
    HISTORY.lock().map(|h| h.len()).unwrap_or(0)
}

/// 클립보드 감시 스레드 시작 — 폴링으로 새 텍스트를 수집한다.
/// `enabled`가 false면 아무것도 하지 않는다 (opt-in). 중복 호출은 무시.
pub fn spawn_watcher(enabled: bool, history_size: usize) {
    CAP.store(history_size.clamp(1, 1000), Ordering::Relaxed);
    if !enabled {
        tracing::info!("클립보드 히스토리 수집: 비활성 (clipboard.history_enabled=false)");
        return;
    }
    if WATCHER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut board = match arboard::Clipboard::new() {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("클립보드 감시 시작 실패: {e}");
                WATCHER_STARTED.store(false, Ordering::SeqCst);
                return;
            }
        };
        // changeCount(macOS)로 실제 변화만 감지한다 — get_text를 매 폴링마다
        // 부르면 다른 클립보드 관리자와 경합하고 비용도 크다. 비-macOS는
        // change_count가 항상 0이라 get_text diff로 자연 폴백한다.
        let mut last_change = macos::change_count();
        let mut last_text = String::new();
        if !macos::is_concealed() {
            if let Ok(t) = board.get_text() {
                record(t.clone()); // 시작 시점 클립보드는 "직전 항목"
                last_text = t;
            }
        }
        tracing::info!(
            "클립보드 히스토리 수집 시작 (상한 {})",
            CAP.load(Ordering::Relaxed)
        );

        loop {
            std::thread::sleep(POLL_INTERVAL);
            // 우리가 붙여넣기 위해 클립보드를 스왑하는 동안엔 수집을 멈춘다
            // (스왑한 슬롯 내용이 "새 복사"로 잡혀 순서가 흐트러지는 것 방지).
            if SUPPRESS_CAPTURE.load(Ordering::Relaxed) {
                continue;
            }
            // macOS: changeCount가 그대로면 변화 없음 — get_text 생략.
            // 비-macOS: change_count가 0 고정이라 이 가드를 통과해 diff로 판정.
            let cc = macos::change_count();
            if cc != 0 && cc == last_change {
                continue;
            }
            last_change = cc;
            // 비밀번호 관리자가 Concealed로 표시한 항목은 수집하지 않는다.
            if macos::is_concealed() {
                continue;
            }
            if let Ok(cur) = board.get_text() {
                if cur != last_text && !cur.is_empty() {
                    record(cur.clone());
                    last_text = cur;
                }
            }
        }
    });
}

/// 붙여넣기 주입 동안 수집을 멈추는 플래그 (슬롯 스왑이 새 복사로 잡히지 않게).
static SUPPRESS_CAPTURE: AtomicBool = AtomicBool::new(false);

/// 런처를 열기 직전의 전경 앱 PID (docs/12 흐름 B의 붙여넣기 대상).
/// 0 = 없음. 런처가 포커스를 뺏기 전에 데몬이 스냅샷한다.
static PREV_APP_PID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// 런처(kmd-desktop)를 띄우기 직전에 호출 — 현재 전경 앱을 기억한다.
/// Launch 액션 경로에서 불러, 전역 핫키/레이어 어느 쪽으로 열든 캡처된다.
pub fn capture_foreground_app() {
    let pid = macos::frontmost_pid();
    if pid > 0 {
        PREV_APP_PID.store(pid, Ordering::Relaxed);
    }
}

/// 히스토리를 쿼리로 필터링해 돌려준다 (흐름 B 런처 검색).
/// 빈 쿼리는 전체(최신순). 대소문자 무시 부분일치 — 데몬은 후보만 넘기고
/// 정교한 랭킹은 UI(Nucleo)가 한다. 슬롯 번호는 붙여넣기 시 그대로 쓴다.
pub fn search(query: &str, limit: usize) -> Vec<kmd_core::ipc::ClipHit> {
    let q = query.trim().to_lowercase();
    let Ok(h) = HISTORY.lock() else {
        return Vec::new();
    };
    h.iter()
        .enumerate()
        .filter(|(_, text)| q.is_empty() || text.to_lowercase().contains(&q))
        .take(limit)
        .map(|(i, text)| kmd_core::ipc::ClipHit {
            slot: i + 1,
            preview: preview_line(text),
        })
        .collect()
}

/// 첫 줄 + 길이 제한 미리보기 (UI 표시용, 내용 로그엔 안 남김).
fn preview_line(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.chars().count() > 200 {
        first.chars().take(200).collect::<String>() + "…"
    } else {
        first.to_string()
    }
}

/// n번째 최근 항목을 **현재 전경 앱**에 붙여넣는다 (docs/12 흐름 A — 레이어).
/// 포커스가 이미 대상 앱에 있으므로 앱 전환 없이 바로 주입한다.
pub fn paste_slot(n: usize) {
    paste_impl(n, false);
}

/// n번째 항목을 **런처 열기 전 앱**으로 포커스를 되돌린 뒤 붙여넣는다
/// (docs/12 흐름 B — 런처 검색 결과 선택). capture_foreground_app으로
/// 기억해 둔 PID를 활성화한다.
pub fn paste_slot_to_previous(n: usize) {
    paste_impl(n, true);
}

/// 슬롯 붙여넣기 공통 구현.
/// `to_previous`면 먼저 이전 전경 앱으로 포커스를 되돌린다.
///
/// 1) (선택) 이전 앱 활성화 → 2) 현재 클립보드 저장 → 3) 슬롯 내용으로 교체 →
/// 4) Cmd+V/Ctrl+V 주입 → 5) 원래 클립보드 복원. 스왑 구간엔 수집을 멈춘다.
///
/// **액션 워커/커넥션 스레드에서 호출** — RESTORE_DELAY 만큼 블로킹한다.
fn paste_impl(n: usize, to_previous: bool) {
    let Some(content) = slot(n) else {
        tracing::warn!("클립보드 슬롯 {n} 비어 있음 — 붙여넣기 생략");
        return;
    };

    if to_previous {
        let pid = PREV_APP_PID.load(Ordering::Relaxed);
        if pid > 0 && macos::activate_pid(pid) {
            // 앱 전환이 반영될 시간을 준다 (포커스가 도착해야 Cmd+V가 먹는다).
            std::thread::sleep(Duration::from_millis(120));
        } else {
            tracing::warn!("이전 전경 앱 활성화 실패 (pid={pid}) — 현재 포커스에 붙여넣기");
        }
    }

    let mut board = match arboard::Clipboard::new() {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("클립보드 접근 실패: {e}");
            return;
        }
    };
    let saved = board.get_text().ok();

    SUPPRESS_CAPTURE.store(true, Ordering::Relaxed);
    if let Err(e) = board.set_text(content) {
        tracing::warn!("클립보드 세팅 실패: {e}");
        SUPPRESS_CAPTURE.store(false, Ordering::Relaxed);
        return;
    }
    // 세팅이 시스템에 반영될 짧은 시간을 준 뒤 주입한다.
    std::thread::sleep(Duration::from_millis(30));
    crate::keybind::inject_paste();

    std::thread::sleep(RESTORE_DELAY);
    if let Some(prev) = saved {
        let _ = board.set_text(prev);
    }
    SUPPRESS_CAPTURE.store(false, Ordering::Relaxed);
}

// ── macOS NSPasteboard ──────────────────────────────────────────────────────
//
// changeCount: 클립보드가 바뀔 때마다 증가하는 정수 — 실제 변화만 싸게 감지.
// Concealed: `org.nspasteboard.ConcealedType` 마크(비번 관리자 관례)가 있으면
// 민감 항목이므로 수집하지 않는다. NSPasteboard는 스레드 제약이 없어 감시
// 스레드에서 직접 호출해도 된다.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    /// `[[NSPasteboard generalPasteboard] changeCount]`. 실패 시 0.
    pub fn change_count() -> isize {
        unsafe {
            let Some(cls) = AnyClass::get(c"NSPasteboard") else {
                return 0;
            };
            let pb: *mut AnyObject = msg_send![cls, generalPasteboard];
            if pb.is_null() {
                return 0;
            }
            msg_send![pb, changeCount]
        }
    }

    /// 현재 클립보드에 Concealed(비밀번호) 마크가 있으면 true.
    /// `[pb availableTypeFromArray:@[@"org.nspasteboard.ConcealedType"]]` != nil.
    pub fn is_concealed() -> bool {
        unsafe {
            let (Some(pb_cls), Some(str_cls), Some(arr_cls)) = (
                AnyClass::get(c"NSPasteboard"),
                AnyClass::get(c"NSString"),
                AnyClass::get(c"NSArray"),
            ) else {
                return false;
            };
            let pb: *mut AnyObject = msg_send![pb_cls, generalPasteboard];
            if pb.is_null() {
                return false;
            }
            let ty: *mut AnyObject = msg_send![str_cls, stringWithUTF8String: c"org.nspasteboard.ConcealedType".as_ptr()];
            if ty.is_null() {
                return false;
            }
            let arr: *mut AnyObject = msg_send![arr_cls, arrayWithObject: ty];
            if arr.is_null() {
                return false;
            }
            let found: *mut AnyObject = msg_send![pb, availableTypeFromArray: arr];
            !found.is_null()
        }
    }

    /// 현재 전경 앱의 PID. `[[NSWorkspace sharedWorkspace] frontmostApplication]`.
    /// 없으면 0. (docs/12 흐름 B — 런처가 포커스를 뺏기 전에 캡처)
    pub fn frontmost_pid() -> i32 {
        unsafe {
            let Some(ws_cls) = AnyClass::get(c"NSWorkspace") else {
                return 0;
            };
            let ws: *mut AnyObject = msg_send![ws_cls, sharedWorkspace];
            if ws.is_null() {
                return 0;
            }
            let app: *mut AnyObject = msg_send![ws, frontmostApplication];
            if app.is_null() {
                return 0;
            }
            msg_send![app, processIdentifier]
        }
    }

    /// PID로 앱을 전면 활성화. 성공하면 true.
    /// `[[NSRunningApplication runningApplicationWithProcessIdentifier:] activateWithOptions:]`
    pub fn activate_pid(pid: i32) -> bool {
        // NSApplicationActivateIgnoringOtherApps
        const ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
        unsafe {
            let Some(cls) = AnyClass::get(c"NSRunningApplication") else {
                return false;
            };
            let app: *mut AnyObject = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
            if app.is_null() {
                return false;
            }
            let ok: bool = msg_send![app, activateWithOptions: ACTIVATE_IGNORING_OTHER_APPS];
            ok
        }
    }
}

// 비-macOS: changeCount 개념이 없어 0 고정(감시 루프가 get_text diff로 폴백),
// Concealed 판정도 아직 없음(추후 Windows CF_CLIPBOARD_VIEWER_IGNORE 등).
#[cfg(not(target_os = "macos"))]
mod macos {
    pub fn change_count() -> isize {
        0
    }
    pub fn is_concealed() -> bool {
        false
    }
    pub fn frontmost_pid() -> i32 {
        0
    }
    pub fn activate_pid(_pid: i32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 전역 HISTORY를 공유하므로 테스트를 직렬화한다 (병렬 실행 시 상호 오염 방지).
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        HISTORY.lock().unwrap().clear();
        CAP.store(50, Ordering::Relaxed);
        guard
    }

    #[test]
    fn record_최신이_앞으로_그리고_중복_이동() {
        let _g = reset();
        record("a".into());
        record("b".into());
        record("c".into());
        assert_eq!(slot(1).as_deref(), Some("c")); // 최신
        assert_eq!(slot(2).as_deref(), Some("b"));
        assert_eq!(slot(3).as_deref(), Some("a"));

        // "a" 재복사 → 중복 생성 없이 맨 앞으로
        record("a".into());
        assert_eq!(slot(1).as_deref(), Some("a"));
        assert_eq!(slot(2).as_deref(), Some("c"));
        assert_eq!(len(), 3, "중복은 새 항목을 만들지 않는다");
    }

    #[test]
    fn 상한_초과시_오래된것부터_밀린다() {
        let _g = reset();
        CAP.store(3, Ordering::Relaxed);
        for s in ["1", "2", "3", "4"] {
            record(s.into());
        }
        assert_eq!(len(), 3);
        assert_eq!(slot(1).as_deref(), Some("4"));
        assert_eq!(slot(3).as_deref(), Some("2"));
        assert!(slot(4).is_none(), "1은 밀려남");
    }

    #[test]
    fn 빈_문자열과_초과크기는_수집하지_않는다() {
        let _g = reset();
        record(String::new());
        record("x".repeat(MAX_ITEM_BYTES + 1));
        assert_eq!(len(), 0);
    }

    #[test]
    fn 빈_슬롯은_none() {
        let _g = reset();
        assert!(slot(0).is_none());
        assert!(slot(1).is_none());
    }
}
