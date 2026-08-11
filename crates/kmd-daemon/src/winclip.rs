//! Windows 붙여넣기 주입/민감정보 **정책** — 순수 로직 (플랫폼 무관 컴파일).
//!
//! FFI 없는 판단 로직만 둔다: 붙여넣을 텍스트를 어떤 UTF-16 시퀀스로 주입하고,
//! 어떤 마커가 "수집 제외" 신호인지. macOS에서도 컴파일·테스트되므로 Windows
//! 실기 없이 정책 회귀를 잡는다. 실제 주입(SendInput)은 `clipboard.rs`의
//! `platform` 모듈 담당.
//!
//! ## 왜 클립보드 스냅샷/복원 정책이 없는가 (설계 불변식)
//!
//! Windows 붙여넣기는 시스템 클립보드를 **전혀 변형하지 않는다** — 임시 교체
//! (SetClipboardData) + Ctrl+V + 복원 대신, 항목 텍스트를 SendInput
//! KEYEVENTF_UNICODE 타이핑으로 직접 주입한다. 복원 기반 설계는 어떤 변형으로도
//! "부분 포맷 소실 불가능"을 보장할 수 없다:
//! - 다중 SetClipboardData는 트랜잭션이 아니다 — EmptyClipboard 후 n번째 set이
//!   실패하면 롤백이 없다.
//! - OLE(OleGetClipboard→보관→OleSetClipboard)도 지연 렌더 원본에선 소스 앱
//!   프록시라, 스왑 중 소스 앱이 종료하면 복원할 데이터 자체가 사라진다.
//!
//! 히스토리는 텍스트 전용이므로 타이핑 주입으로 기능 손실이 없고, 원본
//! 클립보드(이미지·파일·커스텀 포맷 전부)는 건드리지 않았으니 소실이 원천
//! 불가능하다. 한계는 [`encode_for_injection`] 문서 참고.

/// 주입 텍스트의 UTF-16 code unit 상한. 유닛당 키 이벤트 2개(down/up)를 보내므로
/// 상한이 없으면 1MB 히스토리 항목이 수백만 이벤트가 되어 입력 큐를 점령한다.
/// 초과 시 주입을 거부(Err)한다 — 클립보드는 무변형이라 사용자 데이터 손실은 없다.
pub const MAX_INJECT_UTF16_UNITS: usize = 64 * 1024;

/// SendInput 한 번에 보내는 code unit 수. 배치 사이에 짧은 양보를 넣어
/// 대상 앱 메시지 큐가 소화할 시간을 주고 CPU 포화를 피한다 (사고 수칙).
pub const INJECT_CHUNK_UNITS: usize = 256;

/// 텍스트 → 주입용 UTF-16 시퀀스 (KEYEVENTF_UNICODE의 wScan 값들).
///
/// 개행 정규화: CRLF/LF → CR 하나. 물리 Enter가 만드는 WM_CHAR는 0x0D(CR)라서
/// LF를 그대로 주입하면 많은 편집 컨트롤이 개행으로 인식하지 못한다.
///
/// 명시적 한계 (클립보드 무변형 주입의 트레이드오프):
/// - 길이: [`MAX_INJECT_UTF16_UNITS`] 초과 텍스트는 Err — Ctrl+V처럼 무제한
///   길이를 즉시 붙여넣지 못한다.
/// - 호환: KEYEVENTF_UNICODE는 VK_PACKET 경유라 raw input/스캔코드만 읽는 앱
///   (일부 게임·가상화 콘솔)에선 무시될 수 있다. 사용자가 물리적으로 누르고
///   있는 modifier가 일부 앱에서 문자 해석을 바꿀 수 있다 (Ctrl+문자 등).
/// - 서식: 텍스트만 주입된다 (히스토리 자체가 텍스트 전용이라 기능 손실 없음).
pub fn encode_for_injection(text: &str) -> Result<Vec<u16>, String> {
    let mut units: Vec<u16> = Vec::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut buf = [0u16; 2];
    while let Some(c) = chars.next() {
        let mapped = match c {
            '\r' => {
                // CRLF는 CR 하나로 — LF까지 주입하면 개행이 두 번 된다.
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                '\r'
            }
            '\n' => '\r',
            other => other,
        };
        units.extend_from_slice(mapped.encode_utf16(&mut buf));
    }
    if units.len() > MAX_INJECT_UTF16_UNITS {
        return Err(format!(
            "텍스트가 UTF-16 {}유닛으로 주입 상한({MAX_INJECT_UTF16_UNITS}유닛)을 넘습니다 — \
             클립보드는 변경되지 않았습니다",
            units.len()
        ));
    }
    Ok(units)
}

/// 주입 배치 경계 계획. 각 배치는 `chunk_units` 이하이며, **서러게이트 쌍을
/// 배치 경계에서 쪼개지 않는다** — 배치 사이 양보 중 실제 키 입력이 끼어들면
/// 반쪽 서러게이트가 깨진 문자로 남기 때문이다.
pub fn injection_chunks(units: &[u16], chunk_units: usize) -> Vec<std::ops::Range<usize>> {
    // 쌍(2유닛)을 절대 쪼개지 않으려면 배치가 최소 2유닛은 되어야 한다.
    let chunk = chunk_units.max(2);
    let mut ranges = Vec::with_capacity(units.len().div_ceil(chunk));
    let mut start = 0;
    while start < units.len() {
        let mut end = (start + chunk).min(units.len());
        if end < units.len() && is_high_surrogate(units[end - 1]) && is_low_surrogate(units[end]) {
            end -= 1;
        }
        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn is_high_surrogate(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}

// ── 민감 항목 마커 (수집 제외) ──────────────────────────────────────────────

/// 존재만으로 수집 제외인 마커 (클립보드 관리자 표준 관례).
pub const MARKER_EXCLUDE: &str = "ExcludeClipboardContentFromMonitorProcessing";
/// 값이 0(DWORD)이면 수집 제외인 마커 (Windows 클립보드 히스토리 opt-out).
/// `CanUploadToCloudClipboard=0`은 **클라우드 동기화**만 거부하는 신호이지
/// 로컬 히스토리 민감 표시가 아니므로 여기 넣지 않는다 (과잉 제외 방지 —
/// 로컬 수집 제외는 ExcludeClipboardContentFromMonitorProcessing과
/// CanIncludeInClipboardHistory=0이 담당한다).
pub const MARKERS_OPT_OUT_ZERO: [&str; 1] = ["CanIncludeInClipboardHistory"];

/// opt-out 마커의 값이 "제외"(DWORD 0)인지.
pub fn opt_out_value_is_exclusion(data: &[u8]) -> bool {
    data.get(..4).is_some_and(|v| v == 0u32.to_ne_bytes())
}

/// 포맷 이름+데이터가 "이 클립보드는 수집하지 말라"는 신호인지 판정.
pub fn format_is_sensitive(name: &str, data: &[u8]) -> bool {
    if name == MARKER_EXCLUDE {
        return true;
    }
    MARKERS_OPT_OUT_ZERO.contains(&name) && opt_out_value_is_exclusion(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_텍스트는_그대로_utf16이_된다() {
        let units = encode_for_injection("abc 123!").unwrap();
        assert_eq!(units, "abc 123!".encode_utf16().collect::<Vec<u16>>());
    }

    #[test]
    fn 개행은_cr_하나로_정규화된다() {
        // 물리 Enter의 WM_CHAR는 0x0D — LF/CRLF 모두 CR 하나로 주입해야
        // 편집 컨트롤이 개행으로 인식하고, CRLF가 개행 두 번이 되지 않는다.
        assert_eq!(encode_for_injection("a\nb").unwrap(), vec![97, 0x0D, 98]);
        assert_eq!(encode_for_injection("a\r\nb").unwrap(), vec![97, 0x0D, 98]);
        assert_eq!(encode_for_injection("a\rb").unwrap(), vec![97, 0x0D, 98]);
        assert_eq!(
            encode_for_injection("a\r\n\r\nb").unwrap(),
            vec![97, 0x0D, 0x0D, 98]
        );
    }

    #[test]
    fn 비bmp_문자는_서러게이트_쌍으로_인코딩된다() {
        let units = encode_for_injection("🦀").unwrap();
        assert_eq!(units.len(), 2);
        assert!(is_high_surrogate(units[0]));
        assert!(is_low_surrogate(units[1]));
    }

    #[test]
    fn 주입_상한을_넘으면_클립보드_무변형을_알리며_거부한다() {
        let text = "가".repeat(MAX_INJECT_UTF16_UNITS + 1);
        let err = encode_for_injection(&text).unwrap_err();
        assert!(err.contains("주입 상한"), "{err}");
        assert!(err.contains("클립보드는 변경되지 않았습니다"), "{err}");
    }

    #[test]
    fn 상한_이내_최대_길이는_허용된다() {
        let text = "a".repeat(MAX_INJECT_UTF16_UNITS);
        assert_eq!(
            encode_for_injection(&text).unwrap().len(),
            MAX_INJECT_UTF16_UNITS
        );
    }

    #[test]
    fn 배치_경계는_서러게이트_쌍을_쪼개지_않는다() {
        // 홀수 위치마다 쌍이 걸리도록: [BMP, high, low, high, low, ...]
        let mut units = vec![97u16];
        for _ in 0..8 {
            units.extend_from_slice(&[0xD83E, 0xDD80]); // 🦀
        }
        // 배치 크기 2 → 경계가 쌍 한가운데(97 | D83E, DD80 ...)를 지나가는 구성
        let ranges = injection_chunks(&units, 2);
        let mut covered = Vec::new();
        for range in &ranges {
            let slice = &units[range.clone()];
            assert!(
                !is_high_surrogate(*slice.last().unwrap()) || range.end == units.len(),
                "배치가 high surrogate로 끝나면 쌍이 쪼개진 것: {range:?}"
            );
            assert!(
                !is_low_surrogate(slice[0]) || range.start == 0,
                "배치가 low surrogate로 시작하면 쌍이 쪼개진 것: {range:?}"
            );
            covered.extend_from_slice(slice);
        }
        assert_eq!(covered, units, "배치들을 이어 붙이면 원본과 같아야 한다");
    }

    #[test]
    fn 배치_크기와_전체_커버리지가_유지된다() {
        let units: Vec<u16> = (0..1000).map(|i| 97 + (i % 26) as u16).collect();
        let ranges = injection_chunks(&units, INJECT_CHUNK_UNITS);
        assert!(ranges.iter().all(|r| r.len() <= INJECT_CHUNK_UNITS));
        assert_eq!(ranges.iter().map(std::ops::Range::len).sum::<usize>(), 1000);
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end, 1000);
    }

    #[test]
    fn 빈_텍스트는_빈_계획이_된다() {
        assert!(encode_for_injection("").unwrap().is_empty());
        assert!(injection_chunks(&[], INJECT_CHUNK_UNITS).is_empty());
    }

    #[test]
    fn windows_수집_제외_마커를_민감항목으로_판정한다() {
        assert!(format_is_sensitive(MARKER_EXCLUDE, &[]));
        assert!(format_is_sensitive(
            "CanIncludeInClipboardHistory",
            &0u32.to_ne_bytes()
        ));
        assert!(!format_is_sensitive(
            "CanIncludeInClipboardHistory",
            &1u32.to_ne_bytes()
        ));
        assert!(!format_is_sensitive("HTML Format", &[]));
    }

    #[test]
    fn 클라우드_업로드_거부는_로컬_수집_제외가_아니다() {
        // CanUploadToCloudClipboard=0은 클라우드 동기화 opt-out일 뿐이다 —
        // 로컬 히스토리 민감 표시로 취급하면 정상 항목을 과잉 제외한다.
        assert!(!format_is_sensitive(
            "CanUploadToCloudClipboard",
            &0u32.to_ne_bytes()
        ));
        assert!(!MARKERS_OPT_OUT_ZERO.contains(&"CanUploadToCloudClipboard"));
    }
}
