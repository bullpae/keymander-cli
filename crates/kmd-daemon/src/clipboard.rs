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
//! - **비밀번호 제외**: macOS는 `org.nspasteboard.ConcealedType` 마크(1Password
//!   등 비번 관리자 관례), Windows는 `ExcludeClipboardContentFromMonitorProcessing`
//!   등 수집 제외 마커가 붙은 항목을 수집하지 않는다 (P1.1).
//!
//! 항목 식별: 슬롯 번호(1-기반, 최신순)는 새 복사가 들어올 때마다 밀리므로
//! 검색→선택 사이에 다른 항목을 가리킬 수 있다. 각 항목은 데몬 수명 동안
//! 변하지 않는 안정 ID를 갖고, 런처 선택은 ID로 붙여넣는다 (`paste_item`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// 수집 상한 (텍스트 1건). 초과 시 수집하지 않는다.
const MAX_ITEM_BYTES: usize = 1024 * 1024;
/// 감시 폴링 주기. macOS changeCount / Windows sequence number만 읽는 값싼
/// 폴링이라 유휴 비용은 무시 가능한 수준 — 클립보드는 변화가 있을 때만 연다.
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// 붙여넣기 주입 후 원래 클립보드를 복원하기까지의 지연.
/// 대상 앱이 Cmd+V를 처리할 시간을 준다 (클립보드 관리자 표준 기법).
const RESTORE_DELAY: Duration = Duration::from_millis(300);

/// 히스토리 항목. `id`는 데몬 수명 동안 안정적 — 같은 내용을 다시 복사해
/// 맨 앞으로 이동해도 유지된다 (런처가 선택 시점의 항목을 정확히 집는 근거).
#[derive(Debug, Clone)]
struct ClipEntry {
    id: u64,
    text: String,
}

/// 히스토리 링 버퍼. 맨 앞(index 0)이 최신.
static HISTORY: Mutex<VecDeque<ClipEntry>> = Mutex::new(VecDeque::new());
/// 안정 ID 발급기. 0은 "ID 없음"(구버전 프로토콜 폴백)으로 예약.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// 붙여넣기/복원 직렬화 — 두 요청이 스냅샷·복원을 교차해 클립보드를
/// 손상하지 않게 한다 (IPC 커넥션 스레드가 여러 개다).
static PASTE_LOCK: Mutex<()> = Mutex::new(());
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
        // 같은 내용은 중복 생성 없이 맨 앞으로 이동 (dedupe) — ID는 유지한다.
        let entry = if let Some(pos) = h.iter().position(|e| e.text == text) {
            h.remove(pos).expect("position으로 찾은 항목")
        } else {
            ClipEntry {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                text,
            }
        };
        h.push_front(entry);
        while h.len() > cap {
            h.pop_back();
        }
    }
}

/// 안정 ID로 항목 조회. 밀려났거나 모르는 ID면 None.
fn entry_by_id(id: u64) -> Option<ClipEntry> {
    HISTORY.lock().ok()?.iter().find(|e| e.id == id).cloned()
}

/// n번째 최근 항목(1-기반) 조회. 없으면 None.
fn entry_by_slot(n: usize) -> Option<ClipEntry> {
    if n == 0 {
        return None;
    }
    HISTORY.lock().ok()?.get(n - 1).cloned()
}

/// n번째 최근 항목 텍스트 (테스트용).
#[cfg(test)]
fn slot(n: usize) -> Option<String> {
    entry_by_slot(n).map(|e| e.text)
}

/// n번째 최근 항목의 안정 ID (테스트용).
#[cfg(test)]
fn slot_id(n: usize) -> Option<u64> {
    entry_by_slot(n).map(|e| e.id)
}

/// 현재 히스토리 길이 (진단/테스트용).
pub fn len() -> usize {
    HISTORY.lock().map(|h| h.len()).unwrap_or(0)
}

/// 클립보드 감시 스레드 시작 — 변화가 있을 때만 읽어 수집한다.
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
        // 세대 번호(macOS changeCount / Windows GetClipboardSequenceNumber)로
        // 실제 변화만 감지한다 — get_text를 매 폴링마다 부르면 클립보드를 계속
        // 열어 다른 클립보드 관리자·복사 중인 앱과 경합하고 비용도 크다.
        // 세대 번호가 없는 플랫폼(change_count=0)은 get_text diff로 폴백한다.
        let mut last_change = platform::change_count();
        let mut last_text = String::new();
        if !platform::is_concealed() {
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
            // 세대 번호가 그대로면 변화 없음 — 클립보드를 열지 않는다.
            // 세대 번호 미지원(0)이면 이 가드를 통과해 diff로 판정.
            let cc = platform::change_count();
            if cc != 0 && cc == last_change {
                continue;
            }
            last_change = cc;
            // 비밀번호 관리자 등이 수집 제외로 표시한 항목은 수집하지 않는다.
            if platform::is_concealed() {
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

/// 런처를 열기 직전의 전경 앱 대상 (macOS PID / Windows HWND, docs/12 흐름 B).
/// 0 = 없음. 런처가 포커스를 뺏기 전에 데몬이 스냅샷한다.
static PREV_APP_TARGET: AtomicIsize = AtomicIsize::new(0);

/// 런처(kmd-desktop)를 띄우기 직전에 호출 — 현재 전경 앱을 기억한다.
/// Launch 액션 경로에서 불러, 전역 핫키/레이어 어느 쪽으로 열든 캡처된다.
pub fn capture_foreground_app() {
    let target = platform::frontmost_target();
    if target != 0 {
        PREV_APP_TARGET.store(target, Ordering::Relaxed);
    }
}

/// 히스토리를 쿼리로 필터링해 돌려준다 (흐름 B 런처 검색).
/// 빈 쿼리는 전체(최신순). 대소문자 무시 부분일치 — 데몬은 후보만 넘기고
/// 정교한 랭킹은 UI(Nucleo)가 한다. 실행은 안정 ID(`ClipHit.id`)로 한다.
pub fn search(query: &str, limit: usize) -> Vec<kmd_core::ipc::ClipHit> {
    let q = query.trim().to_lowercase();
    let Ok(h) = HISTORY.lock() else {
        return Vec::new();
    };
    h.iter()
        .enumerate()
        .filter(|(_, entry)| q.is_empty() || entry.text.to_lowercase().contains(&q))
        .take(limit)
        .map(|(i, entry)| kmd_core::ipc::ClipHit {
            id: entry.id,
            slot: i + 1,
            preview: preview_line(&entry.text),
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
/// 키 훅 경로 전용 — 실패는 로그로만 남긴다 (IPC 경로는 checked 계열 사용).
pub fn paste_slot(n: usize) {
    if let Err(e) = paste_slot_checked(n, false) {
        tracing::warn!("클립보드 슬롯 {n} 붙여넣기 실패: {e}");
    }
}

/// n번째 최근 항목(1-기반)을 붙여넣는다. `to_previous`면 런처 열기 전 앱으로
/// 포커스를 되돌린 뒤 주입한다 (docs/12 흐름 B). 빈 슬롯/실패는 Err.
pub fn paste_slot_checked(n: usize, to_previous: bool) -> Result<(), String> {
    let entry = entry_by_slot(n).ok_or_else(|| format!("클립보드 슬롯 {n}이 비어 있습니다"))?;
    paste_impl(entry, to_previous)
}

/// 안정 ID로 항목을 붙여넣는다 (런처 검색 결과 선택 경로). 검색 이후 새 복사가
/// 들어와 슬롯이 밀려도 같은 항목을 집는다. 만료/실패는 Err.
pub fn paste_item(id: u64, to_previous: bool) -> Result<(), String> {
    let entry =
        entry_by_id(id).ok_or_else(|| "선택한 클립보드 항목이 만료되었습니다".to_string())?;
    paste_impl(entry, to_previous)
}

/// 복원 판단: 우리가 넣은 임시 값이 **여전히 현재 클립보드일 때만** 복원한다.
/// 그 사이 사용자/다른 앱이 복사했다면 덮어쓰지 않는다 (사용자 데이터 보호).
/// 세대 번호가 있으면(swap≠0) 세대 비교, 없으면 텍스트 일치로 판정한다.
fn should_restore(
    swap_generation: isize,
    current_generation: isize,
    injected_text_matches: bool,
) -> bool {
    if swap_generation != 0 {
        current_generation == swap_generation
    } else {
        injected_text_matches
    }
}

/// 슬롯/ID 붙여넣기 공통 구현.
/// `to_previous`면 먼저 이전 전경 앱으로 포커스를 되돌린다.
///
/// 1) (선택) 이전 앱 활성화 → 2) 현재 클립보드 스냅샷(실패 시 **중단** — 원본을
/// 파괴하지 않는다) → 3) 항목 텍스트로 교체 → 4) Cmd+V/Ctrl+V 주입 →
/// 5) 우리 값이 아직 현재일 때만 복원. 스왑 구간엔 수집을 멈춘다.
///
/// **액션 워커/커넥션 스레드에서 호출** — RESTORE_DELAY 만큼 블로킹한다.
fn paste_impl(entry: ClipEntry, to_previous: bool) -> Result<(), String> {
    let _guard = PASTE_LOCK
        .lock()
        .map_err(|_| "클립보드 동작 잠금이 손상되었습니다".to_string())?;

    if to_previous {
        let target = PREV_APP_TARGET.load(Ordering::Relaxed);
        if target != 0 && platform::activate_target(target) {
            // 앱 전환이 반영될 시간을 준다 (포커스가 도착해야 Cmd+V가 먹는다).
            std::thread::sleep(Duration::from_millis(120));
        } else {
            tracing::warn!("이전 전경 앱 활성화 실패 (target={target}) — 현재 포커스에 붙여넣기");
        }
    }

    // 스냅샷 실패 = 원본을 손실 없이 복원할 수 없음 → 붙여넣기 자체를 포기한다.
    // (사용자의 클립보드가 그대로 남으므로 수동 Ctrl+V로 항상 복구 가능.)
    let saved = platform::snapshot().map_err(|e| format!("클립보드 백업 실패로 중단: {e}"))?;
    let mut board = arboard::Clipboard::new().map_err(|e| format!("클립보드 접근 실패: {e}"))?;

    SUPPRESS_CAPTURE.store(true, Ordering::Relaxed);
    let result = paste_swap(&mut board, &entry, saved);
    SUPPRESS_CAPTURE.store(false, Ordering::Relaxed);
    result
}

/// 스왑→주입→조건부 복원. SUPPRESS_CAPTURE 해제를 위해 paste_impl에서 분리.
fn paste_swap(
    board: &mut arboard::Clipboard,
    entry: &ClipEntry,
    saved: platform::Snapshot,
) -> Result<(), String> {
    board
        .set_text(&entry.text)
        .map_err(|e| format!("클립보드 세팅 실패: {e}"))?;
    // 우리 스왑 직후의 세대 번호 — 복원 시점에 이 값이면 "그 사이 아무도 안 씀".
    let swap_generation = platform::change_count();
    // 세팅이 시스템에 반영될 짧은 시간을 준 뒤 주입한다.
    std::thread::sleep(Duration::from_millis(30));
    crate::keybind::inject_paste();

    std::thread::sleep(RESTORE_DELAY);
    let injected_text_matches =
        swap_generation == 0 && board.get_text().ok().as_deref() == Some(entry.text.as_str());
    if should_restore(
        swap_generation,
        platform::change_count(),
        injected_text_matches,
    ) {
        platform::restore(saved).map_err(|e| format!("기존 클립보드 복원 실패: {e}"))?;
    } else {
        tracing::info!("붙여넣기 후 클립보드가 이미 바뀜 — 복원 생략 (새 복사 보존)");
    }
    Ok(())
}

// ── macOS NSPasteboard ──────────────────────────────────────────────────────
//
// changeCount: 클립보드가 바뀔 때마다 증가하는 정수 — 실제 변화만 싸게 감지.
// Concealed: `org.nspasteboard.ConcealedType` 마크(비번 관리자 관례)가 있으면
// 민감 항목이므로 수집하지 않는다. NSPasteboard는 스레드 제약이 없어 감시
// 스레드에서 직접 호출해도 된다.
#[cfg(target_os = "macos")]
mod platform {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    /// 텍스트 스냅샷 (기존 macOS 계약 유지 — 비텍스트 복원은 docs/12 후속).
    #[derive(Debug)]
    pub struct Snapshot {
        text: Option<String>,
    }

    /// 현재 클립보드 백업. 텍스트가 아니면 None 스냅샷 (복원 시 no-op).
    pub fn snapshot() -> Result<Snapshot, String> {
        let mut board = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        Ok(Snapshot {
            text: board.get_text().ok(),
        })
    }

    /// 백업 복원. None 스냅샷은 아무것도 하지 않는다 (기존 동작과 동일).
    pub fn restore(snapshot: Snapshot) -> Result<(), String> {
        if let Some(text) = snapshot.text {
            let mut board = arboard::Clipboard::new().map_err(|e| e.to_string())?;
            board.set_text(text).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

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
    pub fn frontmost_target() -> isize {
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
            let pid: i32 = msg_send![app, processIdentifier];
            pid as isize
        }
    }

    /// PID로 앱을 전면 활성화. 성공하면 true.
    /// `[[NSRunningApplication runningApplicationWithProcessIdentifier:] activateWithOptions:]`
    pub fn activate_target(target: isize) -> bool {
        // NSApplicationActivateIgnoringOtherApps
        const ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
        unsafe {
            let Some(cls) = AnyClass::get(c"NSRunningApplication") else {
                return false;
            };
            let app: *mut AnyObject =
                msg_send![cls, runningApplicationWithProcessIdentifier: target as i32];
            if app.is_null() {
                return false;
            }
            let ok: bool = msg_send![app, activateWithOptions: ACTIVATE_IGNORING_OTHER_APPS];
            ok
        }
    }
}

// ── Windows 클립보드 ────────────────────────────────────────────────────────
//
// 감시는 GetClipboardSequenceNumber(클립보드를 열지 않는 세대 번호)로 변화를
// 감지하고, 변화가 있을 때만 연다. 붙여넣기 전 백업은 HGLOBAL 기반 포맷 전부를
// 상한(crate::winclip) 안에서 바이트 복사한다 — 텍스트/DIB 이미지/파일 목록
// (CF_HDROP)/커스텀 등록 포맷 보존. GDI 핸들 포맷은 DIB가 있으면 재합성에
// 맡기고, 그마저 없으면 붙여넣기를 중단해 원본을 지킨다 (fail-safe).
#[cfg(windows)]
mod platform {
    use crate::winclip;
    use clipboard_win::{raw, Clipboard, EnumFormats};

    /// 전체 포맷 바이트 스냅샷.
    #[derive(Debug)]
    pub struct Snapshot {
        formats: Vec<(u32, Vec<u8>)>,
    }

    /// GetClipboardSequenceNumber — 클립보드를 열지 않고 읽는 세대 번호.
    /// 권한 없는 세션 등 실패 시 0 (호출부가 텍스트 diff로 폴백).
    pub fn change_count() -> isize {
        raw::seq_num().map_or(0, |n| n.get() as isize)
    }

    /// 수집 제외 마커 판정. 존재 확인은 클립보드를 열지 않고 하고
    /// (IsClipboardFormatAvailable), 값 확인이 필요할 때만 연다.
    /// 열기/읽기 실패 시 **보수적으로 제외**(true) — 민감할 수 있는 항목을
    /// 불확실한 상태에서 수집하지 않는다.
    pub fn is_concealed() -> bool {
        if let Some(f) = clipboard_win::register_format(winclip::MARKER_EXCLUDE) {
            if clipboard_win::is_format_avail(f.get()) {
                return true;
            }
        }
        let value_markers: Vec<u32> = winclip::MARKERS_OPT_OUT_ZERO
            .iter()
            .filter_map(|name| clipboard_win::register_format(name))
            .map(|f| f.get())
            .filter(|f| clipboard_win::is_format_avail(*f))
            .collect();
        if value_markers.is_empty() {
            return false;
        }
        let Ok(_clip) = Clipboard::new_attempts(5) else {
            return true;
        };
        for format in value_markers {
            let mut data = Vec::new();
            match raw::get_vec(format, &mut data) {
                Ok(_) if data.len() >= 4 && !winclip::opt_out_value_is_exclusion(&data) => {}
                // 값이 0(제외) 또는 못 읽음/형식 이상 → 보수적으로 제외
                _ => return true,
            }
        }
        false
    }

    /// 현재 전경 창 HWND (docs/12 흐름 B — 런처가 포커스를 뺏기 전에 캡처).
    pub fn frontmost_target() -> isize {
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() as isize }
    }

    /// HWND로 창을 전면 활성화. 성공하면 true.
    pub fn activate_target(target: isize) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let hwnd = target as windows_sys::Win32::Foundation::HWND;
        unsafe {
            if hwnd.is_null() || IsWindow(hwnd) == 0 {
                return false;
            }
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd) != 0
        }
    }

    /// 현재 클립보드의 모든 보존 가능 포맷을 상한 안에서 백업한다.
    /// 상한 초과·복원 불가 포맷·읽기 실패는 Err — 호출부가 붙여넣기를 중단한다.
    pub fn snapshot() -> Result<Snapshot, String> {
        let _clip = Clipboard::new_attempts(10).map_err(|e| format!("클립보드 열기 실패: {e}"))?;
        let mut probes = Vec::new();
        for format in EnumFormats::new() {
            if winclip::is_gdi_or_owner_format(format) {
                probes.push(winclip::FormatProbe::Unreadable(format));
                continue;
            }
            // 크기를 먼저 관측해 상한 검사를 통과한 포맷만 실제로 복사한다
            // (거대 데이터를 읽어놓고 버리는 일이 없게).
            match raw::size(format) {
                Some(size) => probes.push(winclip::FormatProbe::Readable(format, size.get())),
                None => probes.push(winclip::FormatProbe::Unreadable(format)),
            }
        }
        let plan = winclip::plan_snapshot(&probes)?;
        let mut formats = Vec::with_capacity(plan.len());
        for format in plan {
            let mut data = Vec::new();
            raw::get_vec(format, &mut data)
                .map_err(|e| format!("클립보드 포맷 {format} 읽기 실패: {e}"))?;
            formats.push((format, data));
        }
        Ok(Snapshot { formats })
    }

    /// 백업을 그대로 되돌린다. 빈 스냅샷은 빈 클립보드로 복원.
    pub fn restore(snapshot: Snapshot) -> Result<(), String> {
        let _clip = Clipboard::new_attempts(10).map_err(|e| format!("클립보드 열기 실패: {e}"))?;
        raw::empty().map_err(|e| format!("클립보드 비우기 실패: {e}"))?;
        for (format, data) in snapshot.formats {
            raw::set_without_clear(format, &data)
                .map_err(|e| format!("클립보드 포맷 {format} 복원 실패: {e}"))?;
        }
        Ok(())
    }
}

// 기타 플랫폼: 세대 번호 없음(0 → 감시는 get_text diff, 복원 판정은 텍스트
// 비교 폴백), Concealed 판정 없음, 전경창 제어 없음. 백업은 텍스트만.
#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    #[derive(Debug)]
    pub struct Snapshot {
        text: Option<String>,
    }

    pub fn snapshot() -> Result<Snapshot, String> {
        let mut board = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        Ok(Snapshot {
            text: board.get_text().ok(),
        })
    }

    pub fn restore(snapshot: Snapshot) -> Result<(), String> {
        if let Some(text) = snapshot.text {
            let mut board = arboard::Clipboard::new().map_err(|e| e.to_string())?;
            board.set_text(text).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn change_count() -> isize {
        0
    }
    pub fn is_concealed() -> bool {
        false
    }
    pub fn frontmost_target() -> isize {
        0
    }
    pub fn activate_target(_target: isize) -> bool {
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

        // "a" 재복사 → 중복 생성 없이 안정 ID를 유지한 채 맨 앞으로
        let a_id = slot_id(3).unwrap();
        record("a".into());
        assert_eq!(slot(1).as_deref(), Some("a"));
        assert_eq!(slot_id(1), Some(a_id), "dedupe 이동에도 ID 유지");
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

    #[test]
    fn 안정_id는_새복사로_슬롯이_밀려도_같은_항목을_가리킨다() {
        let _g = reset();
        record("first".into());
        record("selected account number".into());
        let id = slot_id(1).unwrap();

        // 검색→선택 사이에 새 복사가 들어와 슬롯이 밀리는 상황
        record("background clipboard update".into());
        assert_eq!(slot(1).as_deref(), Some("background clipboard update"));
        assert_eq!(
            entry_by_id(id).unwrap().text,
            "selected account number",
            "슬롯이 밀려도 ID는 같은 항목"
        );
    }

    #[test]
    fn 검색_결과에_안정_id가_실린다() {
        let _g = reset();
        record("alpha".into());
        record("beta".into());
        let hits = search("", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].slot, 1);
        assert_eq!(entry_by_id(hits[0].id).unwrap().text, "beta");
        assert_eq!(entry_by_id(hits[1].id).unwrap().text, "alpha");
    }

    #[test]
    fn 빈_슬롯과_만료된_id_붙여넣기는_에러다() {
        let _g = reset();
        // 항목 조회가 클립보드 접근보다 먼저 실패하므로 실제 클립보드를 건드리지 않는다.
        assert!(paste_slot_checked(1, false).is_err());
        assert!(paste_item(u64::MAX, false).is_err());
    }

    #[test]
    fn 복원은_우리가_넣은_값이_현재일_때만_한다() {
        // 세대 번호 지원: 세대가 그대로면 복원, 바뀌었으면(외부 복사) 생략
        assert!(should_restore(7, 7, false));
        assert!(!should_restore(7, 8, true), "외부 복사 발생 → 복원 생략");
        // 세대 번호 미지원(0): 텍스트 일치로 판정
        assert!(should_restore(0, 0, true));
        assert!(!should_restore(0, 0, false));
    }
}
