//! TCP IPC 서버 — 검색 엔진 메모리 상주 + 요청 처리

use crate::autostart;
use crate::keybind::{self, KeybindConfig};
use kmd_core::ipc::{self, Request, Response, SearchHit};
use kmd_core::{Config, Index, SearchEngine};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

/// 데몬 메인 루프
pub fn run() -> color_eyre::Result<()> {
    let started_at = Instant::now();
    let shutdown = Arc::new(AtomicBool::new(false));
    // Shutdown 요청 → 채널로 메인 스레드를 즉시 깨운다 (200ms 폴링 제거)
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    // 설정 로드 (성공/폴백 로그는 load_config 내부에서)
    let config = load_config();

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

    // 인증 토큰 생성 후 포트/PID 파일 기록 (daemon.port: line1=port, line2=token)
    let token = Arc::new(ipc::generate_token());
    write_runtime_files(port, &token)?;

    println!(
        "kmd-daemon 실행 중 (port={port}, pid={})",
        std::process::id()
    );

    // accept 루프를 별도 스레드에서 blocking 모드로 실행 — 50ms 바쁜 대기 제거.
    // 종료 시 `listener`를 닫으면 accept()가 에러를 반환하며 루프가 자연스럽게 종료된다.
    let accept_shutdown = shutdown.clone();
    let accept_token = token.clone();
    let accept_tx = shutdown_tx.clone();
    let accept_handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if accept_shutdown.load(Ordering::Relaxed) {
                break;
            }
            match stream {
                Ok(stream) => {
                    let engine = engine.clone();
                    let conn_tx = accept_tx.clone();
                    let conn_token = accept_token.clone();
                    std::thread::spawn(move || {
                        if let Err(e) =
                            handle_client(stream, &engine, &conn_tx, started_at, &conn_token)
                        {
                            tracing::warn!("클라이언트 처리 에러: {e}");
                        }
                    });
                }
                Err(e) => {
                    if accept_shutdown.load(Ordering::Relaxed) {
                        break; // 정상 종료 신호
                    }
                    tracing::warn!("accept 에러: {e}");
                }
            }
        }
    });

    // 종료 신호가 올 때까지 블로킹 대기 (Shutdown 요청 시 send로 깨어남)
    let _ = shutdown_rx.recv();
    shutdown.store(true, Ordering::Relaxed);

    // accept 스레드는 accept()에서 블로킹 중이므로 self-connect로 깨운다.
    // 기존에는 클라이언트(kmd daemon stop)의 폴링 connect가 우연히 깨워줄
    // 때만 종료가 완료됐다 — 다른 경로의 Shutdown 요청 시 영구 대기했음.
    let _ = TcpStream::connect(("127.0.0.1", port));

    let _ = accept_handle.join();

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
    shutdown_tx: &mpsc::Sender<()>,
    started_at: Instant,
    token: &str,
) -> color_eyre::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;

    // 요청 크기 상한 — 개행 없는 대용량 스트림으로 인한 메모리 소진 방지
    const MAX_REQUEST_BYTES: u64 = 64 * 1024;
    let mut reader = BufReader::new(std::io::Read::take(&stream, MAX_REQUEST_BYTES));
    let mut writer = &stream;

    // line 1: 인증 토큰. 불일치 시 응답 없이 연결 종료.
    let mut token_line = String::new();
    reader.read_line(&mut token_line)?;
    if !ipc::constant_time_eq(token_line.trim().as_bytes(), token.as_bytes()) {
        tracing::warn!("IPC 인증 실패 — 연결 종료");
        return Ok(());
    }

    // line 2: JSON 요청
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let request = ipc::decode_request(&line)?;
    let response = process_request(request, engine, shutdown_tx, started_at);

    let encoded = ipc::encode_response(&response)?;
    writer.write_all(encoded.as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn process_request(
    request: Request,
    engine: &Arc<Mutex<SearchEngine>>,
    shutdown_tx: &mpsc::Sender<()>,
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
                keymap_layers: KEYMAP_SUMMARY.lock().map(|g| g.clone()).unwrap_or_default(),
                config_error: CONFIG_LOAD_ERROR.lock().map(|g| g.clone()).unwrap_or(None),
            }
        }

        Request::AutostartStatus => Response::AutostartStatus {
            installed: autostart::is_installed(),
        },

        Request::AutostartEnable => match autostart::install() {
            Ok(detail) => Response::Ok {
                message: format!("자동 시작 등록 완료 ({detail})"),
            },
            Err(e) => Response::Error {
                message: format!("자동 시작 등록 실패: {e}"),
            },
        },

        Request::AutostartDisable => match autostart::uninstall() {
            Ok(()) => Response::Ok {
                message: "자동 시작 해제 완료".into(),
            },
            Err(e) => Response::Error {
                message: format!("자동 시작 해제 실패: {e}"),
            },
        },

        Request::Search { query, limit } => {
            let results = match engine.lock() {
                Ok(mut e) => {
                    let (_mode, hits) = e.search(&query, limit);
                    hits
                }
                Err(_) => {
                    return Response::Error {
                        message: "엔진 잠금 실패".into(),
                    }
                }
            };

            let items: Vec<SearchHit> = results
                .into_iter()
                .map(|r| SearchHit {
                    name: r.item.name,
                    path: r.item.path,
                    kind: r.item.kind.to_string(),
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
                Err(_) => {
                    return Response::Error {
                        message: "엔진 잠금 실패".into(),
                    }
                }
            }

            Response::Ok {
                message: format!("인덱스 리빌드 완료 ({count}개 항목)"),
            }
        }

        Request::Shutdown => {
            let _ = shutdown_tx.send(());
            Response::Ok {
                message: "데몬을 종료합니다.".into(),
            }
        }
    }
}

/// config.toml 로드 실패 메시지 — `kmd daemon status`로 노출한다.
/// TOML 문법 오류(테이블 중복 정의 등)가 조용히 기본값 폴백되면 사용자는
/// 설정이 반영된 줄 알게 되므로, 에러를 상태 조회에서 바로 볼 수 있어야 한다.
static CONFIG_LOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn load_config() -> Config {
    let config_dir = Config::default_config_dir();
    match Config::load(&config_dir) {
        Ok(config) => {
            if let Ok(mut g) = CONFIG_LOAD_ERROR.lock() {
                *g = None;
            }
            tracing::info!("설정 로드 완료: {}", config_dir.display());
            config
        }
        Err(e) => {
            // 데몬은 계속 떠 있어야 하므로 기본값으로 폴백하되 에러를 명확히 남긴다
            let msg = format!("{} 로드 실패: {e}", config_dir.display());
            tracing::error!("{msg} — 기본 설정으로 동작합니다");
            if let Ok(mut g) = CONFIG_LOAD_ERROR.lock() {
                *g = Some(msg);
            }
            Config::default()
        }
    }
}

/// 실행 중인 키맵 레이어 요약 — `kmd daemon status`로 노출해 원격 진단에 쓴다
/// (어떤 config가 실제 엔진에 적용됐는지 로그 없이 확인 가능).
static KEYMAP_SUMMARY: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn keymap_layer_summaries(kb: &KeybindConfig) -> Vec<String> {
    kb.layers
        .iter()
        .map(|l| {
            format!(
                "{}: {:?} 홀드 · unmapped={:?} · 매핑 {}개 · 더블탭 {}개",
                l.name,
                l.trigger,
                l.unmapped,
                l.mappings.len(),
                l.double_tap_mappings.len()
            )
        })
        .collect()
}

fn resolve_keybind_preset(config: &Config) -> KeybindConfig {
    // 프리셋 기본값·사용자 병합·한/영 폴백은 전부 kmd-core의
    // effective_keymap(단일 소스)에서 수행된다 — 치트시트와 항상 동일 결과.
    let effective = kmd_core::keymap::effective_keymap(&config.launcher.keymap);
    let mut kb = KeybindConfig::from_config(&effective).unwrap_or_else(KeybindConfig::empty);

    // global_hotkey → kmd-desktop 실행 콤보 (프로필과 무관하게 항상 등록)
    let hotkey = &config.keybindings.global_hotkey;
    if !hotkey.is_empty() {
        if let Some(trigger) = keybind::parse_combo_trigger(hotkey) {
            kb.combos
                .push((trigger, keybind::BindAction::Launch("kmd-desktop".into())));
            tracing::info!("글로벌 핫키 등록: {hotkey} → kmd-desktop 실행");
        }
    }

    let toggle_keymap = config.keybindings.toggle_keymap.trim();
    if !toggle_keymap.is_empty() {
        if let Some(trigger) = keybind::parse_combo_trigger(toggle_keymap) {
            kb.toggle_keymap = Some(trigger);
            tracing::info!("keymap toggle hotkey registered: {toggle_keymap}");
        } else {
            tracing::warn!("keymap toggle hotkey parse failed: {toggle_keymap}");
        }
    }

    let summaries = keymap_layer_summaries(&kb);
    for s in &summaries {
        tracing::info!("레이어 {s}");
    }
    if let Ok(mut g) = KEYMAP_SUMMARY.lock() {
        *g = summaries;
    }

    kb
}

fn write_runtime_files(port: u16, token: &str) -> color_eyre::Result<()> {
    // 런타임 파일은 포터블 모드와 무관하게 OS 표준 사용자 디렉터리에 둔다
    // (토큰 노출 방지 — ipc::runtime_dir 문서 참조)
    std::fs::create_dir_all(ipc::runtime_dir())?;

    // daemon.port: line1=port, line2=token (동일 사용자만 읽도록 Unix 0600).
    // 쓰기 후 chmod 하면 그 사이에 다른 사용자가 읽을 수 있으므로
    // 처음부터 0600으로 생성한다. 기존 파일은 권한이 남을 수 있어 먼저 제거.
    let port_path = ipc::port_file_path();
    let _ = std::fs::remove_file(&port_path);
    {
        use std::io::Write as _;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&port_path)?;
        write!(f, "{port}\n{token}\n")?;
    }

    std::fs::write(ipc::pid_file_path(), std::process::id().to_string())?;
    Ok(())
}

fn cleanup_runtime_files() {
    ipc::cleanup_stale_files();
}

#[cfg(test)]
mod tests {
    use super::*;

    // 프리셋·병합 시맨틱 테스트는 단일 소스인 kmd-core::keymap
    // (effective_keymap)의 테스트로 이동했다. 여기서는 daemon 고유 경로인
    // "effective → KeybindConfig 변환 + 프로필 비활성"만 확인한다.

    fn resolve_with_profile(profile: &str) -> KeybindConfig {
        let mut config = Config::default();
        config.launcher.keymap.active_profile = profile.to_string();
        config.keybindings.global_hotkey = String::new();
        config.keybindings.toggle_keymap = String::new();
        resolve_keybind_preset(&config)
    }

    #[test]
    fn resolve_vim_nav_기본_레이어() {
        let kb = resolve_with_profile("vim-nav");
        assert_eq!(kb.layers.len(), 1);
        assert_eq!(kb.layers[0].trigger, keybind::VKey::LAlt);
        assert!(!kb.combos.is_empty(), "한/영 Shift+Space 콤보 포함");
    }

    #[test]
    fn resolve_확장자_붙은_프로필도_vim_nav() {
        // 과거 치트시트는 완전 일치("vim-nav")만 프리셋으로 인식해
        // "vim-nav.kbd"에서 daemon과 표시가 어긋났다 — 회귀 방지
        let kb = resolve_with_profile("vim-nav.kbd");
        assert_eq!(kb.layers.len(), 1);
    }

    #[test]
    fn resolve_none_프로필은_키맵_비활성() {
        let kb = resolve_with_profile("none");
        assert!(kb.layers.is_empty());
        assert!(kb.remaps.is_empty());
        assert!(kb.combos.is_empty(), "비활성 시 한/영 콤보도 없음");
    }

    #[test]
    fn resolve_global_hotkey는_프로필과_무관() {
        let mut config = Config::default();
        config.launcher.keymap.active_profile = "none".to_string();
        config.keybindings.global_hotkey = "alt+space".to_string();
        config.keybindings.toggle_keymap = String::new();
        let kb = resolve_keybind_preset(&config);
        assert_eq!(kb.combos.len(), 1, "none 프로필에서도 글로벌 핫키는 등록");
    }

    #[test]
    fn resolve_launch_cmd_경로순회_방지() {
        let resolved = keybind::resolve_launch_cmd("../../../etc/passwd");
        assert!(
            !resolved.contains(".."),
            "경로 순회 문자가 제거되어야 함: {resolved}"
        );
    }
}
