//! kmd-daemon — keymander 백그라운드 데몬
//!
//! TCP localhost IPC 서버로 검색 엔진을 메모리에 상주시키고,
//! 글로벌 핫키와 키 바인딩을 관리한다.

mod autostart;
mod keybind;
mod server;

use color_eyre::Result;
use std::env;

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("start");

    match command {
        "start" => server::run()?,
        "stop" => send_shutdown()?,
        "status" => send_status()?,
        "install" => cmd_install()?,
        "uninstall" => cmd_uninstall()?,
        other => {
            eprintln!("알 수 없는 명령: {other}");
            eprintln!("사용법: kmd-daemon [start|stop|status|install|uninstall]");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// 실행 중인 데몬에 Shutdown 요청 전송
fn send_shutdown() -> Result<()> {
    use kmd_core::ipc;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let port = read_port_file()?;
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    let req = ipc::encode_request(&ipc::Request::Shutdown)?;
    stream.write_all(req.as_bytes())?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp = ipc::decode_response(&line)?;

    match resp {
        ipc::Response::Ok { message } => println!("{message}"),
        ipc::Response::Error { message } => eprintln!("에러: {message}"),
        _ => println!("응답: {line}"),
    }
    Ok(())
}

/// 실행 중인 데몬에 Status 요청 전송
fn send_status() -> Result<()> {
    use kmd_core::ipc;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let port = match read_port_file() {
        Ok(p) => p,
        Err(_) => {
            let auto = if autostart::is_installed() { "등록됨" } else { "미등록" };
            println!("데몬이 실행 중이지 않습니다.");
            println!("  자동 시작:  {auto}");
            return Ok(());
        }
    };

    match TcpStream::connect(format!("127.0.0.1:{port}")) {
        Ok(mut stream) => {
            let req = ipc::encode_request(&ipc::Request::Status)?;
            stream.write_all(req.as_bytes())?;

            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let resp = ipc::decode_response(&line)?;

            match resp {
                ipc::Response::Status {
                    uptime_secs,
                    index_items,
                    pid,
                } => {
                    let auto = if autostart::is_installed() { "등록됨" } else { "미등록" };
                    println!("데몬 상태: 실행 중");
                    println!("  PID:        {pid}");
                    println!("  가동 시간:  {uptime_secs}초");
                    println!("  인덱스:     {index_items}개 항목");
                    println!("  자동 시작:  {auto}");
                }
                _ => println!("응답: {line}"),
            }
        }
        Err(_) => {
            let auto = if autostart::is_installed() { "등록됨" } else { "미등록" };
            println!("데몬이 실행 중이지 않습니다. (포트 파일은 존재하나 연결 실패)");
            println!("  자동 시작:  {auto}");
            let _ = std::fs::remove_file(ipc::port_file_path());
            let _ = std::fs::remove_file(ipc::pid_file_path());
        }
    }
    Ok(())
}

/// 부팅 시 자동 시작 등록
fn cmd_install() -> Result<()> {
    match autostart::install() {
        Ok(detail) => {
            println!("자동 시작 등록 완료");
            println!("  {detail}");
            println!("  다음 로그인부터 자동으로 데몬이 시작됩니다.");
        }
        Err(e) => eprintln!("자동 시작 등록 실패: {e}"),
    }
    Ok(())
}

/// 부팅 시 자동 시작 해제
fn cmd_uninstall() -> Result<()> {
    match autostart::uninstall() {
        Ok(()) => println!("자동 시작 해제 완료"),
        Err(e) => eprintln!("자동 시작 해제 실패: {e}"),
    }
    Ok(())
}

fn read_port_file() -> Result<u16> {
    let path = kmd_core::ipc::port_file_path();
    let content = std::fs::read_to_string(&path)
        .map_err(|_| color_eyre::eyre::eyre!("포트 파일을 찾을 수 없습니다: {}", path.display()))?;
    let port: u16 = content
        .trim()
        .parse()
        .map_err(|_| color_eyre::eyre::eyre!("잘못된 포트 번호: {content}"))?;
    Ok(port)
}
