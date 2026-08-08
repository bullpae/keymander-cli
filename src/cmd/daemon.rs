//! `kmd daemon` — 백그라운드 데몬 관리 (start/stop/status)

use color_eyre::Result;
use kmd_core::ipc;
use std::net::TcpStream;

pub enum Action {
    Start,
    Stop,
    Status,
    Install,
    Uninstall,
    PasteTest { delay_ms: u64 },
    ClipTest { slot: usize, delay_ms: u64 },
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::Start => start_daemon(),
        Action::Stop => send_command(ipc::Request::Shutdown, "stop"),
        Action::Status => check_status(),
        Action::Install => run_daemon_cmd("install"),
        Action::Uninstall => run_daemon_cmd("uninstall"),
        Action::PasteTest { delay_ms } => paste_test(delay_ms),
        Action::ClipTest { slot, delay_ms } => clip_test(slot, delay_ms),
    }
}

/// [P1 검증] 지연 후 데몬에 클립보드 슬롯 붙여넣기를 요청한다.
fn clip_test(slot: usize, delay_ms: u64) -> Result<()> {
    println!(
        "{delay_ms}ms 후 클립보드 슬롯 {slot}을 전경 앱에 붙여넣습니다 — 대상 앱으로 전환하세요..."
    );
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    send_command(ipc::Request::ClipPaste { slot }, "clip-test")
}

/// [P3 스파이크] 지연 후 데몬에 붙여넣기 주입을 요청한다.
/// 지연 동안 사용자가 붙여넣을 대상 앱으로 포커스를 옮긴다.
fn paste_test(delay_ms: u64) -> Result<()> {
    println!("{delay_ms}ms 후 전경 앱에 붙여넣기를 주입합니다 — 지금 대상 앱으로 전환하세요...");
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    send_command(ipc::Request::InjectPaste, "paste-test")
}

/// 데몬 로그 파일 경로 (런타임 디렉터리 아래, 시작마다 새로 씀)
fn daemon_log_path() -> std::path::PathBuf {
    ipc::log_file_path()
}

/// 데몬 stdout/stderr 리다이렉트 대상. 로그 파일 생성 실패 시 null 폴백.
/// 이전에는 무조건 null이어서 키맵 파싱 경고 등 진단 정보가 전부 버려졌다.
fn daemon_log_stdio() -> (std::process::Stdio, std::process::Stdio) {
    let path = daemon_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = std::fs::File::create(&path) {
        if let Ok(clone) = file.try_clone() {
            return (file.into(), clone.into());
        }
    }
    (std::process::Stdio::null(), std::process::Stdio::null())
}

fn start_daemon() -> Result<()> {
    if let Ok(port) = ipc::read_port() {
        if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            println!("데몬이 이미 실행 중입니다 (port={port}).");
            return Ok(());
        }
        ipc::cleanup_stale_files();
    }

    let daemon_exe = find_sibling_exe("kmd-daemon");
    let (log_out, log_err) = daemon_log_stdio();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        std::process::Command::new(&daemon_exe)
            .arg("start")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(log_out)
            .stderr(log_err)
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new(&daemon_exe)
            .arg("start")
            .stdin(std::process::Stdio::null())
            .stdout(log_out)
            .stderr(log_err)
            .spawn()?;
    }

    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(port) = ipc::read_port() {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                println!("데몬 시작 완료 (port={port})");
                return Ok(());
            }
        }
    }

    println!("데몬 시작 중... (포트 파일이 아직 생성되지 않았습니다)");
    Ok(())
}

fn check_status() -> Result<()> {
    match ipc::send_request_result(&ipc::Request::Status) {
        Ok(ipc::Response::Status {
            uptime_secs,
            index_items,
            pid,
            keymap_layers,
            config_error,
            keybind_error,
            hook_health,
        }) => {
            println!("데몬 상태: 실행 중");
            println!("  PID:        {pid}");
            println!("  가동 시간:  {uptime_secs}초");
            println!("  인덱스:     {index_items}개 항목");
            // 훅이 죽었으면 레이어 목록은 config에서 온 설정값일 뿐 실제 동작이
            // 아니다 — 오해하지 않도록 먼저 못 박는다.
            if keybind_error.is_some() {
                println!("  키 훅:      ❌ 미동작 (아래 키 입력이 가로채지지 않습니다)");
            }
            for layer in &keymap_layers {
                println!("  레이어:     {layer}");
            }
            if let Some(hook) = &hook_health {
                println!("  키보드 훅:  {hook}");
            }
            println!("  로그:       {}", daemon_log_path().display());
            if let Some(err) = &keybind_error {
                println!("  ⚠ 키 훅:    {err}");
            }
            if let Some(err) = &config_error {
                println!("  ⚠ 설정:     {err}");
                println!("              → 데몬이 기본 설정으로 동작 중입니다. config.toml을 고친 뒤 재시작하세요.");
            }
        }
        Ok(_) => println!("예기치 않은 응답"),
        Err(ipc::IpcError::Io(_)) => {
            println!("데몬이 실행 중이지 않습니다. (포트 파일은 존재하나 연결 실패)");
            ipc::cleanup_stale_files();
        }
        Err(_) => {
            println!("데몬이 실행 중이지 않습니다.");
        }
    }
    Ok(())
}

fn send_command(request: ipc::Request, _action_name: &str) -> Result<()> {
    let is_shutdown = matches!(request, ipc::Request::Shutdown);
    match ipc::send_request_result(&request) {
        Ok(ipc::Response::Ok { message }) => {
            println!("{message}");
            if is_shutdown {
                wait_for_daemon_exit();
            }
        }
        Ok(ipc::Response::Error { message }) => eprintln!("에러: {message}"),
        Ok(_) => println!("완료"),
        Err(ipc::IpcError::Io(_)) => {
            println!("데몬이 이미 종료되었거나 종료 중입니다.");
            ipc::cleanup_stale_files();
        }
        Err(ipc::IpcError::NoDaemon) => {
            println!("데몬이 실행 중이지 않습니다.");
        }
        Err(e) => eprintln!("{e}"),
    }
    Ok(())
}

/// Shutdown 응답 후 데몬이 실제 종료될 때까지 대기 (최대 3초)
fn wait_for_daemon_exit() {
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        match ipc::read_port() {
            Err(_) => return,
            Ok(port) => {
                if TcpStream::connect(format!("127.0.0.1:{port}")).is_err() {
                    ipc::cleanup_stale_files();
                    return;
                }
            }
        }
    }
    ipc::cleanup_stale_files();
}

/// kmd-desktop 바이너리를 실행
pub fn launch_desktop() -> Result<()> {
    let desktop_exe = find_sibling_exe("kmd-desktop");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;

        std::process::Command::new(&desktop_exe)
            .creation_flags(DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new(&desktop_exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }

    Ok(())
}

fn find_sibling_exe(name: &str) -> std::path::PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();

    #[cfg(windows)]
    let bin_name = format!("{name}.exe");
    #[cfg(not(windows))]
    let bin_name = name.to_string();

    let same_dir = exe_dir.join(&bin_name);
    if same_dir.exists() {
        return same_dir;
    }

    std::path::PathBuf::from(bin_name)
}

fn run_daemon_cmd(cmd: &str) -> Result<()> {
    let daemon_exe = find_sibling_exe("kmd-daemon");
    let output = std::process::Command::new(&daemon_exe)
        .arg(cmd)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match output {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            std::process::exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("kmd-daemon 실행 실패: {e}");
            eprintln!("경로: {}", daemon_exe.display());
            std::process::exit(1);
        }
    }
}
