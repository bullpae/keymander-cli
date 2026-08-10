//! 데몬 자동 시작 등록/해제
//!
//! Windows: Startup 폴더에 VBS 스크립트 (콘솔 창 없이 백그라운드 실행)
//! macOS:   ~/Library/LaunchAgents/com.keymander.daemon.plist
//! Linux:   ~/.config/systemd/user/kmd-daemon.service

/// 현재 실행 파일 경로로 자동 시작 등록
pub fn install() -> Result<String, String> {
    platform::install()
}

/// 자동 시작 해제
pub fn uninstall() -> Result<(), String> {
    platform::uninstall()
}

/// 자동 시작이 등록되어 있는지 확인
pub fn is_installed() -> bool {
    platform::is_installed()
}

// ── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use std::path::PathBuf;

    const SCRIPT_NAME: &str = "kmd-daemon.vbs";

    fn startup_dir() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|p| format!("{p}\\AppData\\Roaming"))
                .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Roaming".into())
        });
        PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup")
    }

    pub fn install() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| format!("실행 파일 경로 확인 실패: {e}"))?;
        let vbs = format!(
            "Set s = CreateObject(\"WScript.Shell\")\ns.Run \"\"\"{}\"\" start\", 0, False",
            exe.display()
        );

        let path = startup_dir().join(SCRIPT_NAME);
        std::fs::write(&path, vbs).map_err(|e| format!("시작 프로그램 등록 실패: {e}"))?;

        Ok(format!("등록 위치: {}", path.display()))
    }

    pub fn uninstall() -> Result<(), String> {
        let path = startup_dir().join(SCRIPT_NAME);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("시작 프로그램 제거 실패: {e}"))?;
        }
        Ok(())
    }

    pub fn is_installed() -> bool {
        startup_dir().join(SCRIPT_NAME).exists()
    }
}

// ── macOS ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;

    const LABEL: &str = "com.keymander.daemon";

    fn plist_path() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home.join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    }

    pub fn install() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| format!("실행 파일 경로 확인 실패: {e}"))?;

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>start</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
            exe = exe.display(),
        );

        let path = plist_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, plist).map_err(|e| e.to_string())?;

        // load(구식)가 아닌 bootout→bootstrap: 이전 등록이 다른 바이너리 경로를
        // 가리키고 있어도 새 plist로 확실히 재바인딩된다.
        let (domain, service) = launchd_target();
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &service])
            .output();
        let _ = std::process::Command::new("launchctl")
            .args(["bootstrap", &domain, &path.to_string_lossy()])
            .output();

        Ok(format!("등록 위치: {}", path.display()))
    }

    pub fn uninstall() -> Result<(), String> {
        let path = plist_path();
        if path.exists() {
            let (_, service) = launchd_target();
            let _ = std::process::Command::new("launchctl")
                .args(["bootout", &service])
                .output();
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// launchctl 도메인(`gui/<uid>`)과 서비스 타깃 문자열
    fn launchd_target() -> (String, String) {
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "501".into());
        let domain = format!("gui/{uid}");
        let service = format!("{domain}/{LABEL}");
        (domain, service)
    }

    pub fn is_installed() -> bool {
        plist_path().exists()
    }
}

// ── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use std::path::PathBuf;

    const SERVICE_NAME: &str = "kmd-daemon";

    fn service_path() -> PathBuf {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".config"))
                    .unwrap_or_else(|_| PathBuf::from("/tmp/.config"))
            });
        config_dir
            .join("systemd/user")
            .join(format!("{SERVICE_NAME}.service"))
    }

    pub fn install() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| format!("실행 파일 경로 확인 실패: {e}"))?;

        let unit = format!(
            r#"[Unit]
Description=keymander Daemon

[Service]
ExecStart={exe} start
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target"#,
            exe = exe.display(),
        );

        let path = service_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, unit).map_err(|e| e.to_string())?;

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", SERVICE_NAME])
            .output();

        Ok(format!("등록 위치: {}", path.display()))
    }

    pub fn uninstall() -> Result<(), String> {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", SERVICE_NAME])
            .output();

        let path = service_path();
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();

        Ok(())
    }

    pub fn is_installed() -> bool {
        service_path().exists()
    }
}

// ── 미지원 플랫폼 ──────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod platform {
    pub fn install() -> Result<String, String> {
        Err("이 플랫폼에서는 자동 시작이 지원되지 않습니다.".into())
    }

    pub fn uninstall() -> Result<(), String> {
        Ok(())
    }

    pub fn is_installed() -> bool {
        false
    }
}
