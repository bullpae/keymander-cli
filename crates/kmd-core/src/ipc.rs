//! IPC 프로토콜 타입 및 소켓 경로 헬퍼
//!
//! 데몬 ↔ 클라이언트(kmd, kmd-desktop) 간 통신에 사용하는 메시지 정의.
//!
//! 와이어 프로토콜 (요청):
//!   line 1: <token-hex>
//!   line 2: <json-request>
//!
//! 토큰은 데몬 시작 시 32바이트 OS RNG로 생성되어 `daemon.port` 파일
//! 두 번째 줄에 저장된다. 동일 사용자만 읽을 수 있도록 Unix에서는
//! 0600 권한으로 기록한다. 인증 실패 시 서버는 응답 없이 연결을 종료한다.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── 요청 ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// 검색 쿼리 실행
    Search {
        query: String,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    /// 인덱스 리빌드
    RebuildIndex,
    /// 데몬 상태 조회
    Status,
    /// 자동 시작 등록 상태 조회
    AutostartStatus,
    /// 자동 시작 등록
    AutostartEnable,
    /// 자동 시작 해제
    AutostartDisable,
    /// 데몬 종료
    Shutdown,
    /// 연결 확인
    Ping,
}

fn default_limit() -> usize {
    20
}

// ── 응답 ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// 검색 결과
    SearchResults { items: Vec<SearchHit> },
    /// 데몬 상태
    Status {
        uptime_secs: u64,
        index_items: usize,
        pid: u32,
    },
    /// 자동 시작 등록 상태
    AutostartStatus { installed: bool },
    /// 단순 성공
    Ok { message: String },
    /// 에러
    Error { message: String },
    /// Ping 응답
    Pong,
}

/// IPC 전송용 검색 결과 아이템
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub icon: String,
    pub score: u32,
}

// ── 경로 헬퍼 ────────────────────────────────────────────────────────────────

/// 데몬 런타임 파일 경로. 형식:
/// ```text
/// <port>
/// <token-hex>
/// ```
pub fn port_file_path() -> PathBuf {
    crate::Config::default_data_dir().join("daemon.port")
}

/// 데몬 PID 파일 경로
pub fn pid_file_path() -> PathBuf {
    crate::Config::default_data_dir().join("daemon.pid")
}

// ── 인증 토큰 ────────────────────────────────────────────────────────────────

/// 32바이트 OS RNG에서 hex 64자 토큰 생성
pub fn generate_token() -> String {
    use std::fmt::Write;
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("OS RNG unavailable");
    let mut s = String::with_capacity(64);
    for b in buf {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// 상수 시간 바이트 비교 — 타이밍 공격 방지
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 데몬 런타임 파일을 (port, token)으로 파싱
pub fn read_runtime() -> Result<(u16, String), IpcError> {
    let content = std::fs::read_to_string(port_file_path()).map_err(|_| IpcError::NoDaemon)?;
    let mut lines = content.lines();
    let port: u16 = lines
        .next()
        .ok_or(IpcError::NoDaemon)?
        .trim()
        .parse()
        .map_err(|_| IpcError::NoDaemon)?;
    let token = lines.next().ok_or(IpcError::NoDaemon)?.trim().to_string();
    if token.is_empty() {
        return Err(IpcError::NoDaemon);
    }
    Ok((port, token))
}

// ── 직렬화 헬퍼 ──────────────────────────────────────────────────────────────

/// 요청을 JSON Line 형식으로 직렬화
pub fn encode_request(req: &Request) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(req)?;
    s.push('\n');
    Ok(s)
}

/// 응답을 JSON Line 형식으로 직렬화
pub fn encode_response(res: &Response) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(res)?;
    s.push('\n');
    Ok(s)
}

/// JSON Line에서 요청 역직렬화
pub fn decode_request(line: &str) -> Result<Request, serde_json::Error> {
    serde_json::from_str(line.trim())
}

/// JSON Line에서 응답 역직렬화
pub fn decode_response(line: &str) -> Result<Response, serde_json::Error> {
    serde_json::from_str(line.trim())
}

// ── 에러 타입 ────────────────────────────────────────────────────────────────

/// IPC 통신 오류
#[derive(Debug)]
pub enum IpcError {
    /// 데몬이 실행 중이지 않음 (포트 파일 없음 / 파싱 실패)
    NoDaemon,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDaemon => write!(f, "데몬이 실행 중이지 않습니다"),
            Self::Io(e) => write!(f, "IO 오류: {e}"),
            Self::Json(e) => write!(f, "JSON 오류: {e}"),
        }
    }
}

impl std::error::Error for IpcError {}

// ── 클라이언트 헬퍼 ─────────────────────────────────────────────────────────

/// 포트 파일에서 데몬 포트 읽기 (실패 시 에러 반환)
pub fn read_port() -> Result<u16, IpcError> {
    read_runtime().map(|(port, _)| port)
}

/// 데몬이 실행 중이면 포트를 반환
pub fn daemon_port() -> Option<u16> {
    read_port().ok()
}

/// 오래된 런타임 파일(port/pid) 정리
pub fn cleanup_stale_files() {
    let _ = std::fs::remove_file(port_file_path());
    let _ = std::fs::remove_file(pid_file_path());
}

/// 데몬에 요청 전송 후 응답 반환 (실패 시 에러 반환)
pub fn send_request_result(request: &Request) -> Result<Response, IpcError> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let (port, token) = read_runtime()?;
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).map_err(IpcError::Io)?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(IpcError::Io)?;

    // line 1: token, line 2: JSON request
    let mut payload = String::with_capacity(token.len() + 64);
    payload.push_str(&token);
    payload.push('\n');
    let encoded = encode_request(request).map_err(IpcError::Json)?;
    payload.push_str(&encoded);

    stream.write_all(payload.as_bytes()).map_err(IpcError::Io)?;
    stream.flush().map_err(IpcError::Io)?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(IpcError::Io)?;

    decode_response(&line).map_err(IpcError::Json)
}

/// 데몬에 요청 전송 후 응답 반환. 연결 실패 시 None.
pub fn send_request(request: &Request) -> Option<Response> {
    send_request_result(request).ok()
}

/// 데몬이 실행 중인지 확인 (Ping 시도)
pub fn is_daemon_running() -> bool {
    matches!(send_request(&Request::Ping), Some(Response::Pong))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_roundtrip() {
        let req = Request::Search {
            query: "firefox".into(),
            limit: 10,
        };
        let encoded = encode_request(&req).unwrap();
        let decoded = decode_request(&encoded).unwrap();
        match decoded {
            Request::Search { query, limit } => {
                assert_eq!(query, "firefox");
                assert_eq!(limit, 10);
            }
            _ => panic!("잘못된 타입"),
        }
    }

    #[test]
    fn test_response_roundtrip() {
        let res = Response::Status {
            uptime_secs: 42,
            index_items: 1000,
            pid: 12345,
        };
        let encoded = encode_response(&res).unwrap();
        let decoded = decode_response(&encoded).unwrap();
        match decoded {
            Response::Status {
                uptime_secs,
                index_items,
                pid,
            } => {
                assert_eq!(uptime_secs, 42);
                assert_eq!(index_items, 1000);
                assert_eq!(pid, 12345);
            }
            _ => panic!("잘못된 타입"),
        }
    }

    #[test]
    fn test_ping_pong() {
        let req = Request::Ping;
        let encoded = encode_request(&req).unwrap();
        assert!(encoded.contains("Ping"));

        let res = Response::Pong;
        let encoded = encode_response(&res).unwrap();
        assert!(encoded.contains("Pong"));
    }
}
