//! `kmd daemon` — 백그라운드 데몬 관리 (start/stop/status)

use color_eyre::Result;
use kmd_core::ipc;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

pub enum Action {
    Start,
    Stop,
    Status,
    Install,
    Uninstall,
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::Start => start_daemon(),
        Action::Stop => send_command(ipc::Request::Shutdown, "stop"),
        Action::Status => check_status(),
        Action::Install => run_daemon_cmd("install"),
        Action::Uninstall => run_daemon_cmd("uninstall"),
    }
}

/// 데몬 프로세스를 백그라운드로 시작
fn start_daemon() -> Result<()> {
    // 이미 실행 중인지 확인
    if let Ok(port) = read_port() {
        if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            println!("데몬이 이미 실행 중입니다 (port={port}).");
            return Ok(());
        }
        // 연결 실패 → 오래된 파일 정리
        let _ = std::fs::remove_file(ipc::port_file_path());
        let _ = std::fs::remove_file(ipc::pid_file_path());
    }

    let daemon_exe = find_sibling_exe("kmd-daemon");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        std::process::Command::new(&daemon_exe)
            .arg("start")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new(&daemon_exe)
            .arg("start")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }

    // 데몬이 시작될 때까지 대기 (최대 5초)
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(port) = read_port() {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                println!("데몬 시작 완료 (port={port})");
                return Ok(());
            }
        }
    }

    println!("데몬 시작 중... (포트 파일이 아직 생성되지 않았습니다)");
    Ok(())
}

/// 데몬 상태 확인
fn check_status() -> Result<()> {
    let port = match read_port() {
        Ok(p) => p,
        Err(_) => {
            println!("데몬이 실행 중이지 않습니다.");
            return Ok(());
        }
    };

    match connect_and_send(port, &ipc::Request::Status) {
        Ok(resp) => match resp {
            ipc::Response::Status {
                uptime_secs,
                index_items,
                pid,
            } => {
                println!("데몬 상태: 실행 중");
                println!("  PID:        {pid}");
                println!("  가동 시간:  {uptime_secs}초");
                println!("  인덱스:     {index_items}개 항목");
            }
            _ => println!("예기치 않은 응답"),
        },
        Err(_) => {
            println!("데몬이 실행 중이지 않습니다. (포트 파일은 존재하나 연결 실패)");
            let _ = std::fs::remove_file(ipc::port_file_path());
            let _ = std::fs::remove_file(ipc::pid_file_path());
        }
    }
    Ok(())
}

/// 데몬에 명령 전송
fn send_command(request: ipc::Request, action_name: &str) -> Result<()> {
    let port = match read_port() {
        Ok(p) => p,
        Err(_) => {
            println!("데몬이 실행 중이지 않습니다.");
            return Ok(());
        }
    };

    match connect_and_send(port, &request) {
        Ok(resp) => match resp {
            ipc::Response::Ok { message } => println!("{message}"),
            ipc::Response::Error { message } => eprintln!("에러: {message}"),
            _ => println!("{action_name} 완료"),
        },
        Err(e) => {
            eprintln!("데몬 연결 실패: {e}");
            let _ = std::fs::remove_file(ipc::port_file_path());
            let _ = std::fs::remove_file(ipc::pid_file_path());
        }
    }
    Ok(())
}

fn connect_and_send(port: u16, request: &ipc::Request) -> Result<ipc::Response> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    let encoded = ipc::encode_request(request)?;
    stream.write_all(encoded.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let resp = ipc::decode_response(&line)?;
    Ok(resp)
}

fn read_port() -> Result<u16> {
    let path = ipc::port_file_path();
    let content = std::fs::read_to_string(&path)?;
    let port: u16 = content.trim().parse()?;
    Ok(port)
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

/// 같은 디렉토리 또는 PATH에서 형제 바이너리 찾기
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

/// kmd-daemon 바이너리에 명령 위임 (install/uninstall 등)
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

