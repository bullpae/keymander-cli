//! kmd-daemon — keymander 백그라운드 데몬
//!
//! TCP localhost IPC 서버로 검색 엔진을 메모리에 상주시키고,
//! 글로벌 핫키와 키 바인딩을 관리한다.

mod autopilot;
mod autostart;
mod keybind;
mod server;

use color_eyre::Result;
use kmd_core::ipc;
use std::env;

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
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

fn autostart_label() -> &'static str {
    if autostart::is_installed() {
        "등록됨"
    } else {
        "미등록"
    }
}

fn send_shutdown() -> Result<()> {
    match ipc::send_request_result(&ipc::Request::Shutdown) {
        Ok(ipc::Response::Ok { message }) => println!("{message}"),
        Ok(ipc::Response::Error { message }) => eprintln!("에러: {message}"),
        Ok(other) => println!("응답: {other:?}"),
        Err(e) => eprintln!("{e}"),
    }
    Ok(())
}

fn send_status() -> Result<()> {
    match ipc::send_request_result(&ipc::Request::Status) {
        Ok(ipc::Response::Status {
            uptime_secs,
            index_items,
            pid,
            keymap_layers,
            config_error,
            hook_health,
        }) => {
            println!("데몬 상태: 실행 중");
            println!("  PID:        {pid}");
            println!("  가동 시간:  {uptime_secs}초");
            println!("  인덱스:     {index_items}개 항목");
            for layer in &keymap_layers {
                println!("  레이어:     {layer}");
            }
            if let Some(hook) = &hook_health {
                println!("  키보드 훅:  {hook}");
            }
            println!("  자동 시작:  {}", autostart_label());
            if let Some(err) = &config_error {
                println!("  ⚠ 설정:     {err}");
                println!("              → 데몬이 기본 설정으로 동작 중입니다. config.toml을 고친 뒤 재시작하세요.");
            }
        }
        Ok(other) => println!("응답: {other:?}"),
        Err(ipc::IpcError::Io(_)) => {
            println!("데몬이 실행 중이지 않습니다. (포트 파일은 존재하나 연결 실패)");
            println!("  자동 시작:  {}", autostart_label());
            ipc::cleanup_stale_files();
        }
        Err(_) => {
            println!("데몬이 실행 중이지 않습니다.");
            println!("  자동 시작:  {}", autostart_label());
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
