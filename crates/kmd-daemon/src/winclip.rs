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

/// 주입 텍스트의 UTF-16 code unit 상한. 유닛당 키 이벤트 2개(down/up)를 **단
/// 한 번의 SendInput 호출**로 보내므로(물리 입력 interleave 창 제거), 상한이
/// 곧 호출 한 번의 이벤트 수다. 4096유닛(이벤트 8192개)은 입력 큐를 점령하지
/// 않는 보수적 값이다. 초과 시 주입을 거부(Err)한다 — 클립보드는 무변형이라
/// 사용자 데이터 손실은 없다.
pub const MAX_INJECT_UTF16_UNITS: usize = 4096;

/// 텍스트 → 주입용 UTF-16 시퀀스 (KEYEVENTF_UNICODE의 wScan 값들).
///
/// **단일 행만 허용**: CR/LF가 하나라도 있으면 Err. Ctrl+V 붙여넣기는 터미널이
/// bracketed paste로 감싸 개행이 명령 실행이 되지 않게 보호하지만, 타이핑
/// 주입의 개행은 물리 Enter와 구분되지 않아 그 보호를 우회한다 — Terminal/REPL
/// 에서 각 행이 **즉시 명령으로 실행**될 수 있으므로 거부가 유일하게 안전하다.
/// 시스템 클립보드 쓰기/복원 경로로 폴백하지도 않는다 (무변형 불변식).
///
/// 명시적 한계 (클립보드 무변형 주입의 트레이드오프):
/// - 다중 행: 위 이유로 Err — Ctrl+V처럼 여러 줄을 붙여넣지 못한다.
/// - 길이: [`MAX_INJECT_UTF16_UNITS`] (4096유닛) 초과 텍스트는 Err — Ctrl+V처럼
///   무제한 길이를 즉시 붙여넣지 못한다.
/// - 호환: KEYEVENTF_UNICODE는 VK_PACKET 경유라 raw input/스캔코드만 읽는 앱
///   (일부 게임·가상화 콘솔)에선 무시될 수 있다. 사용자가 물리적으로 누르고
///   있는 modifier가 일부 앱에서 문자 해석을 바꿀 수 있다 (Ctrl+문자 등).
/// - 서식: 텍스트만 주입된다 (히스토리 자체가 텍스트 전용이라 기능 손실 없음).
pub fn encode_for_injection(text: &str) -> Result<Vec<u16>, String> {
    if text.contains(['\r', '\n']) {
        return Err(
            "여러 줄 텍스트는 타이핑 주입할 수 없습니다 — 주입된 개행은 터미널의 \
             bracketed paste 보호를 우회해 명령이 즉시 실행될 수 있습니다. \
             클립보드는 변경되지 않았습니다"
                .to_string(),
        );
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() > MAX_INJECT_UTF16_UNITS {
        return Err(format!(
            "텍스트가 UTF-16 {}유닛으로 주입 상한({MAX_INJECT_UTF16_UNITS}유닛)을 넘습니다 — \
             클립보드는 변경되지 않았습니다",
            units.len()
        ));
    }
    Ok(units)
}

/// 부분 전송 뒷정리 판정 (순수): SendInput이 이벤트 `sent_events`개만 넣고
/// 중단됐을 때, 홀수면 마지막으로 들어간 이벤트는 짝(up)이 없는 key-down이다 —
/// 대상 앱에 눌린 채 남지 않도록 key-up을 보내야 할 유닛을 돌려준다.
/// 짝수(쌍이 온전히 끝남)면 None.
pub fn dangling_down_unit(units: &[u16], sent_events: usize) -> Option<u16> {
    if sent_events % 2 == 1 {
        // 이벤트 배열은 units[i]의 down이 2i, up이 2i+1 — 마지막 전송 이벤트
        // (sent_events - 1)가 짝수 인덱스이므로 유닛은 (sent_events - 1) / 2.
        units.get((sent_events - 1) / 2).copied()
    } else {
        None
    }
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

    fn is_high_surrogate(unit: u16) -> bool {
        (0xD800..=0xDBFF).contains(&unit)
    }

    fn is_low_surrogate(unit: u16) -> bool {
        (0xDC00..=0xDFFF).contains(&unit)
    }

    #[test]
    fn 단일_행_텍스트는_그대로_utf16이_된다() {
        let units = encode_for_injection("abc 123! 한글").unwrap();
        assert_eq!(units, "abc 123! 한글".encode_utf16().collect::<Vec<u16>>());
    }

    #[test]
    fn 개행이_있으면_bracketed_paste_우회를_막기_위해_거부한다() {
        // 주입된 개행은 물리 Enter와 구분되지 않아 터미널의 bracketed paste
        // 보호를 우회한다 — LF/CR/CRLF 어느 형태든, 어디에 있든 전부 Err.
        for text in [
            "a\nb",
            "a\rb",
            "a\r\nb",
            "\n",
            "\r",
            "끝에 개행\n",
            "cmd1\ncmd2\n",
        ] {
            let err = encode_for_injection(text).unwrap_err();
            assert!(err.contains("bracketed paste"), "{text:?}: {err}");
            assert!(
                err.contains("클립보드는 변경되지 않았습니다"),
                "{text:?}: {err}"
            );
        }
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
        let text = "가".repeat(MAX_INJECT_UTF16_UNITS + 1); // '가' = 1유닛
        let err = encode_for_injection(&text).unwrap_err();
        assert!(err.contains("주입 상한"), "{err}");
        assert!(err.contains("클립보드는 변경되지 않았습니다"), "{err}");
    }

    #[test]
    fn 상한_경계_4096유닛은_허용된다() {
        assert_eq!(MAX_INJECT_UTF16_UNITS, 4096, "보수적 상한 (이벤트 8192개)");
        let text = "a".repeat(MAX_INJECT_UTF16_UNITS);
        assert_eq!(
            encode_for_injection(&text).unwrap().len(),
            MAX_INJECT_UTF16_UNITS
        );
    }

    #[test]
    fn 상한_경계는_서러게이트_쌍_기준으로도_정확하다() {
        // 🦀 = 2유닛: 2048개 = 정확히 4096유닛 → 허용, 하나 더 → 거부.
        let at_cap = "🦀".repeat(MAX_INJECT_UTF16_UNITS / 2);
        assert_eq!(
            encode_for_injection(&at_cap).unwrap().len(),
            MAX_INJECT_UTF16_UNITS
        );
        let over = "🦀".repeat(MAX_INJECT_UTF16_UNITS / 2 + 1);
        assert!(encode_for_injection(&over).is_err());
    }

    #[test]
    fn 빈_텍스트는_빈_시퀀스가_된다() {
        assert!(encode_for_injection("").unwrap().is_empty());
    }

    #[test]
    fn 부분_전송이_홀수면_마지막_down의_유닛을_돌려준다() {
        // "abc"는 이벤트 6개. 5개 전송 = units[2]('c')의 down까지 들어가고 up이 잘림.
        let units: Vec<u16> = "abc".encode_utf16().collect();
        assert_eq!(dangling_down_unit(&units, 5), Some(b'c' as u16));
        assert_eq!(dangling_down_unit(&units, 1), Some(b'a' as u16));
        // 짝수 = down/up 쌍이 온전 — 뒷정리 불필요.
        assert_eq!(dangling_down_unit(&units, 4), None);
        assert_eq!(dangling_down_unit(&units, 0), None);
        assert_eq!(dangling_down_unit(&units, 6), None);
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
