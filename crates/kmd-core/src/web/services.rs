//! 서비스 정의 — 웹/맞춤법/번역 provider 상수

/// 웹 검색 서비스 정의
pub struct WebService {
    pub id: &'static str,
    pub name: &'static str,
    pub prefixes: &'static [&'static str],
    /// ASCII 아이콘 (2자, 레거시 터미널용)
    pub icon: &'static str,
    /// 이모지 아이콘 (모던 터미널용)
    pub emoji_icon: &'static str,
    pub url_template: &'static str,
    pub description: &'static str,
}

/// 맞춤법 검사 서비스 정의
pub struct SpellService {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub emoji_icon: &'static str,
    pub url_template: &'static str,
    pub description: &'static str,
}

/// 번역 서비스 정의
pub struct TranslateService {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub emoji_icon: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateDirection {
    Auto,
    EnToKo,
    KoToEn,
}

// pick_icon은 HasId trait의 디폴트 구현으로 통합 (하단 참조)

// ── 서비스 상수 ──────────────────────────────────────────────────────────────

pub const WEB_SERVICES: &[WebService] = &[
    WebService {
        id: "google",
        name: "Google",
        prefixes: &["@g", "@google"],
        icon: "Gg",
        emoji_icon: "\u{1F50D}", // 🔍
        url_template: "https://google.com/search?q={query}",
        description: "Search Google",
    },
    WebService {
        id: "youtube",
        name: "YouTube",
        prefixes: &["@yt", "@youtube"],
        icon: "Yt",
        emoji_icon: "\u{25B6}\u{FE0F}", // ▶️
        url_template: "https://youtube.com/results?search_query={query}",
        description: "Search YouTube",
    },
    WebService {
        id: "github",
        name: "GitHub",
        prefixes: &["@gh", "@github"],
        icon: "Gh",
        emoji_icon: "\u{1F431}", // 🐱
        url_template: "https://github.com/search?q={query}",
        description: "Search GitHub",
    },
    WebService {
        id: "stackoverflow",
        name: "StackOverflow",
        prefixes: &["@so", "@stackoverflow"],
        icon: "So",
        emoji_icon: "\u{1F4DA}", // 📚
        url_template: "https://stackoverflow.com/search?q={query}",
        description: "Search StackOverflow",
    },
    WebService {
        id: "npm",
        name: "npm",
        prefixes: &["@npm"],
        icon: "Np",
        emoji_icon: "\u{1F4E6}", // 📦
        url_template: "https://www.npmjs.com/search?q={query}",
        description: "Search npm packages",
    },
    WebService {
        id: "crates",
        name: "crates.io",
        prefixes: &["@crates", "@cargo"],
        icon: "Cr",
        emoji_icon: "\u{1F4E6}", // 📦
        url_template: "https://crates.io/search?q={query}",
        description: "Search Rust crates",
    },
    WebService {
        id: "wikipedia",
        name: "Wikipedia",
        prefixes: &["@w", "@wiki"],
        icon: "Wi",
        emoji_icon: "\u{1F4D6}", // 📖
        url_template: "https://en.wikipedia.org/wiki/Special:Search/{query}",
        description: "Search Wikipedia",
    },
    WebService {
        id: "x",
        name: "X (Twitter)",
        prefixes: &["@x", "@twitter"],
        icon: " X",
        emoji_icon: "\u{1D54F}", // 𝕏
        url_template: "https://x.com/search?q={query}",
        description: "Search X (Twitter)",
    },
    WebService {
        id: "maps",
        name: "Google Maps",
        prefixes: &["@map", "@maps"],
        icon: "Mp",
        emoji_icon: "\u{1F5FA}", // 🗺
        url_template: "https://maps.google.com/maps?q={query}",
        description: "Search Google Maps",
    },
    WebService {
        id: "naver_search",
        name: "Naver",
        prefixes: &["@naver", "@kr"],
        icon: "Nv",
        emoji_icon: "\u{1F1F0}\u{1F1F7}", // 🇰🇷
        url_template: "https://search.naver.com/search.naver?query={query}",
        description: "Search Naver",
    },
    WebService {
        id: "daum",
        name: "Daum",
        prefixes: &["@daum", "@dm"],
        icon: "Dm",
        emoji_icon: "\u{1F310}", // 🌐
        url_template: "https://search.daum.net/search?w=tot&q={query}",
        description: "Search Daum",
    },
    WebService {
        id: "naver_dict",
        name: "Naver Dict",
        prefixes: &["@dict", "@ndict"],
        icon: "Nv",
        emoji_icon: "\u{1F4D7}", // 📗
        url_template: "https://dict.naver.com/search?query={query}",
        description: "Search Naver Dictionary",
    },
    // AI 서비스
    WebService {
        id: "perplexity",
        name: "Perplexity",
        prefixes: &["@ai", "@pplx", "@perplexity"],
        icon: "Ai",
        emoji_icon: "\u{1F916}", // 🤖
        url_template: "https://www.perplexity.ai/search?q={query}",
        description: "Ask Perplexity AI",
    },
    WebService {
        id: "chatgpt",
        name: "ChatGPT",
        prefixes: &["@gpt", "@chatgpt"],
        icon: "Gp",
        emoji_icon: "\u{1F4AC}", // 💬
        url_template: "https://chatgpt.com/?q={query}",
        description: "Ask ChatGPT",
    },
    WebService {
        id: "claude",
        name: "Claude",
        prefixes: &["@claude"],
        icon: "Cl",
        emoji_icon: "\u{2728}", // ✨
        url_template: "https://claude.ai/new?q={query}",
        description: "Ask Claude AI",
    },
    WebService {
        id: "gemini",
        name: "Gemini",
        prefixes: &["@gemini"],
        icon: "Gm",
        emoji_icon: "\u{264A}", // ♊
        url_template: "https://gemini.google.com/app?q={query}",
        description: "Ask Google Gemini",
    },
    WebService {
        id: "grok",
        name: "Grok",
        prefixes: &["@grok"],
        icon: "Gr",
        emoji_icon: "\u{1F680}", // 🚀
        url_template: "https://grok.com/?q={query}",
        description: "Ask xAI Grok",
    },
];

pub const SPELL_SERVICES: &[SpellService] = &[
    SpellService {
        id: "naver_spell",
        name: "Naver Spell",
        icon: "Sp",
        emoji_icon: "\u{270D}\u{FE0F}", // ✍️
        url_template: "https://search.naver.com/search.naver?query={query}+맞춤법+검사",
        description: "Korean spelling check via Naver",
    },
    SpellService {
        id: "pusan_spell",
        name: "Pusan Spell",
        icon: "Ps",
        emoji_icon: "\u{1F4D6}", // 📖
        url_template: "https://search.naver.com/search.naver?query=부산대+맞춤법+{query}",
        description: "Open Pusan spell checker search",
    },
];

pub const TRANSLATE_SERVICES: &[TranslateService] = &[
    TranslateService {
        id: "google_translate",
        name: "Google Translate",
        icon: "Tr",
        emoji_icon: "\u{1F310}", // 🌐
        description: "Translate with Google",
    },
    TranslateService {
        id: "papago",
        name: "Papago",
        icon: "Pg",
        emoji_icon: "\u{1F1F0}\u{1F1F7}", // 🇰🇷
        description: "Translate with Papago",
    },
    TranslateService {
        id: "deepl",
        name: "DeepL",
        icon: "Dl",
        emoji_icon: "\u{1F4D8}", // 📘
        description: "Translate with DeepL",
    },
];

// ── ID 분류 헬퍼 ──────────────────────────────────────────────────────────────

pub fn is_llm_id(id: &str) -> bool {
    matches!(id, "chatgpt" | "gemini" | "claude" | "grok" | "perplexity")
}

pub fn is_multi_web_id(id: &str) -> bool {
    matches!(id, "google" | "naver_search" | "daum")
}

pub fn is_spell_id(id: &str) -> bool {
    matches!(id, "naver_spell" | "pusan_spell")
}

pub fn is_translate_id(id: &str) -> bool {
    matches!(id, "google_translate" | "papago" | "deepl")
}

// ── provider 선택 ─────────────────────────────────────────────────────────────

pub fn selected_llm_services(selected_ids: &[String]) -> Vec<&'static WebService> {
    select_services(selected_ids, WEB_SERVICES, is_llm_id)
}

pub fn selected_multi_web_services(selected_ids: &[String]) -> Vec<&'static WebService> {
    select_services(selected_ids, WEB_SERVICES, is_multi_web_id)
}

pub fn selected_spell_services(selected_ids: &[String]) -> Vec<&'static SpellService> {
    select_services(selected_ids, SPELL_SERVICES, is_spell_id)
}

pub fn selected_translate_services(selected_ids: &[String]) -> Vec<&'static TranslateService> {
    select_services(selected_ids, TRANSLATE_SERVICES, is_translate_id)
}

/// 공용 provider 선택 헬퍼 — 중복 코드 제거
fn select_services<T: HasId>(
    selected_ids: &[String],
    all: &'static [T],
    is_valid: fn(&str) -> bool,
) -> Vec<&'static T> {
    let mut matched = Vec::new();
    for id in selected_ids {
        let normalized = id.trim().to_lowercase();
        if normalized.is_empty() || !is_valid(&normalized) {
            continue;
        }
        if let Some(service) = all.iter().find(|svc| svc.service_id().eq_ignore_ascii_case(&normalized)) {
            matched.push(service);
        }
    }
    if matched.is_empty() {
        all.iter().filter(|svc| is_valid(svc.service_id())).collect()
    } else {
        matched
    }
}

/// 서비스 공통 trait — ID·설명·아이콘 접근 + pick_icon 디폴트 구현
pub trait HasId {
    fn service_id(&self) -> &str;
    fn service_description(&self) -> &str;
    fn ascii_icon(&self) -> &str;
    fn emoji_icon(&self) -> &str;

    /// 터미널 환경에 맞는 아이콘 반환 (emoji 지원 여부에 따라)
    fn pick_icon(&self, use_emoji: bool) -> &str {
        if use_emoji {
            self.emoji_icon()
        } else {
            self.ascii_icon()
        }
    }
}

impl HasId for WebService {
    fn service_id(&self) -> &str {
        self.id
    }
    fn service_description(&self) -> &str {
        self.description
    }
    fn ascii_icon(&self) -> &str {
        self.icon
    }
    fn emoji_icon(&self) -> &str {
        self.emoji_icon
    }
}

impl HasId for SpellService {
    fn service_id(&self) -> &str {
        self.id
    }
    fn service_description(&self) -> &str {
        self.description
    }
    fn ascii_icon(&self) -> &str {
        self.icon
    }
    fn emoji_icon(&self) -> &str {
        self.emoji_icon
    }
}

impl HasId for TranslateService {
    fn service_id(&self) -> &str {
        self.id
    }
    fn service_description(&self) -> &str {
        self.description
    }
    fn ascii_icon(&self) -> &str {
        self.icon
    }
    fn emoji_icon(&self) -> &str {
        self.emoji_icon
    }
}
