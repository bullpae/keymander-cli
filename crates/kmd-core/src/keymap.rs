//! Keymap backend integration (prototype).
//!
//! Current MVP supports Kanata process management and profile files.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Config;

fn state_dir() -> PathBuf {
    Config::default_data_dir().join("keymap")
}

fn pid_file() -> PathBuf {
    state_dir().join("kanata.pid")
}

fn default_profile_dir() -> PathBuf {
    Config::default_config_dir().join("keymap")
}

pub fn profile_dir(config: &Config) -> PathBuf {
    config
        .launcher
        .keymap
        .profile_dir
        .clone()
        .unwrap_or_else(default_profile_dir)
}

pub fn active_profile_path(config: &Config) -> PathBuf {
    profile_dir(config).join(&config.launcher.keymap.active_profile)
}

fn kanata_command(config: &Config) -> String {
    config
        .launcher
        .keymap
        .kanata_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kanata".to_string())
}

fn ensure_state_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir())
}

fn read_pid() -> Option<u32> {
    let path = pid_file();
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse::<u32>().ok()
}

fn write_pid(pid: u32) -> std::io::Result<()> {
    ensure_state_dir()?;
    std::fs::write(pid_file(), pid.to_string())
}

fn clear_pid() {
    let _ = std::fs::remove_file(pid_file());
}

fn is_pid_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            return text.contains(&pid.to_string());
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

pub fn status() -> String {
    if let Some(pid) = read_pid() {
        if is_pid_running(pid) {
            format!("running (pid={pid})")
        } else {
            "stale pid file (process not running)".to_string()
        }
    } else {
        "stopped".to_string()
    }
}

pub fn list_profiles(config: &Config) -> Vec<String> {
    let dir = profile_dir(config);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("kbd") {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

pub fn create_profile_template(config: &Config, file_name: &str) -> Result<PathBuf, String> {
    let dir = profile_dir(config);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = if file_name.ends_with(".kbd") {
        file_name.to_string()
    } else {
        format!("{file_name}.kbd")
    };
    let path = dir.join(name);
    if path.exists() {
        return Ok(path);
    }
    let template = r#"(defsrc
  caps a s d f j k l ;
)

(deflayer base
  esc  a s d f j k l ;
)
"#;
    std::fs::write(&path, template).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn start(config: &Config) -> Result<String, String> {
    if !config
        .launcher
        .keymap
        .backend
        .eq_ignore_ascii_case("kanata")
    {
        return Err("지원하지 않는 keymap backend 입니다. (kanata 만 지원)".to_string());
    }
    if let Some(pid) = read_pid() {
        if is_pid_running(pid) {
            return Err(format!("이미 실행 중입니다. pid={pid}"));
        }
    }

    let profile = active_profile_path(config);
    if !profile.exists() {
        return Err(format!(
            "활성 프로파일이 없습니다: {}\n먼저 `kmd keymap init` 또는 `kmd keymap use <profile>` 를 실행하세요.",
            profile.display()
        ));
    }

    let exe = kanata_command(config);
    let mut cmd = Command::new(exe);
    cmd.args(["--cfg", &profile.to_string_lossy()]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    write_pid(pid).map_err(|e| e.to_string())?;
    Ok(format!(
        "kanata 시작됨 (pid={pid}, profile={})",
        profile.to_string_lossy()
    ))
}

pub fn stop() -> Result<String, String> {
    let Some(pid) = read_pid() else {
        return Ok("실행 중인 keymap 프로세스가 없습니다.".to_string());
    };

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|e| e.to_string())?;
        clear_pid();
        if status.success() {
            return Ok(format!("kanata 중지 완료 (pid={pid})"));
        }
        Err(format!("kanata 중지 실패 (pid={pid})"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| e.to_string())?;
        clear_pid();
        if status.success() {
            return Ok(format!("kanata 중지 완료 (pid={pid})"));
        }
        Err(format!("kanata 중지 실패 (pid={pid})"))
    }
}

pub fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("프로파일 이름이 비어 있습니다.".to_string());
    }
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("프로파일 이름에 사용할 수 없는 문자가 포함되어 있습니다.".to_string());
    }
    Ok(())
}

pub fn with_extension(name: &str) -> String {
    if name.ends_with(".kbd") {
        name.to_string()
    } else {
        format!("{name}.kbd")
    }
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}
