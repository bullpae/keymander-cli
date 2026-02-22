//! 소스 감사 테스트: Windows 자식 프로세스의 CREATE_NO_WINDOW 준수 검증
//!
//! Desktop 런처(kmd-desktop)는 GUI 앱이므로, 자식 프로세스가 콘솔 창을
//! 띄우면 사용자 경험을 해친다. 이 테스트는 kmd-core 소스 내에서
//! `Command::new(` 호출이 있는 파일이 `CREATE_NO_WINDOW`를 사용하거나,
//! 명시적 면제 대상인지 확인한다.
//!
//! v0.3.5 회귀 방지: apps.rs, files.rs에서 CREATE_NO_WINDOW 누락으로
//! 콘솔 창이 노출되던 버그.

use std::path::Path;

/// 면제 파일: 사용자가 의도적으로 실행하는 대화형 명령
const EXEMPT_FILES: &[&str] = &[
    "builtin_shell.rs", // 사용자가 직접 실행하는 셸 명령 (!ip 등)
];

/// `Command::new(`를 포함하는 .rs 파일이 `CREATE_NO_WINDOW` 또는
/// `creation_flags`를 함께 포함하는지 검증한다.
#[test]
fn test_all_windows_commands_have_create_no_window() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(src_dir.is_dir(), "kmd-core/src 디렉터리를 찾을 수 없음");

    let mut violations = Vec::new();

    visit_rs_files(&src_dir, &mut |path| {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if EXEMPT_FILES.contains(&file_name) {
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        if !content.contains("Command::new(") {
            return;
        }

        // 테스트 코드 영역 제외
        let code = strip_test_modules(&content);

        if !code.contains("Command::new(") {
            return;
        }

        // Windows에서만 실행되는 Command 호출이 있는지 확인
        // (macOS/Linux 전용 코드는 무시)
        let has_windows_command = has_windows_scope_command(&code);

        if !has_windows_command {
            return;
        }

        // CREATE_NO_WINDOW 또는 creation_flags가 있는지 확인
        if !code.contains("CREATE_NO_WINDOW") && !code.contains("creation_flags") {
            violations.push(format!(
                "{}: Command::new() 사용하지만 CREATE_NO_WINDOW 없음",
                path.display()
            ));
        }
    });

    if !violations.is_empty() {
        panic!(
            "CREATE_NO_WINDOW 누락 파일 발견 ({}):\n  - {}",
            violations.len(),
            violations.join("\n  - ")
        );
    }
}

/// .rs 파일을 재귀적으로 방문
fn visit_rs_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, f);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            f(&path);
        }
    }
}

/// #[cfg(test)] mod tests { ... } 블록을 제거한 코드 반환
fn strip_test_modules(content: &str) -> String {
    let mut result = String::new();
    let mut depth = 0i32;
    let mut in_test_mod = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if !in_test_mod {
            if trimmed == "#[cfg(test)]" {
                in_test_mod = true;
                continue;
            }
            result.push_str(line);
            result.push('\n');
        } else {
            // #[cfg(test)] 다음 줄부터 mod block 추적
            for ch in trimmed.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if depth <= 0 && result.is_empty() || (depth <= 0 && trimmed.contains('}')) {
                in_test_mod = false;
                depth = 0;
            }
        }
    }
    result
}

/// Windows 스코프에서 Command::new를 호출하는지 판별
/// (macOS/Linux 전용 함수 내 Command는 무시)
fn has_windows_scope_command(code: &str) -> bool {
    // 간단한 휴리스틱: cfg(target_os = "windows") 블록 또는
    // 플랫폼 무관 코드에서 Command::new가 호출되는 경우
    let lines: Vec<&str> = code.lines().collect();
    let mut in_non_windows = false;
    let mut brace_depth = 0i32;

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.contains("cfg(target_os = \"macos\")")
            || trimmed.contains("cfg(target_os = \"linux\")")
            || trimmed.contains("cfg(not(target_os = \"windows\"))")
            || trimmed.contains("cfg(not(any(target_os = \"windows\"")
            || trimmed.contains("cfg(unix)")
        {
            in_non_windows = true;
            brace_depth = 0;
        }

        if in_non_windows {
            for ch in trimmed.chars() {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth <= 0 {
                        in_non_windows = false;
                    }
                }
            }
            continue;
        }

        if trimmed.contains("Command::new(") {
            return true;
        }
    }
    false
}
