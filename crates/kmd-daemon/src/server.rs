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

fn load_config() -> Config {
    let config_dir = Config::default_config_dir();
    Config::load(&config_dir).unwrap_or_default()
}

/// config의 keymap 설정에 따라 KeybindConfig 결정.
///
/// - "vim-nav": config 파일에 커스텀 설정이 있으면 그 값을 사용, 없으면 하드코딩 프리셋
/// - "minimal": config 파일 → 없으면 minimal 프리셋
/// - "none": 키 바인딩 비활성화 (global_hotkey만 동작)
/// - 기타: TOML 커스텀 설정 시도 → 없으면 vim-nav 프리셋 폴백
///
/// `keybindings.global_hotkey`는 프로필과 무관하게 항상 적용됨.
fn resolve_keybind_preset(config: &Config) -> KeybindConfig {
    let profile = &config.launcher.keymap.active_profile;
    let keymap_disabled = profile.contains("none");

    let base = if keymap_disabled {
        KeybindConfig::empty()
    } else if profile.contains("minimal") {
        KeybindConfig::minimal_preset()
    } else {
        KeybindConfig::vim_nav_preset()
    };

    let mut kb = if let Some(custom) = KeybindConfig::from_config(&config.launcher.keymap) {
        merge_keybind_config(base, custom)
    } else {
        base
    };

    if !keymap_disabled {
        ensure_hangul_fallback(&mut kb);
    }

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

    kb
}

/// 프리셋(base) + 사용자 TOML(custom) 병합. KeybindConfig::merge 위임.
fn merge_keybind_config(base: KeybindConfig, custom: KeybindConfig) -> KeybindConfig {
    base.merge(custom)
}

/// 한/영 전환 기본 제스처를 보장한다.
///
/// - Shift+Space → 한/영: 전 플랫폼 공통. macOS는 execute_action에서 TIS API로
///   입력 소스를 전환하므로, 물리적 한/영 키가 없는 60% 키보드에서도 Windows와
///   동일한 제스처를 제공한다.
/// - RShift 더블탭 → 한/영: Windows 전용 보조 제스처.
fn ensure_hangul_fallback(kb: &mut KeybindConfig) {
    let has_shift_space = kb.combos.iter().any(|(trigger, _)| {
        trigger.key == keybind::VKey::Space
            && trigger.modifiers.len() == 1
            && trigger.modifiers[0] == keybind::Modifier::Shift
    });

    if !has_shift_space {
        kb.combos.push((
            keybind::ComboTrigger {
                modifiers: vec![keybind::Modifier::Shift],
                key: keybind::VKey::Space,
            },
            keybind::BindAction::SendKey(keybind::VKey::Hangul),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let has_rshift_dt = kb
            .double_taps
            .iter()
            .any(|dt| dt.key == keybind::VKey::RShift);
        if !has_rshift_dt {
            kb.double_taps.push(keybind::DoubleTapBinding {
                key: keybind::VKey::RShift,
                action: keybind::BindAction::SendKey(keybind::VKey::Hangul),
                timeout_ms: 300,
            });
        }
    }
}

fn write_runtime_files(port: u16, token: &str) -> color_eyre::Result<()> {
    let data_dir = Config::default_data_dir();
    std::fs::create_dir_all(&data_dir)?;

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

    #[cfg(target_os = "windows")]
    #[test]
    fn merge_keybind_keeps_base_combos_when_custom_has_none() {
        let base = KeybindConfig::minimal_preset();
        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![],
            combos: vec![],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        assert!(
            !merged.combos.is_empty(),
            "Windows: custom 설정이 비어있어도 base 콤보(Hangul)는 유지되어야 함"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn merge_keybind_keeps_base_combos_on_non_windows() {
        let base = KeybindConfig::minimal_preset();
        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![],
            combos: vec![],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        assert!(
            !merged.combos.is_empty(),
            "macOS/Linux: base의 Shift+Space 한/영 콤보는 유지되어야 함 (크로스플랫폼 통일)"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn merge_keybind_overrides_same_combo_trigger() {
        let base = KeybindConfig::minimal_preset();
        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![],
            combos: vec![(
                keybind::ComboTrigger {
                    modifiers: vec![keybind::Modifier::Shift],
                    key: keybind::VKey::Space,
                },
                keybind::BindAction::SendKey(keybind::VKey::Hanja),
            )],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        let mut matched = false;
        for (trigger, action) in merged.combos {
            if trigger.key == keybind::VKey::Space
                && trigger.modifiers.len() == 1
                && trigger.modifiers[0] == keybind::Modifier::Shift
            {
                matched = matches!(action, keybind::BindAction::SendKey(keybind::VKey::Hanja));
            }
        }
        assert!(matched, "동일 트리거 콤보는 custom 액션으로 덮어써야 함");
    }

    #[test]
    fn merge_layer_deep_보존_base_매핑() {
        let base = KeybindConfig::vim_nav_preset();
        let base_h = base.layers[0].mappings.contains_key(&keybind::VKey::H);
        let base_period = base.layers[0].mappings.contains_key(&keybind::VKey::Period);
        let base_dt_i = base.layers[0]
            .double_tap_mappings
            .contains_key(&keybind::VKey::I);
        assert!(base_h && base_period && base_dt_i, "프리셋 확인");

        // custom은 H만 재정의 → Period, double-tap I 등 base 매핑은 유지
        let mut custom_mappings = std::collections::HashMap::new();
        custom_mappings.insert(
            keybind::VKey::H,
            keybind::BindAction::SendKey(keybind::VKey::Right),
        );
        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![keybind::Layer {
                name: "nav".into(),
                trigger: keybind::VKey::LAlt,
                tap_action: Some(keybind::VKey::Escape),
                tap_hold_ms: 200,
                mappings: custom_mappings,
                double_tap_mappings: std::collections::HashMap::new(),
            }],
            combos: vec![],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        assert_eq!(merged.layers.len(), 1);
        let layer = &merged.layers[0];

        // H는 custom으로 덮어씀
        assert!(
            matches!(
                layer.mappings.get(&keybind::VKey::H),
                Some(keybind::BindAction::SendKey(keybind::VKey::Right))
            ),
            "H는 custom 값(Right)이어야 함"
        );

        // Period는 base에서 보존
        assert!(
            layer.mappings.contains_key(&keybind::VKey::Period),
            "Period는 base에서 보존되어야 함 (deep merge)"
        );

        // double-tap I는 base에서 보존
        assert!(
            layer.double_tap_mappings.contains_key(&keybind::VKey::I),
            "double-tap I는 base에서 보존되어야 함"
        );

        // Space(launch:kmd-desktop)도 base에서 보존
        assert!(
            layer.mappings.contains_key(&keybind::VKey::Space),
            "Space는 base에서 보존되어야 함"
        );
    }

    #[test]
    fn merge_layer_deep_custom_double_tap_덮어쓰기() {
        let base = KeybindConfig::vim_nav_preset();
        let custom_dt = keybind::LayerDoubleTap {
            single_action: keybind::BindAction::SendKey(keybind::VKey::Home),
            double_action: keybind::BindAction::SendKey(keybind::VKey::End),
            timeout_ms: 500,
        };
        let mut dt_map = std::collections::HashMap::new();
        dt_map.insert(keybind::VKey::I, custom_dt);

        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![keybind::Layer {
                name: "nav".into(),
                trigger: keybind::VKey::LAlt,
                tap_action: None,
                tap_hold_ms: 200,
                mappings: std::collections::HashMap::new(),
                double_tap_mappings: dt_map,
            }],
            combos: vec![],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        let layer = &merged.layers[0];

        // custom의 double-tap I가 base를 덮어씀
        let dt = layer.double_tap_mappings.get(&keybind::VKey::I).unwrap();
        assert_eq!(dt.timeout_ms, 500, "custom timeout_ms 적용");

        // tap_action은 None이면 base 값 유지 (Escape)
        assert_eq!(
            layer.tap_action,
            Some(keybind::VKey::Escape),
            "custom tap_action이 None이면 base 유지"
        );
    }

    #[test]
    fn merge_새_레이어_추가() {
        let base = KeybindConfig::vim_nav_preset();
        let mut mappings = std::collections::HashMap::new();
        mappings.insert(
            keybind::VKey::A,
            keybind::BindAction::SendKey(keybind::VKey::B),
        );
        let custom = KeybindConfig {
            remaps: std::collections::HashMap::new(),
            layers: vec![keybind::Layer {
                name: "custom-layer".into(),
                trigger: keybind::VKey::RAlt,
                tap_action: None,
                tap_hold_ms: 150,
                mappings,
                double_tap_mappings: std::collections::HashMap::new(),
            }],
            combos: vec![],
            double_taps: vec![],
            toggle_keymap: None,
        };

        let merged = merge_keybind_config(base, custom);
        assert_eq!(merged.layers.len(), 2, "새 레이어가 추가되어야 함");
        assert_eq!(merged.layers[1].name, "custom-layer");
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
