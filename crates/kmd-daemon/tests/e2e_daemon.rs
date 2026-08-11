//! 데몬 프로세스/IPC E2E (docs/14 Tier 1)
//!
//! 빌드된 실제 kmd-daemon 바이너리를 spawn해 기동→인증→상태→중복기동 거부→
//! 종료까지 프로세스 경계를 통째로 검증한다. 단위 테스트가 못 잡는
//! "데몬이 실제로 뜨고 IPC가 사는가"를 CI에서 보장하는 것이 목적.
//!
//! - `KMD_E2E=1`일 때만 실행 (미설정 시 조용히 skip — CI test 잡이 설정한다)
//! - unix 전용: 자식 프로세스의 HOME/XDG_*를 tempdir로 돌려 config·data·
//!   런타임 파일을 실사용 환경과 격리한다. Windows의 dirs는 KnownFolder API를
//!   쓰므로 env 격리가 안 된다 → Tier 2에서 KMD_DATA_DIR 오버라이드로 편입.
//! - 테스트 config는 트리거를 LAlt로 명시한다 — 기본값(CapsLock)이면 macOS
//!   데몬이 hidutil 재맵(시스템 전역 상태)을 적용한다. config_error 어서션이
//!   이 안전장치의 무효화(파싱 실패 → 기본 config 폴백)를 잡는다.
#![cfg(unix)]

use kmd_core::ipc::{self, Request, Response};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// 테스트 격리 홈 디렉터리 — Drop 시 삭제
struct TempHome(PathBuf);

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// spawn한 데몬 — 테스트 실패로 조기 반환해도 프로세스가 남지 않게 하는 안전망
struct DaemonGuard(Child);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 자식 데몬 기준의 데이터 디렉터리 (포트/락 파일 위치)
fn child_data_dir(home: &TempHome) -> PathBuf {
    #[cfg(target_os = "macos")]
    return home.0.join("Library/Application Support/kmd");
    #[cfg(not(target_os = "macos"))]
    return home.0.join("xdg-data/kmd");
}

/// 자식 데몬 기준의 config 디렉터리
fn child_config_dir(home: &TempHome) -> PathBuf {
    #[cfg(target_os = "macos")]
    return home.0.join("Library/Application Support/kmd");
    #[cfg(not(target_os = "macos"))]
    return home.0.join("xdg-config/kmd");
}

fn spawn_daemon(home: &TempHome) -> DaemonGuard {
    let log = std::fs::File::create(home.0.join("daemon-stdio.log")).expect("로그 파일 생성");
    let child = Command::new(env!("CARGO_BIN_EXE_kmd-daemon"))
        .arg("start")
        .env("HOME", &home.0)
        .env("XDG_CONFIG_HOME", home.0.join("xdg-config"))
        .env("XDG_DATA_HOME", home.0.join("xdg-data"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().expect("로그 핸들 복제"))
        .stderr(log)
        .spawn()
        .expect("kmd-daemon spawn");
    DaemonGuard(child)
}

/// 포트 파일이 생길 때까지 폴링 후 (port, token) 파싱
fn wait_runtime(home: &TempHome, deadline: Duration) -> (u16, String) {
    let port_file = child_data_dir(home).join("daemon.port");
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Ok(content) = std::fs::read_to_string(&port_file) {
            let mut lines = content.lines();
            if let (Some(port), Some(token)) = (lines.next(), lines.next()) {
                if let Ok(port) = port.trim().parse::<u16>() {
                    if !token.trim().is_empty() {
                        return (port, token.trim().to_string());
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "포트 파일이 {}초 안에 생성되지 않음: {}\n데몬 로그:\n{}",
        deadline.as_secs(),
        port_file.display(),
        std::fs::read_to_string(home.0.join("daemon-stdio.log")).unwrap_or_default()
    );
}

/// 요청 1회 전송. 응답이 없으면(인증 거부 = 무응답 연결 종료) None.
fn send(port: u16, token: &str, req: &Request) -> Option<Response> {
    let stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut writer = &stream;
    let payload = format!("{token}\n{}", ipc::encode_request(req).ok()?);
    writer.write_all(payload.as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    if line.trim().is_empty() {
        return None;
    }
    ipc::decode_response(&line).ok()
}

/// 자식 프로세스 종료를 폴링 대기. 종료하면 true.
fn wait_exit(child: &mut Child, deadline: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn e2e_daemon_lifecycle() {
    if std::env::var("KMD_E2E").is_err() {
        eprintln!("KMD_E2E 미설정 — E2E skip (CI가 설정, 로컬: KMD_E2E=1 cargo test ...)");
        return;
    }

    let home = TempHome(std::env::temp_dir().join(format!("kmd-e2e-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&home.0);

    // 격리 config:
    // - 트리거 LAlt 명시 (파일 상단 주석의 hidutil 안전장치)
    // - 파일 인덱싱 차단 (builtin 프로바이더 + 빈 search_paths) — CI macOS
    //   러너는 Spotlight가 꺼져 있어 mdfind가 무기한 멈추고, 인덱스 빌드가
    //   IPC 서버 기동보다 앞서므로 포트 파일이 영영 안 생긴다 (실측).
    //   라이프사이클 테스트에 파일 인덱싱은 관심사가 아니다.
    let config_dir = child_config_dir(&home);
    std::fs::create_dir_all(&config_dir).expect("config 디렉터리 생성");
    std::fs::write(
        config_dir.join("config.toml"),
        r#"
[launcher]
file_search_provider = "builtin"
search_paths = []
index_directories = false
scan_drives = false

[launcher.keymap.layers.nav]
trigger = "LAlt"
tap_action = "Escape"
tap_hold_ms = 200

[launcher.keymap.layers.nav.mappings]
H = "Left"
"#,
    )
    .expect("config.toml 작성");

    // ── 기동 → 포트 파일 → 인증 → 상태 ──────────────────────────────────
    let mut daemon = spawn_daemon(&home);
    let (port, token) = wait_runtime(&home, Duration::from_secs(60));

    assert!(
        matches!(send(port, &token, &Request::Ping), Some(Response::Pong)),
        "Ping에 Pong이 와야 한다"
    );

    match send(port, &token, &Request::Status) {
        Some(Response::Status {
            pid,
            keymap_layers,
            config_error,
            ..
        }) => {
            assert_eq!(
                pid,
                daemon.0.id(),
                "Status의 pid는 spawn한 프로세스여야 한다"
            );
            assert!(
                config_error.is_none(),
                "격리 config가 파싱돼야 한다 (실패 시 기본 config 폴백 = hidutil 위험): {config_error:?}"
            );
            assert!(
                keymap_layers.iter().any(|l| l.contains("nav")),
                "nav 레이어가 로드돼야 한다: {keymap_layers:?}"
            );
        }
        other => panic!("Status 응답이 아님: {other:?}"),
    }

    // ── 인증: 잘못된 토큰은 무응답으로 연결 종료 ─────────────────────────
    assert!(
        send(port, "deadbeef-wrong-token", &Request::Ping).is_none(),
        "잘못된 토큰은 응답 없이 거부돼야 한다"
    );

    // ── 단일 인스턴스: 같은 환경에서 중복 기동은 곧 종료 ─────────────────
    let mut dup = spawn_daemon(&home);
    assert!(
        wait_exit(&mut dup.0, Duration::from_secs(10)),
        "중복 데몬은 단일 인스턴스 가드로 종료돼야 한다"
    );
    assert!(
        matches!(send(port, &token, &Request::Ping), Some(Response::Pong)),
        "중복 기동 시도 후에도 원래 데몬은 살아 있어야 한다"
    );

    // ── 정상 종료 ────────────────────────────────────────────────────────
    match send(port, &token, &Request::Shutdown) {
        Some(Response::Ok { .. }) => {}
        other => panic!("Shutdown에 Ok가 와야 한다: {other:?}"),
    }
    assert!(
        wait_exit(&mut daemon.0, Duration::from_secs(10)),
        "Shutdown 후 프로세스가 종료돼야 한다"
    );
}
