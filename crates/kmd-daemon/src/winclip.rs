//! Windows 클립보드 스냅샷/민감정보 **정책** — 순수 로직 (플랫폼 무관 컴파일).
//!
//! FFI 없는 판단 로직만 둔다: 어떤 포맷을 백업할 수 있고, 상한(개수/포맷당/전체
//! 바이트)을 넘으면 어떻게 실패하는지, 어떤 마커가 "수집 제외" 신호인지.
//! macOS에서도 컴파일·테스트되므로 Windows 실기 없이 정책 회귀를 잡는다.
//! 실제 클립보드 접근(clipboard-win)은 `clipboard.rs`의 `platform` 모듈 담당.

/// 스냅샷에 담는 포맷 개수 상한. 실사용 클립보드는 10개 안팎이고,
/// 그 이상은 비정상 소유자일 가능성이 높다 (메모리 보호).
pub const MAX_SNAPSHOT_FORMATS: usize = 32;
/// 포맷 하나의 바이트 상한 (대형 DIB/커스텀 데이터 복사 방어).
pub const MAX_SNAPSHOT_FORMAT_BYTES: usize = 16 * 1024 * 1024;
/// 스냅샷 전체 바이트 상한.
pub const MAX_SNAPSHOT_TOTAL_BYTES: usize = 64 * 1024 * 1024;

// 표준 클립보드 포맷 ID (winuser.h). clipboard-win 상수와 같은 값이지만
// 이 모듈은 비-Windows에서도 컴파일되어야 하므로 직접 정의한다.
pub const CF_TEXT: u32 = 1;
pub const CF_BITMAP: u32 = 2;
pub const CF_METAFILEPICT: u32 = 3;
pub const CF_DIB: u32 = 8;
pub const CF_PALETTE: u32 = 9;
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_ENHMETAFILE: u32 = 14;
pub const CF_DIBV5: u32 = 17;
pub const CF_OWNERDISPLAY: u32 = 0x0080;
pub const CF_DSPTEXT: u32 = 0x0081;
pub const CF_DSPBITMAP: u32 = 0x0082;
pub const CF_DSPMETAFILEPICT: u32 = 0x0083;
pub const CF_DSPENHMETAFILE: u32 = 0x008E;
const CF_GDIOBJFIRST: u32 = 0x0300;
const CF_GDIOBJLAST: u32 = 0x03FF;

/// GDI 핸들/소유자 의존 포맷 — HGLOBAL이 아니라 GlobalLock으로 읽을 수 없다.
/// 스냅샷 열거 시 바이트 복사를 시도하지 말아야 한다 (핸들 오용 방지).
pub fn is_gdi_or_owner_format(format: u32) -> bool {
    matches!(
        format,
        CF_BITMAP
            | CF_METAFILEPICT
            | CF_PALETTE
            | CF_ENHMETAFILE
            | CF_OWNERDISPLAY
            | CF_DSPBITMAP
            | CF_DSPMETAFILEPICT
            | CF_DSPENHMETAFILE
    ) || (CF_GDIOBJFIRST..=CF_GDIOBJLAST).contains(&format)
}

/// 스냅샷 전 포맷 하나의 관측 결과.
#[derive(Debug, Clone, Copy)]
pub enum FormatProbe {
    /// HGLOBAL 기반 — 바이트 복사 가능, size = GlobalSize 관측치.
    Readable(u32, usize),
    /// GDI 핸들이거나 크기/데이터를 얻지 못한 포맷.
    Unreadable(u32),
}

/// 스냅샷 계획: 관측된 포맷들로부터 "안전하게 백업 가능한 포맷 목록"을 정한다.
///
/// 실패(Err) = 원본 클립보드를 손실 없이 복원할 수 없다는 뜻 — 호출자는
/// 붙여넣기를 **중단**해 사용자의 클립보드를 절대 파괴하지 않는다 (fail-safe).
///
/// 읽지 못하는 포맷의 면책 규칙 (Windows가 자동 합성하는 동등 표현):
/// - 비트맵/팔레트 등 **래스터** 계열은 CF_DIB/CF_DIBV5가 있으면 복원 시
///   재합성된다. 메타파일(벡터)은 DIB로 재합성되지 않으므로 면책하지 않는다.
/// - CF_DSPTEXT는 텍스트 포맷이 있으면 동등하다.
pub fn plan_snapshot(probes: &[FormatProbe]) -> Result<Vec<u32>, String> {
    let mut readable: Vec<(u32, usize)> = Vec::new();
    let mut unreadable: Vec<u32> = Vec::new();
    for probe in probes {
        match *probe {
            FormatProbe::Readable(format, size) => readable.push((format, size)),
            FormatProbe::Unreadable(format) => unreadable.push(format),
        }
    }

    if readable.len() > MAX_SNAPSHOT_FORMATS {
        return Err(format!(
            "클립보드 포맷이 {}개로 스냅샷 상한({MAX_SNAPSHOT_FORMATS}개)을 넘습니다",
            readable.len()
        ));
    }
    let mut total: usize = 0;
    for (format, size) in &readable {
        // DIB(스크린샷 등 이미지)는 16MB를 쉽게 넘으므로 포맷당 상한을 면제하고
        // 전체 예산(64MiB)으로만 막는다. 그 외 포맷은 포맷당 상한을 유지한다.
        let per_format_capped = !matches!(*format, CF_DIB | CF_DIBV5);
        if per_format_capped && *size > MAX_SNAPSHOT_FORMAT_BYTES {
            return Err(format!(
                "클립보드 포맷 {format}이 {size}바이트로 포맷당 상한({MAX_SNAPSHOT_FORMAT_BYTES}바이트)을 넘습니다"
            ));
        }
        total = total.saturating_add(*size);
    }
    if total > MAX_SNAPSHOT_TOTAL_BYTES {
        return Err(format!(
            "클립보드 전체 {total}바이트로 스냅샷 상한({MAX_SNAPSHOT_TOTAL_BYTES}바이트)을 넘습니다"
        ));
    }

    let has_dib = readable
        .iter()
        .any(|(f, _)| matches!(*f, CF_DIB | CF_DIBV5));
    let has_text = readable
        .iter()
        .any(|(f, _)| matches!(*f, CF_TEXT | CF_UNICODETEXT));
    let unpreserved: Vec<u32> = unreadable
        .into_iter()
        .filter(|f| !excused(*f, has_dib, has_text))
        .collect();
    if !unpreserved.is_empty() {
        // 포맷 ID만 남긴다 — 이름/내용은 로그·에러에 싣지 않는다 (프라이버시).
        return Err(format!(
            "복원할 수 없는 클립보드 포맷이 있습니다: {unpreserved:?}"
        ));
    }

    Ok(readable.into_iter().map(|(f, _)| f).collect())
}

/// 읽지 못한 포맷이 다른 백업 포맷으로 재합성 가능하면 true.
///
/// 래스터 계열만 DIB로 면책한다. 메타파일(CF_METAFILEPICT/CF_ENHMETAFILE 등,
/// Office 개체·차트 복사의 벡터 원본)은 DIB 래스터로 재합성되지 않으므로 DIB가
/// 있어도 면책하지 않는다 — 안전하게 deep-copy할 수 없는 조합은 plan_snapshot이
/// 실패해 붙여넣기 스왑 자체가 중단된다 (벡터 포맷 손실 방지, fail-safe).
fn excused(format: u32, has_dib: bool, has_text: bool) -> bool {
    match format {
        CF_BITMAP | CF_PALETTE | CF_DSPBITMAP => has_dib,
        CF_DSPTEXT => has_text,
        _ => false,
    }
}

/// 복원 직전(클립보드 잠금 후) 세대 번호 재검증 — 사전 판정과 잠금(OpenClipboard)
/// 사이에 끼어든 외부 복사(TOCTOU)를 덮어쓰지 않기 위한 최종 판정.
/// `expected == 0`은 세대 미지원 또는 무조건 복원 경로 — 검증 없이 진행한다.
pub fn generation_still_current(expected: isize, current: isize) -> bool {
    expected == 0 || expected == current
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
    fn 정상_포맷들은_모두_스냅샷에_포함된다() {
        let probes = [
            FormatProbe::Readable(CF_UNICODETEXT, 128),
            FormatProbe::Readable(CF_DIB, 4096),
            FormatProbe::Readable(0xC123, 256), // 커스텀 등록 포맷
        ];
        let plan = plan_snapshot(&probes).unwrap();
        assert_eq!(plan, vec![CF_UNICODETEXT, CF_DIB, 0xC123]);
    }

    #[test]
    fn gdi_비트맵은_dib가_있으면_면책된다() {
        let probes = [
            FormatProbe::Unreadable(CF_BITMAP),
            FormatProbe::Unreadable(CF_PALETTE),
            FormatProbe::Unreadable(CF_DSPBITMAP),
            FormatProbe::Readable(CF_DIBV5, 4096),
        ];
        assert_eq!(plan_snapshot(&probes).unwrap(), vec![CF_DIBV5]);
    }

    #[test]
    fn 메타파일은_dib가_있어도_면책되지_않는다() {
        // Office 개체 복사 등의 벡터 원본 — DIB 래스터로 재합성되지 않으므로
        // 스냅샷 계획이 실패해 붙여넣기 스왑이 중단되어야 한다 (손실 방지).
        for metafile in [
            CF_METAFILEPICT,
            CF_ENHMETAFILE,
            CF_DSPMETAFILEPICT,
            CF_DSPENHMETAFILE,
        ] {
            let probes = [
                FormatProbe::Unreadable(metafile),
                FormatProbe::Readable(CF_DIBV5, 4096),
                FormatProbe::Readable(CF_UNICODETEXT, 64),
            ];
            let err = plan_snapshot(&probes).unwrap_err();
            assert!(err.contains("복원할 수 없는"), "포맷 {metafile}: {err}");
        }
    }

    #[test]
    fn gdi_비트맵만_있고_dib가_없으면_실패한다() {
        let probes = [FormatProbe::Unreadable(CF_BITMAP)];
        let err = plan_snapshot(&probes).unwrap_err();
        assert!(err.contains("복원할 수 없는"), "{err}");
    }

    #[test]
    fn 지연_렌더_실패_등_읽지못한_일반_포맷은_실패한다() {
        let probes = [
            FormatProbe::Readable(CF_UNICODETEXT, 16),
            FormatProbe::Unreadable(0xC456), // 읽지 못한 커스텀 포맷
        ];
        assert!(plan_snapshot(&probes).is_err());
    }

    #[test]
    fn 포맷당_바이트_상한을_넘으면_실패한다() {
        let probes = [FormatProbe::Readable(0xC789, MAX_SNAPSHOT_FORMAT_BYTES + 1)];
        let err = plan_snapshot(&probes).unwrap_err();
        assert!(err.contains("포맷당 상한"), "{err}");
    }

    #[test]
    fn 대형_dib는_포맷당_상한을_면제받고_전체_예산_안에서_허용된다() {
        // 4K 스크린샷 DIB는 16MB를 쉽게 넘는다 — 전체 64MiB 예산 안이면 백업한다.
        let probes = [
            FormatProbe::Readable(CF_DIB, 40 * 1024 * 1024),
            FormatProbe::Readable(CF_UNICODETEXT, 128),
        ];
        let plan = plan_snapshot(&probes).unwrap();
        assert_eq!(plan, vec![CF_DIB, CF_UNICODETEXT]);
        // CF_DIBV5도 동일하게 면제된다.
        let probes = [FormatProbe::Readable(
            CF_DIBV5,
            MAX_SNAPSHOT_FORMAT_BYTES + 1,
        )];
        assert!(plan_snapshot(&probes).is_ok());
    }

    #[test]
    fn 대형_dib도_전체_상한은_넘지_못한다() {
        let probes = [FormatProbe::Readable(CF_DIB, MAX_SNAPSHOT_TOTAL_BYTES + 1)];
        let err = plan_snapshot(&probes).unwrap_err();
        assert!(err.contains("전체"), "{err}");
    }

    #[test]
    fn 전체_바이트_상한을_넘으면_실패한다() {
        // 포맷당 상한 이하지만 합계가 전체 상한 초과
        let per = MAX_SNAPSHOT_FORMAT_BYTES;
        let count = MAX_SNAPSHOT_TOTAL_BYTES / per + 1;
        let probes: Vec<FormatProbe> = (0..count as u32)
            .map(|i| FormatProbe::Readable(0xC000 + i, per))
            .collect();
        let err = plan_snapshot(&probes).unwrap_err();
        assert!(err.contains("전체"), "{err}");
    }

    #[test]
    fn 포맷_개수_상한을_넘으면_실패한다() {
        let probes: Vec<FormatProbe> = (0..MAX_SNAPSHOT_FORMATS as u32 + 1)
            .map(|i| FormatProbe::Readable(0xC000 + i, 8))
            .collect();
        let err = plan_snapshot(&probes).unwrap_err();
        assert!(err.contains("개"), "{err}");
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

    #[test]
    fn 세대_재검증은_외부_복사를_감지한다() {
        // 잠금 후 세대가 그대로면 복원 진행, 바뀌었으면(외부 복사) 중단.
        assert!(generation_still_current(7, 7));
        assert!(!generation_still_current(7, 8));
        // 0 = 세대 미지원/무조건 복원 경로 — 검증 생략.
        assert!(generation_still_current(0, 999));
    }
}
