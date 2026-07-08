//! Built-in shell command extension
//!
//! Activated with `!` prefix — executes shell commands and shows output.
//! Also provides quick system-info actions.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{Extension, ExtensionAction};
use crate::index::{IndexItem, ItemKind, Source};

/// 셸 명령 최대 실행 시간 — `!ping -t` 같은 무한 명령이 런처를 멈추지 않도록.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// 캡처할 최대 출력 크기 (초과분은 버림)
const MAX_CAPTURE_BYTES: usize = 256 * 1024;

/// 파이프를 끝까지 읽되 MAX_CAPTURE_BYTES까지만 보관하는 리더 스레드.
/// cap 초과분도 계속 읽어 버린다 — 읽기를 멈추면 자식이 파이프 블로킹으로
/// 종료하지 못한다.
fn spawn_capped_reader<R: Read + Send + 'static>(
    pipe: Option<R>,
) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut captured = Vec::new();
        if let Some(mut pipe) = pipe {
            let mut buf = [0u8; 8192];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if captured.len() < MAX_CAPTURE_BYTES {
                            let take = n.min(MAX_CAPTURE_BYTES - captured.len());
                            captured.extend_from_slice(&buf[..take]);
                        }
                    }
                }
            }
        }
        String::from_utf8_lossy(&captured).trim().to_string()
    })
}

/// 타임아웃 시 프로세스 트리 전체를 종료.
/// Windows에서 child.kill()은 cmd.exe만 죽이고 손자 프로세스(ping 등)는
/// 남아 파이프를 계속 쥐고 있으므로 taskkill /T 로 트리를 정리한다.
fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// 명령을 타임아웃/출력 상한과 함께 실행하고 (성공 여부, stdout, stderr) 반환
fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<(bool, String, String, Option<i32>), String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to execute: {}", e))?;
    let stdout_reader = spawn_capped_reader(child.stdout.take());
    let stderr_reader = spawn_capped_reader(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_process_tree(&mut child);
                    // 트리 종료 후 리더 스레드는 EOF로 곧 끝난다
                    let partial = stdout_reader.join().unwrap_or_default();
                    let _ = stderr_reader.join();
                    let mut msg = format!("Timed out after {:.1}s", timeout.as_secs_f32());
                    if !partial.is_empty() {
                        msg.push_str(" — partial output:\n");
                        msg.push_str(&partial);
                    }
                    return Err(msg);
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                kill_process_tree(&mut child);
                return Err(format!("Failed to wait: {}", e));
            }
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    Ok((status.success(), stdout, stderr, status.code()))
}

/// Quick action: a pre-defined system info command
struct QuickAction {
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    #[cfg(windows)]
    command: &'static str,
    #[cfg(windows)]
    args: &'static [&'static str],
    #[cfg(not(windows))]
    command: &'static str,
    #[cfg(not(windows))]
    args: &'static [&'static str],
}

/// Pre-defined quick actions for system information
static QUICK_ACTIONS: &[QuickAction] = &[
    QuickAction {
        name: "IP Address",
        description: "Show network IP addresses",
        icon: "\u{1F310}", // 🌐
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch 'Loopback' } | Select-Object -ExpandProperty IPAddress) -join \", \""],
        #[cfg(not(windows))]
        command: "sh",
        #[cfg(not(windows))]
        args: &["-c", "ip -4 addr show 2>/dev/null | grep -oP 'inet \\K[\\d.]+' | grep -v '^127\\.' | head -5 || ifconfig 2>/dev/null | grep 'inet ' | grep -v '127.0.0.1' | awk '{print $2}' | head -5 || hostname -I 2>/dev/null"],
    },
    QuickAction {
        name: "Hostname",
        description: "Show computer name",
        icon: "\u{1F4BB}", // 💻
        #[cfg(windows)]
        command: "hostname",
        #[cfg(windows)]
        args: &[],
        #[cfg(not(windows))]
        command: "hostname",
        #[cfg(not(windows))]
        args: &[],
    },
    QuickAction {
        name: "Uptime",
        description: "Show system uptime",
        icon: "\u{23F1}\u{FE0F}", // ⏱️
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "$os = Get-CimInstance Win32_OperatingSystem; $up = (Get-Date) - $os.LastBootUpTime; \"{0}d {1}h {2}m\" -f $up.Days, $up.Hours, $up.Minutes"],
        #[cfg(not(windows))]
        command: "uptime",
        #[cfg(not(windows))]
        args: &["-p"],
    },
    QuickAction {
        name: "Disk Usage",
        description: "Show disk space usage",
        icon: "\u{1F4BE}", // 💾
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "Get-PSDrive -PSProvider FileSystem | ForEach-Object { \"{0}: {1:N1}GB free / {2:N1}GB\" -f $_.Name, ($_.Free/1GB), (($_.Used+$_.Free)/1GB) }"],
        #[cfg(not(windows))]
        command: "df",
        #[cfg(not(windows))]
        args: &["-h", "--total"],
    },
    QuickAction {
        name: "Memory Usage",
        description: "Show memory usage",
        icon: "\u{1F9E0}", // 🧠
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "$os = Get-CimInstance Win32_OperatingSystem; $total = [math]::Round($os.TotalVisibleMemorySize/1MB,1); $free = [math]::Round($os.FreePhysicalMemory/1MB,1); $used = $total - $free; \"Used: {0}GB / Total: {1}GB ({2}%)\" -f $used, $total, [math]::Round($used/$total*100)"],
        #[cfg(not(windows))]
        command: "free",
        #[cfg(not(windows))]
        args: &["-h"],
    },
    QuickAction {
        name: "Public IP",
        description: "Show public IP address",
        icon: "\u{1F30D}", // 🌍
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "(Invoke-WebRequest -Uri 'https://api.ipify.org' -UseBasicParsing -TimeoutSec 5).Content"],
        #[cfg(not(windows))]
        command: "curl",
        #[cfg(not(windows))]
        args: &["-s", "--max-time", "5", "https://api.ipify.org"],
    },
    QuickAction {
        name: "OS Version",
        description: "Show operating system version",
        icon: "\u{2699}\u{FE0F}", // ⚙️
        #[cfg(windows)]
        command: "powershell",
        #[cfg(windows)]
        args: &["-NoProfile", "-Command",
            "(Get-CimInstance Win32_OperatingSystem).Caption + ' ' + (Get-CimInstance Win32_OperatingSystem).Version"],
        #[cfg(not(windows))]
        command: "uname",
        #[cfg(not(windows))]
        args: &["-srm"],
    },
    QuickAction {
        name: "User Name",
        description: "Show current user",
        icon: "\u{1F464}", // 👤
        #[cfg(windows)]
        command: "whoami",
        #[cfg(windows)]
        args: &[],
        #[cfg(not(windows))]
        command: "whoami",
        #[cfg(not(windows))]
        args: &[],
    },
];

/// Windows에서 콘솔 창 없이 cmd 실행하는 헬퍼
#[cfg(target_os = "windows")]
fn hidden_cmd() -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("cmd");
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[cfg(not(target_os = "windows"))]
fn hidden_cmd() -> Command {
    Command::new("cmd")
}

pub struct ShellExtension;

impl ShellExtension {
    /// Execute a shell command and capture its output
    pub fn execute_command(cmd_line: &str) -> Result<String, String> {
        let cmd_line = cmd_line.trim();
        if cmd_line.is_empty() {
            return Err("Empty command".to_string());
        }

        let cmd = if cfg!(target_os = "windows") {
            let mut c = hidden_cmd();
            c.args(["/c", cmd_line]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", cmd_line]);
            c
        };

        let (success, stdout, stderr, code) = run_with_timeout(cmd, COMMAND_TIMEOUT)?;

        if success {
            if stdout.is_empty() {
                Ok("(no output)".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            let msg = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Exit code: {}", code.unwrap_or(-1))
            };
            Err(msg)
        }
    }

    /// Quick action 이름인지 확인
    pub fn is_quick_action(name: &str) -> bool {
        QUICK_ACTIONS
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// Execute a quick action by name
    pub fn execute_quick_action(name: &str) -> Result<String, String> {
        let action = QUICK_ACTIONS
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Unknown quick action: {}", name))?;

        let mut cmd = Command::new(action.command);
        cmd.args(action.args);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let (_success, stdout, _stderr, _code) = run_with_timeout(cmd, COMMAND_TIMEOUT)?;

        if stdout.is_empty() {
            Ok("(no output)".to_string())
        } else {
            Ok(stdout)
        }
    }

    /// List quick actions, optionally filtered
    fn list_quick_actions(filter: &str) -> Vec<IndexItem> {
        let filter_lower = filter.to_lowercase();
        QUICK_ACTIONS
            .iter()
            .filter(|a| {
                filter.is_empty()
                    || a.name.to_lowercase().contains(&filter_lower)
                    || a.description.to_lowercase().contains(&filter_lower)
            })
            .map(|a| IndexItem {
                name: format!("{} {}", a.icon, a.name),
                path: a.name.to_string(), // used as key for execute_quick_action
                kind: ItemKind::Shell,
                source: Source::Plugin,
                icon: a.icon.to_string(),
                keywords: a.description.to_string(),
                icon_path: None,
            })
            .collect()
    }
}

impl Extension for ShellExtension {
    fn name(&self) -> &str {
        "Shell"
    }

    fn prefix(&self) -> Option<&str> {
        Some("!")
    }

    fn search(&self, query: &str) -> Vec<IndexItem> {
        let query = query.trim();

        if query.is_empty() {
            // Show quick actions when just "!" is typed
            return Self::list_quick_actions("");
        }

        // Check if it's a quick-action filter
        let mut results = Self::list_quick_actions(query);

        // Also show the raw command as an option to execute
        results.push(IndexItem {
            name: format!("\u{1F4DF} Run: {}", query), // 📟
            path: query.to_string(),
            kind: ItemKind::Shell,
            source: Source::Plugin,
            icon: "\u{1F4DF}".to_string(), // 📟
            keywords: format!("shell execute run command {}", query),
            icon_path: None,
        });

        results
    }

    fn execute(&self, item: &IndexItem) -> ExtensionAction {
        // If path matches a quick action name, execute it
        if QUICK_ACTIONS.iter().any(|a| a.name == item.path) {
            match Self::execute_quick_action(&item.path) {
                Ok(output) => ExtensionAction::CopyToClipboard(output),
                Err(e) => ExtensionAction::Display(format!("Error: {}", e)),
            }
        } else {
            // Execute as raw shell command
            match Self::execute_command(&item.path) {
                Ok(output) => ExtensionAction::CopyToClipboard(output),
                Err(e) => ExtensionAction::Display(format!("Error: {}", e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_actions_list() {
        let actions = ShellExtension::list_quick_actions("");
        assert!(
            actions.len() >= 5,
            "Expected at least 5 quick actions, got {}",
            actions.len()
        );
    }

    #[test]
    fn test_quick_actions_filter() {
        let actions = ShellExtension::list_quick_actions("ip");
        assert!(
            !actions.is_empty(),
            "Expected at least 1 result for 'ip' filter"
        );
    }

    #[test]
    fn test_search_empty_shows_quick_actions() {
        let ext = ShellExtension;
        let results = ext.search("");
        assert!(results.len() >= 5);
    }

    #[test]
    fn test_search_command_appends_run() {
        let ext = ShellExtension;
        let results = ext.search("echo hello");
        // Last result should be "Run: echo hello"
        let last = results.last().unwrap();
        assert!(last.name.contains("Run:"));
        assert_eq!(last.path, "echo hello");
    }

    #[test]
    fn test_execute_simple_command() {
        let result = ShellExtension::execute_command("echo test123");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test123"));
    }

    #[test]
    fn test_execute_hostname() {
        let result = ShellExtension::execute_quick_action("Hostname");
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_run_with_timeout_kills_hanging_command() {
        // 종료되지 않는 명령이 타임아웃으로 중단되는지 확인
        let cmd = if cfg!(target_os = "windows") {
            let mut c = hidden_cmd();
            c.args(["/c", "ping -n 60 127.0.0.1"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "sleep 60"]);
            c
        };

        let start = Instant::now();
        let result = run_with_timeout(cmd, Duration::from_millis(500));
        let elapsed = start.elapsed();

        assert!(result.is_err(), "타임아웃 시 Err 반환");
        assert!(
            result.unwrap_err().contains("Timed out"),
            "타임아웃 메시지 포함"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "타임아웃(0.5s) 부근에서 반환되어야 함: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_run_with_timeout_normal_completion() {
        let cmd = if cfg!(target_os = "windows") {
            let mut c = hidden_cmd();
            c.args(["/c", "echo done123"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "echo done123"]);
            c
        };

        let (success, stdout, _stderr, _code) =
            run_with_timeout(cmd, Duration::from_secs(10)).unwrap();
        assert!(success);
        assert!(stdout.contains("done123"));
    }
}
