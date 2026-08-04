//! 시스템 아이콘 매핑 — 이모지 문자열 → 테마 틴트 SVG Handle
//!
//! kmd-desktop 전용. kmd-core가 만드는 `IndexItem.icon`(이모지 문자열)을
//! Lucide SVG(ISC 라이선스)로 오버라이드한다. 매칭 실패 시 `None` →
//! 기존 이모지 텍스트 fallback이므로 kmd-core/TUI는 무수정.
//!
//! SVG는 `stroke="currentColor"`이며 렌더 시점에 `svg::Style { color }`로
//! 테마 시맨틱 컬러가 입혀진다. 테마가 바뀌면 아이콘 색도 자동 추종.
//!
//! **캐싱**: brand_icons와 동일하게 `LazyLock`으로 Handle을 프로세스 내
//! 1회만 생성하여 iced의 벡터 캐시가 정상 작동한다.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::svg::Handle;
use iced::Color;
use kmd_core::ItemKind;

use crate::theme::DesktopTheme;

// ── 아이콘 컬러 역할 ─────────────────────────────────────────────────────────

/// 아이콘이 사용할 테마 시맨틱 컬러 슬롯.
/// 고정 hex 대신 역할로 정의해 5개 테마 전부에서 팔레트 안에 머문다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconRole {
    Accent,
    Teal,
    Green,
    Yellow,
    Peach,
    Red,
    Subtext,
}

impl IconRole {
    /// 역할을 현재 테마의 실제 색으로 해석.
    pub fn color(self, t: &DesktopTheme) -> Color {
        match self {
            IconRole::Accent => t.accent,
            IconRole::Teal => t.teal,
            IconRole::Green => t.green,
            IconRole::Yellow => t.yellow,
            IconRole::Peach => t.peach,
            IconRole::Red => t.red,
            IconRole::Subtext => t.subtext,
        }
    }
}

// ── SVG 임베드 + Handle 캐시 ─────────────────────────────────────────────────

macro_rules! svg_entry {
    ($id:literal) => {
        (
            $id,
            include_bytes!(concat!("../assets/svg/", $id, ".svg")).as_slice(),
        )
    };
}

static HANDLE_CACHE: LazyLock<HashMap<&'static str, Handle>> = LazyLock::new(|| {
    let entries: &[(&str, &[u8])] = &[
        svg_entry!("arrow-left"),
        svg_entry!("arrow-right"),
        svg_entry!("arrow-right-to-line"),
        svg_entry!("arrows-up-down"),
        svg_entry!("book-open"),
        svg_entry!("bot"),
        svg_entry!("braces"),
        svg_entry!("brain"),
        svg_entry!("calculator"),
        svg_entry!("cat"),
        svg_entry!("chart-column"),
        svg_entry!("check"),
        svg_entry!("circle"),
        svg_entry!("circle-help"),
        svg_entry!("corner-down-left"),
        svg_entry!("database"),
        svg_entry!("earth"),
        svg_entry!("eye"),
        svg_entry!("file"),
        svg_entry!("file-code"),
        svg_entry!("file-text"),
        svg_entry!("film"),
        svg_entry!("flask-conical"),
        svg_entry!("folder"),
        svg_entry!("folder-search"),
        svg_entry!("gamepad-2"),
        svg_entry!("globe"),
        svg_entry!("hand"),
        svg_entry!("image"),
        svg_entry!("info"),
        svg_entry!("keyboard"),
        svg_entry!("keyboard-music"),
        svg_entry!("languages"),
        svg_entry!("link"),
        svg_entry!("lock"),
        svg_entry!("log-out"),
        svg_entry!("map"),
        svg_entry!("message-circle"),
        svg_entry!("monitor"),
        svg_entry!("moon"),
        svg_entry!("music"),
        svg_entry!("octagon-x"),
        svg_entry!("package"),
        svg_entry!("pen-line"),
        svg_entry!("pin"),
        svg_entry!("play"),
        svg_entry!("plus"),
        svg_entry!("power"),
        svg_entry!("refresh-cw"),
        svg_entry!("rocket"),
        svg_entry!("save"),
        svg_entry!("search"),
        svg_entry!("settings"),
        svg_entry!("smartphone"),
        svg_entry!("smile"),
        svg_entry!("sparkles"),
        svg_entry!("square"),
        svg_entry!("terminal"),
        svg_entry!("timer"),
        svg_entry!("trash-2"),
        svg_entry!("triangle-alert"),
        svg_entry!("type"),
        svg_entry!("user"),
        svg_entry!("wind"),
        svg_entry!("x"),
        svg_entry!("zap"),
    ];
    entries
        .iter()
        .map(|(id, bytes)| (*id, Handle::from_memory(*bytes)))
        .collect()
});

// ── 이모지 → (아이콘 ID, 컬러 역할) ──────────────────────────────────────────

/// kmd-core 전역에서 아이콘으로 쓰이는 이모지 문자열의 전수 매핑.
/// FE0F(variation selector) 유무가 다른 이모지는 별개 항목이다 —
/// 예: `🗺️`(keys 치트시트) vs `🗺`(maps 웹 폴백).
const EMOJI_MAP: &[(&str, &str, IconRole)] = &[
    // ── 시스템 명령 (system_commands.rs) ──
    ("\u{23FB}", "power", IconRole::Red),         // ⏻ 전원
    ("\u{1F504}", "refresh-cw", IconRole::Peach), // 🔄 재시작/리로드
    ("\u{1F4A4}", "moon", IconRole::Yellow),      // 💤 절전
    ("\u{1F512}", "lock", IconRole::Red),         // 🔒 잠금
    ("\u{1F6AA}", "log-out", IconRole::Red),      // 🚪 로그아웃
    ("\u{2699}\u{FE0F}", "settings", IconRole::Subtext), // ⚙️ 설정
    ("\u{2699}", "settings", IconRole::Subtext),  // ⚙ (c/cpp 파일)
    ("\u{1F4CA}", "chart-column", IconRole::Green), // 📊 작업관리자/시트
    ("\u{1F5D1}", "trash-2", IconRole::Red),      // 🗑 휴지통
    // ── 프리픽스 명령 (query_prefix.rs) ──
    ("\u{1F310}", "globe", IconRole::Teal),       // 🌐 웹 검색
    ("\u{1F522}", "calculator", IconRole::Green), // 🔢 계산기
    ("\u{1F5A9}", "calculator", IconRole::Green), // 🖩 계산 결과
    ("\u{1F60A}", "smile", IconRole::Yellow),     // 😊 이모지 검색
    ("\u{26A1}", "zap", IconRole::Peach),         // ⚡ 변환/콤보
    ("\u{1F4DD}", "file-text", IconRole::Accent), // 📝 프롬프트/메모
    ("\u{1F4C2}", "folder-search", IconRole::Yellow), // 📂 폴더 검색
    ("\u{1F5FA}\u{FE0F}", "keyboard", IconRole::Accent), // 🗺️ 키 매핑 시트
    ("\u{2328}\u{FE0F}", "keyboard", IconRole::Accent), // ⌨️ 키맵
    ("\u{1F4E6}", "package", IconRole::Peach),    // 📦 패키지/버전
    ("\u{2753}", "circle-help", IconRole::Teal),  // ❓ 도움말
    ("\u{1F9E0}", "brain", IconRole::Accent),     // 🧠 LLM/코어
    ("\u{1F50D}", "search", IconRole::Teal),      // 🔍 검색
    ("\u{1F50E}", "search", IconRole::Teal),      // 🔎 멀티 웹 검색
    ("\u{270D}\u{FE0F}", "pen-line", IconRole::Green), // ✍️ 맞춤법
    ("\u{1F5E3}\u{FE0F}", "languages", IconRole::Teal), // 🗣️ 번역
    ("\u{1F4BB}", "terminal", IconRole::Green),   // 💻 셸
    ("\u{1F4DF}", "terminal", IconRole::Green),   // 📟 Run
    ("\u{1F4C4}", "file", IconRole::Subtext),     // 📄 파일/문서
    ("\u{1F9EA}", "flask-conical", IconRole::Green), // 🧪 테스트
    ("\u{1F5A5}\u{FE0F}", "monitor", IconRole::Subtext), // 🖥️ 시스템 정보
    // ── 폴더 검색 (folder_search.rs) ──
    ("\u{1F4C1}", "folder", IconRole::Yellow), // 📁 폴더
    ("\u{26A0}\u{FE0F}", "triangle-alert", IconRole::Yellow), // ⚠️ 경고
    // ── 키맵 치트시트 (keymap.rs) ──
    ("\u{25B6}\u{FE0F}", "play", IconRole::Green), // ▶️ 시작
    ("\u{23F9}\u{FE0F}", "square", IconRole::Red), // ⏹️ 정지
    ("\u{2705}", "check", IconRole::Green),        // ✅ 활성
    ("\u{26AA}", "circle", IconRole::Subtext),     // ⚪ 비활성
    ("\u{1F4CC}", "pin", IconRole::Accent),        // 📌 고정
    ("\u{2139}\u{FE0F}", "info", IconRole::Teal),  // ℹ️ 정보
    ("\u{274C}", "x", IconRole::Red),              // ❌ 닫기/오류
    ("\u{1F503}", "arrows-up-down", IconRole::Subtext), // 🔃 위아래 이동
    ("\u{23CE}", "corner-down-left", IconRole::Subtext), // ⏎ 엔터
    ("\u{21E5}", "arrow-right-to-line", IconRole::Subtext), // ⇥ 탭
    ("\u{2B05}\u{FE0F}", "arrow-left", IconRole::Subtext), // ⬅️ 뒤로
    ("\u{27A1}\u{FE0F}", "arrow-right", IconRole::Subtext), // ➡️ 액션
    ("\u{1F441}\u{FE0F}", "eye", IconRole::Teal),  // 👁️ 미리보기
    ("\u{1F6D1}", "octagon-x", IconRole::Red),     // 🛑 종료
    ("\u{1F3AE}", "gamepad-2", IconRole::Peach),   // 🎮 키맨더 모드
    ("\u{1F4F1}", "smartphone", IconRole::Teal),   // 📱 레이어
    ("\u{1F517}", "link", IconRole::Teal),         // 🔗 리맵
    ("\u{1F919}", "hand", IconRole::Yellow),       // 🤙 탭홀드
    ("\u{1F918}", "hand", IconRole::Yellow),       // 🤘 탭홀드 항목
    ("\u{1F91C}", "hand", IconRole::Yellow),       // 🤜 더블탭
    ("\u{1F44B}", "hand", IconRole::Yellow),       // 👋 탭
    ("\u{1F4A8}", "wind", IconRole::Peach),        // 💨 더블탭 항목
    ("\u{1F3B9}", "keyboard-music", IconRole::Accent), // 🎹 콤보
    ("\u{2795}", "plus", IconRole::Green),         // ➕ 추가
    // ── 셸 내장 명령 (builtin_shell.rs) ──
    ("\u{1F30D}", "earth", IconRole::Teal), // 🌍 네트워크
    ("\u{23F1}\u{FE0F}", "timer", IconRole::Yellow), // ⏱️ 업타임
    ("\u{1F4BE}", "save", IconRole::Green), // 💾 디스크
    ("\u{1F464}", "user", IconRole::Peach), // 👤 사용자
    // ── 웹 서비스 이모지 폴백 (services.rs — 브랜드 PNG 없는 경우) ──
    ("\u{1F431}", "cat", IconRole::Subtext),     // 🐱 GitHub
    ("\u{1F4DA}", "book-open", IconRole::Teal),  // 📚 문서
    ("\u{1F4D6}", "book-open", IconRole::Teal),  // 📖 사전
    ("\u{1F4D7}", "book-open", IconRole::Green), // 📗 사전(초록)
    ("\u{1F4D8}", "book-open", IconRole::Teal),  // 📘 hwp/사전
    ("\u{1F4D5}", "book-open", IconRole::Peach), // 📕 pdf
    ("\u{1F5FA}", "map", IconRole::Green),       // 🗺 지도
    ("\u{1F916}", "bot", IconRole::Teal),        // 🤖 ChatGPT 폴백
    ("\u{1F4AC}", "message-circle", IconRole::Teal), // 💬 채팅
    ("\u{2728}", "sparkles", IconRole::Peach),   // ✨ Claude 폴백
    ("\u{264A}", "sparkles", IconRole::Teal),    // ♊ Gemini 폴백
    ("\u{1F680}", "rocket", IconRole::Peach),    // 🚀 Grok 폴백
    // ── 파일 확장자 (files.rs ICON_TABLE) ──
    ("\u{1F980}", "file-code", IconRole::Accent), // 🦀 rust
    ("\u{1F40D}", "file-code", IconRole::Accent), // 🐍 python
    ("\u{1F4DC}", "file-code", IconRole::Accent), // 📜 js/ts
    ("\u{1F535}", "file-code", IconRole::Accent), // 🔵 go
    ("\u{2615}", "file-code", IconRole::Accent),  // ☕ java
    ("\u{1F7E3}", "file-code", IconRole::Accent), // 🟣 c#
    ("\u{1F41A}", "terminal", IconRole::Green),   // 🐚 셸 스크립트
    ("\u{1F4CB}", "braces", IconRole::Subtext),   // 📋 설정 파일
    ("\u{1F5C3}", "database", IconRole::Teal),    // 🗃 DB
    ("\u{1F5BC}", "image", IconRole::Peach),      // 🖼 이미지
    ("\u{1F3B5}", "music", IconRole::Yellow),     // 🎵 오디오
    ("\u{1F3AC}", "film", IconRole::Peach),       // 🎬 비디오
    ("\u{1F524}", "type", IconRole::Subtext),     // 🔤 폰트
];

static EMOJI_LOOKUP: LazyLock<HashMap<&'static str, (&'static str, IconRole)>> =
    LazyLock::new(|| {
        EMOJI_MAP
            .iter()
            .map(|(emoji, id, role)| (*emoji, (*id, *role)))
            .collect()
    });

// ── 공개 API ──────────────────────────────────────────────────────────────────

/// `IndexItem.icon`(이모지 문자열)에 대응하는 **캐시된** SVG Handle과 컬러 역할 반환.
///
/// - `ItemKind::Emoji`는 검색 결과 자체가 이모지이므로 매핑하지 않는다.
/// - 매칭 실패 시 `None` → 호출부에서 이모지 텍스트 fallback.
pub fn system_icon_for(icon: &str, kind: ItemKind) -> Option<(Handle, IconRole)> {
    if kind == ItemKind::Emoji {
        return None;
    }
    let (id, role) = EMOJI_LOOKUP.get(icon.trim())?;
    Some((HANDLE_CACHE.get(id)?.clone(), *role))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 시스템_명령_이모지_매핑() {
        // :help 목록의 핵심 명령 아이콘들이 전부 매핑되는지
        for emoji in [
            "\u{1F310}",         // 🌐 web
            "\u{1F522}",         // 🔢 calc
            "\u{1F60A}",         // 😊 emoji
            "\u{2699}\u{FE0F}",  // ⚙️ set
            "\u{26A1}",          // ⚡ transform
            "\u{1F4DD}",         // 📝 prompt
            "\u{1F4C2}",         // 📂 folder
            "\u{1F5FA}\u{FE0F}", // 🗺️ keys
        ] {
            assert!(
                system_icon_for(emoji, ItemKind::SystemCommand).is_some(),
                "매핑 누락: {emoji:?}"
            );
        }
    }

    #[test]
    fn 이모지_검색_결과는_매핑_안함() {
        // :emoji 결과는 실제 이모지를 보여줘야 하므로 SVG로 바꾸면 안 된다
        assert!(system_icon_for("\u{1F60A}", ItemKind::Emoji).is_none());
    }

    #[test]
    fn fe0f_유무_구분() {
        // 🗺️(keys) → keyboard, 🗺(maps) → map
        let (_, keys_role) = system_icon_for("\u{1F5FA}\u{FE0F}", ItemKind::SystemCommand).unwrap();
        assert_eq!(keys_role, IconRole::Accent);
        let (_, maps_role) = system_icon_for("\u{1F5FA}", ItemKind::WebSearch).unwrap();
        assert_eq!(maps_role, IconRole::Green);
    }

    #[test]
    fn 미매핑_이모지는_none() {
        assert!(system_icon_for("\u{1F525}", ItemKind::File).is_none()); // 🔥
    }

    #[test]
    fn 매핑_테이블의_아이콘id는_전부_캐시에_존재() {
        for (emoji, id, _) in EMOJI_MAP {
            assert!(
                HANDLE_CACHE.contains_key(id),
                "{emoji:?} → {id:?} SVG 미임베드"
            );
        }
    }
}
