use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct AgqConfig {
    pub addr: String,
    pub session_key: Option<String>,
    pub timeout: Duration,
}

impl AgqConfig {
    pub fn from_env() -> Self {
        let addr = std::env::var("AGQ_ADDR").unwrap_or_else(|_| "127.0.0.1:6380".to_string());
        let session_key = std::env::var("AGQ_SESSION_KEY").ok();
        let timeout_secs = std::env::var("AGQ_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);

        Self {
            addr,
            session_key,
            timeout: Duration::from_secs(timeout_secs),
        }
    }
}

pub struct AgqClient {
    config: AgqConfig,
}

#[derive(Debug, Clone)]
pub struct SubmissionResult {
    pub job_id: String,
    pub submitted_at: SystemTime,
}

impl AgqClient {
    pub fn new(config: AgqConfig) -> Self {
        Self { config }
    }

    pub fn submit_plan(&self, plan_json: &str) -> Result<SubmissionResult, String> {
        let stream =
            TcpStream::connect(&self.config.addr).map_err(|e| format!("connect error: {e}"))?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(|e| format!("failed to set read timeout: {e}"))?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(|e| format!("failed to set write timeout: {e}"))?;

        let mut reader = BufReader::new(stream);
        if let Some(ref key) = self.config.session_key {
            let auth = resp_array(&["AUTH", key]);
            {
                let stream = reader.get_mut();
                stream
                    .write_all(&auth)
                    .map_err(|e| format!("failed to send AUTH: {e}"))?;
            }

            let auth_response = read_resp_value(&mut reader)?;
            match auth_response {
                RespValue::SimpleString(_) | RespValue::BulkString(_) => {}
                RespValue::Error(msg) => return Err(format!("AUTH failed: {msg}")),
                other => return Err(format!("unexpected AUTH response: {:?}", other)),
            }
        }

        let submit = resp_array(&["PLAN.SUBMIT", plan_json]);
        {
            let stream = reader.get_mut();
            stream
                .write_all(&submit)
                .map_err(|e| format!("failed to send PLAN.SUBMIT: {e}"))?;
        }

        let response = read_resp_value(&mut reader)?;

        match response {
            RespValue::SimpleString(s) | RespValue::BulkString(s) => Ok(SubmissionResult {
                job_id: s,
                submitted_at: SystemTime::now(),
            }),
            RespValue::Error(msg) => Err(format!("AGQ error: {msg}")),
            other => Err(format!("unexpected AGQ response: {:?}", other)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum RespValue {
    SimpleString(String),
    BulkString(String),
    Error(String),
    Integer(i64),
    Array(Vec<RespValue>),
    Null,
}

fn read_resp_value<R: BufRead + Read>(reader: &mut R) -> Result<RespValue, String> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("failed to read RESP: {e}"))?;

    if line.is_empty() {
        return Err("empty response from AGQ".to_string());
    }

    let prefix = line
        .chars()
        .next()
        .ok_or_else(|| "malformed RESP response".to_string())?;

    let rest = line[1..].trim_end_matches("\r\n");

    match prefix {
        '+' => Ok(RespValue::SimpleString(rest.to_string())),
        '-' => Ok(RespValue::Error(rest.to_string())),
        ':' => rest
            .parse::<i64>()
            .map(RespValue::Integer)
            .map_err(|e| format!("invalid RESP integer: {e}")),
        '$' => {
            let len: i64 = rest
                .parse()
                .map_err(|e| format!("invalid RESP bulk length: {e}"))?;
            if len < 0 {
                return Ok(RespValue::Null);
            }
            let mut buf = vec![0u8; len as usize + 2]; // include CRLF
            reader
                .read_exact(&mut buf)
                .map_err(|e| format!("failed to read bulk body: {e}"))?;
            let body = String::from_utf8_lossy(&buf[..len as usize]).to_string();
            Ok(RespValue::BulkString(body))
        }
        '*' => {
            let count: i64 = rest
                .parse()
                .map_err(|e| format!("invalid RESP array length: {e}"))?;
            if count < 0 {
                return Ok(RespValue::Null);
            }
            let mut items = Vec::new();
            for _ in 0..count {
                items.push(read_resp_value(reader)?);
            }
            Ok(RespValue::Array(items))
        }
        other => Err(format!("unsupported RESP prefix: {other}")),
    }
}

fn resp_array(items: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(format!("*{}\r\n", items.len()).as_bytes());
    for item in items {
        out.extend_from_slice(format!("${}\r\n", item.len()).as_bytes());
        out.extend_from_slice(item.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn submits_plan_and_parses_job_id() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);

            // Expect AUTH
            let auth_req = read_resp_value(&mut reader).expect("read auth request");
            match auth_req {
                RespValue::Array(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(items[0], RespValue::BulkString("AUTH".to_string()));
                    assert_eq!(items[1], RespValue::BulkString("secret".to_string()));
                }
                other => panic!("unexpected auth request: {:?}", other),
            }

            // Send OK for AUTH
            reader
                .get_mut()
                .write_all(b"+OK\r\n")
                .expect("write auth ok");

            // Expect PLAN.SUBMIT
            let submit_req = read_resp_value(&mut reader).expect("read submit");
            match submit_req {
                RespValue::Array(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(items[0], RespValue::BulkString("PLAN.SUBMIT".to_string()));
                    assert_eq!(items[1], RespValue::BulkString("{\"plan\": []}".to_string()));
                }
                other => panic!("unexpected submit request: {:?}", other),
            }

            // Respond with bulk string job id
            reader
                .get_mut()
                .write_all(b"$6\r\njob-42\r\n")
                .expect("failed to write response");
        });

        let client = AgqClient::new(AgqConfig {
            addr: addr.to_string(),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(2),
        });

        let result = client
            .submit_plan("{\"plan\": []}")
            .expect("submit should succeed");
        assert_eq!(result.job_id, "job-42");

        server.join().unwrap();
    }

    #[test]
    fn fails_when_server_unreachable() {
        let client = AgqClient::new(AgqConfig {
            addr: "127.0.0.1:61234".to_string(),
            session_key: None,
            timeout: Duration::from_secs(1),
        });

        let result = client.submit_plan("{\"plan\": []}");
        assert!(result.is_err());
    }

    #[test]
    fn auth_error_propagates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);

            let _auth_req = read_resp_value(&mut reader).expect("read auth request");
            reader
                .get_mut()
                .write_all(b"-ERR invalid session\r\n")
                .expect("failed to write error");
        });

        let client = AgqClient::new(AgqConfig {
            addr: addr.to_string(),
            session_key: Some("bad".to_string()),
            timeout: Duration::from_secs(2),
        });

        let result = client.submit_plan("{\"plan\": []}");
        assert!(matches!(result, Err(e) if e.contains("AUTH failed")));

        server.join().unwrap();
    }
}
