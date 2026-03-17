//! IPC 프로토콜 타입 및 소켓 경로 헬퍼
//!
//! 데몬 ↔ 클라이언트(kmd, kmd-desktop) 간 통신에 사용하는 메시지 정의.
//! 전송 형식: 줄바꿈 구분 JSON (JSON Lines).

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

/// 데몬 포트 파일 경로 (TCP localhost 포트 번호 저장)
pub fn port_file_path() -> PathBuf {
    crate::Config::default_data_dir().join("daemon.port")
}

/// 데몬 PID 파일 경로
pub fn pid_file_path() -> PathBuf {
    crate::Config::default_data_dir().join("daemon.pid")
}

/// 데몬 기본 포트 (port 파일이 없을 때)
pub const DEFAULT_PORT: u16 = 0; // OS가 빈 포트를 자동 할당

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

// ── 클라이언트 헬퍼 ─────────────────────────────────────────────────────────

/// 데몬이 실행 중이면 포트를 반환
pub fn daemon_port() -> Option<u16> {
    let content = std::fs::read_to_string(port_file_path()).ok()?;
    content.trim().parse().ok()
}

/// 데몬에 요청 전송 후 응답 반환. 연결 실패 시 None.
pub fn send_request(request: &Request) -> Option<Response> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let port = daemon_port()?;
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).ok()?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok()?;

    let encoded = encode_request(request).ok()?;
    stream.write_all(encoded.as_bytes()).ok()?;
    stream.flush().ok()?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;

    decode_response(&line).ok()
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
