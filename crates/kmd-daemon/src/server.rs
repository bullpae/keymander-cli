//! TCP IPC 서버 — 검색 엔진 메모리 상주 + 요청 처리

use crate::keybind::{self, KeybindConfig};
use kmd_core::ipc::{self, Request, Response, SearchHit};
use kmd_core::{Config, Index, SearchEngine};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 데몬 메인 루프
pub fn run() -> color_eyre::Result<()> {
    let started_at = Instant::now();
    let shutdown = Arc::new(AtomicBool::new(false));

    // 설정 로드
    let config = load_config();
    tracing::info!("설정 로드 완료");

    // 키 바인딩 엔진 시작
    let mut kb_backend = keybind::create_backend();
    let kb_preset = resolve_keybind_preset(&config);
    match kb_backend.start(kb_preset) {
        Ok(()) => tracing::info!("키 바인딩 엔진 시작됨"),
        Err(e) => tracing::warn!("키 바인딩 시작 실패: {e}"),
    }

    // 인덱스 빌드 + 검색 엔진 초기화
    let index = Index::build(&config.launcher, config.general.emoji_icons);
    let item_count = index.items.len();
    tracing::info!("{item_count}개 항목으로 인덱스 빌드 완료");

    let engine = Arc::new(Mutex::new({
        let mut e = SearchEngine::new();
        e.set_kind_weights(config.launcher.kind_weights.clone());
        e.load(index.items);
        e
    }));

    // TCP 서버 시작 (OS가 빈 포트 자동 할당)
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    tracing::info!("IPC 서버 시작: 127.0.0.1:{port}");

    // 포트/PID 파일 기록
    write_runtime_files(port)?;

    listener.set_nonblocking(true)?;

    println!("kmd-daemon 실행 중 (port={port}, pid={})", std::process::id());

    // 메인 accept 루프 (non-blocking + poll)
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                let engine = engine.clone();
                let shutdown = shutdown.clone();
                let started_at = started_at;

                std::thread::spawn(move || {
                    if let Err(e) = handle_client(stream, &engine, &shutdown, started_at) {
                        tracing::warn!("클라이언트 처리 에러: {e}");
                    }
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                tracing::warn!("accept 에러: {e}");
            }
        }
    }

    // 키 바인딩 종료
    if let Err(e) = kb_backend.stop() {
        tracing::warn!("키 바인딩 종료 실패: {e}");
    }

    cleanup_runtime_files();
    tracing::info!("데몬 종료");
    Ok(())
}

fn handle_client(
    stream: TcpStream,
    engine: &Arc<Mutex<SearchEngine>>,
    shutdown: &Arc<AtomicBool>,
    started_at: Instant,
) -> color_eyre::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut reader = BufReader::new(&stream);
    let mut writer = &stream;

    let mut line = String::new();
    reader.read_line(&mut line)?;

    let request = ipc::decode_request(&line)?;
    let response = process_request(request, engine, shutdown, started_at);

    let encoded = ipc::encode_response(&response)?;
    writer.write_all(encoded.as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn process_request(
    request: Request,
    engine: &Arc<Mutex<SearchEngine>>,
    shutdown: &Arc<AtomicBool>,
    started_at: Instant,
) -> Response {
    match request {
        Request::Ping => Response::Pong,

        Request::Status => {
            let uptime = started_at.elapsed().as_secs();
            let items = engine.lock().map(|e| e.len()).unwrap_or(0);
            Response::Status {
                uptime_secs: uptime,
                index_items: items,
                pid: std::process::id(),
            }
        }

        Request::Search { query, limit } => {
            let results = match engine.lock() {
                Ok(mut e) => {
                    let (_mode, hits) = e.search(&query, limit);
                    hits
                }
                Err(_) => return Response::Error { message: "엔진 잠금 실패".into() },
            };

            let items: Vec<SearchHit> = results
                .into_iter()
                .map(|r| SearchHit {
                    name: r.item.name,
                    path: r.item.path,
                    kind: format!("{:?}", r.item.kind),
                    icon: r.item.icon,
                    score: r.score,
                })
                .collect();

            Response::SearchResults { items }
        }

        Request::RebuildIndex => {
            let config = load_config();
            let index = Index::build(&config.launcher, config.general.emoji_icons);
            let count = index.items.len();

            match engine.lock() {
                Ok(mut e) => {
                    e.set_kind_weights(config.launcher.kind_weights.clone());
                    e.load(index.items);
                }
                Err(_) => return Response::Error { message: "엔진 잠금 실패".into() },
            }

            Response::Ok {
                message: format!("인덱스 리빌드 완료 ({count}개 항목)"),
            }
        }

        Request::Shutdown => {
            shutdown.store(true, Ordering::Relaxed);
            Response::Ok {
                message: "데몬을 종료합니다.".into(),
            }
        }
    }
}

fn load_config() -> Config {
    let config_dir = Config::default_config_dir();
    Config::load(&config_dir).unwrap_or_default()
}

/// config의 keymap 설정에 따라 KeybindConfig 결정.
///
/// - "vim-nav": config 파일에 커스텀 설정이 있으면 그 값을 사용, 없으면 하드코딩 프리셋
/// - "minimal": config 파일 → 없으면 minimal 프리셋
/// - "none": 키 바인딩 비활성화
/// - 기타: TOML 커스텀 설정 시도 → 없으면 vim-nav 프리셋 폴백
fn resolve_keybind_preset(config: &Config) -> KeybindConfig {
    let profile = &config.launcher.keymap.active_profile;

    if profile.contains("none") {
        return KeybindConfig::empty();
    }

    if profile.contains("minimal") {
        return KeybindConfig::from_config(&config.launcher.keymap)
            .unwrap_or_else(KeybindConfig::minimal_preset);
    }

    // "vim-nav", "custom", 기타 모두: config 파일 값 우선, 없으면 프리셋 폴백
    KeybindConfig::from_config(&config.launcher.keymap)
        .unwrap_or_else(KeybindConfig::vim_nav_preset)
}

fn write_runtime_files(port: u16) -> color_eyre::Result<()> {
    let data_dir = Config::default_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    std::fs::write(ipc::port_file_path(), port.to_string())?;
    std::fs::write(ipc::pid_file_path(), std::process::id().to_string())?;
    Ok(())
}

fn cleanup_runtime_files() {
    ipc::cleanup_stale_files();
}
