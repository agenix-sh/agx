use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq)]
pub enum OpsResponse {
    Jobs(Vec<String>),
    Workers(Vec<String>),
    QueueStats(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub plan_id: String,
    pub description: Option<String>,
    pub task_count: usize,
    pub created_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub action_id: String,
    pub plan_id: String,
    pub plan_description: Option<String>,
    pub jobs_created: usize,
    pub job_ids: Vec<String>,
}

impl ActionEnvelope {
    /// Validate that jobs_created matches job_ids length
    /// Prevents silent failures from AGQ data inconsistencies
    pub fn validate(&self) -> Result<(), String> {
        if self.jobs_created != self.job_ids.len() {
            return Err(format!(
                "ActionEnvelope validation failed: jobs_created ({}) != job_ids.len() ({})",
                self.jobs_created,
                self.job_ids.len()
            ));
        }
        Ok(())
    }
}

impl AgqClient {
    pub fn new(config: AgqConfig) -> Self {
        Self { config }
    }

    pub fn submit_plan(&self, plan_json: &str) -> Result<SubmissionResult, String> {
        let mut reader = self.connect_and_auth()?;

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

    pub fn submit_action(&self, action_json: &str) -> Result<ActionEnvelope, String> {
        let mut reader = self.connect_and_auth()?;

        let submit = resp_array(&["ACTION.SUBMIT", action_json]);
        {
            let stream = reader.get_mut();
            stream
                .write_all(&submit)
                .map_err(|e| format!("failed to send ACTION.SUBMIT: {e}"))?;
        }

        let response = read_resp_value(&mut reader)?;

        match response {
            RespValue::BulkString(s) => {
                let envelope: ActionEnvelope = serde_json::from_str(&s)
                    .map_err(|e| format!("failed to parse ACTION.SUBMIT response: {e}"))?;
                envelope.validate()?;
                Ok(envelope)
            }
            RespValue::Error(msg) => Err(format!("AGQ error: {msg}")),
            other => Err(format!("unexpected AGQ response: {:?}", other)),
        }
    }

    pub fn list_jobs(&self) -> Result<OpsResponse, String> {
        self.simple_query("JOBS.LIST", OpsResponse::Jobs)
    }

    pub fn list_workers(&self) -> Result<OpsResponse, String> {
        self.simple_query("WORKERS.LIST", OpsResponse::Workers)
    }

    pub fn queue_stats(&self) -> Result<OpsResponse, String> {
        self.simple_query("QUEUE.STATS", OpsResponse::QueueStats)
    }

    pub fn list_plans(&self) -> Result<Vec<PlanSummary>, String> {
        let mut reader = self.connect_and_auth()?;
        let command = resp_array(&["PLAN.LIST"]);
        {
            let stream = reader.get_mut();
            stream
                .write_all(&command)
                .map_err(|e| format!("failed to send PLAN.LIST: {e}"))?;
        }

        let response = read_resp_value(&mut reader)?;
        match response {
            RespValue::Array(items) => {
                let mut plans = Vec::new();
                for item in items {
                    match item {
                        RespValue::BulkString(json_str) => {
                            let summary: PlanSummary = serde_json::from_str(&json_str)
                                .map_err(|e| format!("failed to parse plan summary: {e}"))?;
                            plans.push(summary);
                        }
                        other => {
                            return Err(format!(
                                "unexpected item type in PLAN.LIST response: {:?}",
                                other
                            ));
                        }
                    }
                }
                Ok(plans)
            }
            RespValue::Error(msg) => Err(format!("AGQ error: {msg}")),
            other => Err(format!("unexpected AGQ response: {:?}", other)),
        }
    }

    pub fn get_plan(&self, plan_id: &str) -> Result<crate::plan::WorkflowPlan, String> {
        // Validate plan_id to prevent RESP injection and ensure reasonable length
        if !plan_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "invalid plan_id: must contain only alphanumeric characters, underscore, or dash"
                    .to_string(),
            );
        }

        if plan_id.is_empty() {
            return Err("plan_id cannot be empty".to_string());
        }

        if plan_id.len() > 128 {
            return Err("plan_id too long (max 128 characters)".to_string());
        }

        let mut reader = self.connect_and_auth()?;
        let command = resp_array(&["PLAN.GET", plan_id]);
        {
            let stream = reader.get_mut();
            stream
                .write_all(&command)
                .map_err(|e| format!("failed to send PLAN.GET: {e}"))?;
        }

        let response = read_resp_value(&mut reader)?;
        match response {
            RespValue::BulkString(json_str) => {
                let plan: crate::plan::WorkflowPlan = serde_json::from_str(&json_str)
                    .map_err(|e| format!("failed to parse plan: {e}"))?;
                Ok(plan)
            }
            RespValue::Error(msg) => Err(format!("AGQ error: {msg}")),
            other => Err(format!("unexpected AGQ response: {:?}", other)),
        }
    }

    fn simple_query<F>(&self, command: &str, wrap: F) -> Result<OpsResponse, String>
    where
        F: Fn(Vec<String>) -> OpsResponse,
    {
        let mut reader = self.connect_and_auth()?;
        let command_resp = resp_array(&[command]);
        {
            let stream = reader.get_mut();
            stream
                .write_all(&command_resp)
                .map_err(|e| format!("failed to send {command}: {e}"))?;
        }

        let response = read_resp_value(&mut reader)?;
        match response {
            RespValue::Array(items) => {
                let strings = items
                    .into_iter()
                    .filter_map(|v| match v {
                        RespValue::SimpleString(s) | RespValue::BulkString(s) => Some(s),
                        RespValue::Integer(i) => Some(i.to_string()),
                        _ => None,
                    })
                    .collect();
                Ok(wrap(strings))
            }
            RespValue::Error(msg) => Err(format!("AGQ error: {msg}")),
            RespValue::SimpleString(s) | RespValue::BulkString(s) => Ok(wrap(vec![s])),
            other => Err(format!("unexpected AGQ response: {:?}", other)),
        }
    }

    fn connect_and_auth(&self) -> Result<BufReader<TcpStream>, String> {
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

        Ok(reader)
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
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
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
                    assert_eq!(
                        items[1],
                        RespValue::BulkString("{\"plan\": []}".to_string())
                    );
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
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
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

    #[test]
    fn propagate_agq_error_response() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);

            let _auth_req = read_resp_value(&mut reader).expect("read auth request");
            reader.get_mut().write_all(b"+OK\r\n").expect("auth ok");

            let _submit_req = read_resp_value(&mut reader).expect("read submit");
            reader
                .get_mut()
                .write_all(b"-ERR invalid plan\r\n")
                .expect("write error");
        });

        let client = AgqClient::new(AgqConfig {
            addr: addr.to_string(),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(2),
        });

        let result = client.submit_plan("{\"plan\": []}");
        assert!(matches!(result, Err(e) if e.contains("AGQ error")));

        server.join().unwrap();
    }

    #[test]
    fn action_envelope_serializes() {
        let action = ActionEnvelope {
            action_id: "act-1".into(),
            plan_id: "plan-1".into(),
            plan_description: Some("desc".into()),
            jobs_created: 2,
            job_ids: vec!["job-1".into(), "job-2".into()],
        };

        let json = serde_json::to_string(&action).expect("serialize action");
        assert!(json.contains("act-1"));
        assert!(json.contains("job-2"));

        let back: ActionEnvelope = serde_json::from_str(&json).expect("deserialize action");
        assert_eq!(back.job_ids.len(), 2);
        assert_eq!(back.plan_description.as_deref(), Some("desc"));
    }

    #[test]
    fn submits_action_and_parses_response() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
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
                }
                other => panic!("unexpected auth request: {:?}", other),
            }

            // Send OK for AUTH
            reader
                .get_mut()
                .write_all(b"+OK\r\n")
                .expect("write auth ok");

            // Expect ACTION.SUBMIT
            let submit_req = read_resp_value(&mut reader).expect("read submit");
            match submit_req {
                RespValue::Array(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(
                        items[0],
                        RespValue::BulkString("ACTION.SUBMIT".to_string())
                    );
                }
                other => panic!("unexpected submit request: {:?}", other),
            }

            // Respond with bulk string containing ActionEnvelope JSON
            let response_json = r#"{"action_id":"act-123","plan_id":"plan-456","plan_description":null,"jobs_created":2,"job_ids":["job-1","job-2"]}"#;
            let response_bytes = format!("${}\r\n{}\r\n", response_json.len(), response_json);
            reader
                .get_mut()
                .write_all(response_bytes.as_bytes())
                .expect("failed to write response");
        });

        let client = AgqClient::new(AgqConfig {
            addr: addr.to_string(),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(2),
        });

        let action_request = r#"{"plan_id":"plan-456","inputs":{}}"#;
        let result = client
            .submit_action(action_request)
            .expect("submit should succeed");

        assert_eq!(result.action_id, "act-123");
        assert_eq!(result.plan_id, "plan-456");
        assert_eq!(result.job_ids.len(), 2);
        assert_eq!(result.job_ids[0], "job-1");
        assert_eq!(result.job_ids[1], "job-2");

        server.join().unwrap();
    }

    #[test]
    fn action_envelope_validates_jobs_created_match() {
        let valid_envelope = ActionEnvelope {
            action_id: "act-1".into(),
            plan_id: "plan-1".into(),
            plan_description: None,
            jobs_created: 2,
            job_ids: vec!["job-1".into(), "job-2".into()],
        };
        assert!(valid_envelope.validate().is_ok());

        let invalid_envelope = ActionEnvelope {
            action_id: "act-1".into(),
            plan_id: "plan-1".into(),
            plan_description: None,
            jobs_created: 3, // Mismatch!
            job_ids: vec!["job-1".into(), "job-2".into()],
        };
        let result = invalid_envelope.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("jobs_created (3) != job_ids.len() (2)"));
    }

    #[test]
    fn list_plans_returns_summaries() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);

            // Handle AUTH
            let _auth_req = read_resp_value(&mut reader);
            reader.get_mut().write_all(b"+OK\r\n").unwrap();

            // Expect PLAN.LIST
            let list_req = read_resp_value(&mut reader).expect("read list request");
            match list_req {
                RespValue::Array(items) => {
                    assert_eq!(items.len(), 1);
                    assert_eq!(items[0], RespValue::BulkString("PLAN.LIST".to_string()));
                }
                other => panic!("unexpected list request: {:?}", other),
            }

            // Respond with array of plan summaries
            let summary1 = r#"{"plan_id":"plan_123","description":"Test plan","task_count":3,"created_at":"2025-01-19"}"#;
            let summary2 = r#"{"plan_id":"plan_456","description":null,"task_count":1,"created_at":null}"#;
            let response = format!(
                "*2\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
                summary1.len(),
                summary1,
                summary2.len(),
                summary2
            );
            reader.get_mut().write_all(response.as_bytes()).unwrap();
        });

        let config = AgqConfig {
            addr: format!("127.0.0.1:{}", addr.port()),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        let result = client.list_plans();
        server.join().unwrap();

        assert!(result.is_ok());
        let plans = result.unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].plan_id, "plan_123");
        assert_eq!(plans[0].description, Some("Test plan".to_string()));
        assert_eq!(plans[0].task_count, 3);
        assert_eq!(plans[1].plan_id, "plan_456");
    }

    #[test]
    fn list_plans_handles_empty_response() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);
            let _auth_req = read_resp_value(&mut reader);
            reader.get_mut().write_all(b"+OK\r\n").unwrap();
            let _list_req = read_resp_value(&mut reader);

            // Respond with empty array
            reader.get_mut().write_all(b"*0\r\n").unwrap();
        });

        let config = AgqConfig {
            addr: format!("127.0.0.1:{}", addr.port()),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        let result = client.list_plans();
        server.join().unwrap();

        assert!(result.is_ok());
        let plans = result.unwrap();
        assert_eq!(plans.len(), 0);
    }

    #[test]
    fn get_plan_returns_workflow_plan() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);
            let _auth_req = read_resp_value(&mut reader);
            reader.get_mut().write_all(b"+OK\r\n").unwrap();

            // Expect PLAN.GET
            let get_req = read_resp_value(&mut reader).expect("read get request");
            match get_req {
                RespValue::Array(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(items[0], RespValue::BulkString("PLAN.GET".to_string()));
                    assert_eq!(items[1], RespValue::BulkString("plan_abc123".to_string()));
                }
                other => panic!("unexpected get request: {:?}", other),
            }

            // Respond with bulk string plan JSON
            let plan_json = r#"{"tasks":[{"task_number":1,"command":"echo","args":["hello"],"timeout_secs":300}]}"#;
            let response = format!("${}\r\n{}\r\n", plan_json.len(), plan_json);
            reader.get_mut().write_all(response.as_bytes()).unwrap();
        });

        let config = AgqConfig {
            addr: format!("127.0.0.1:{}", addr.port()),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        let result = client.get_plan("plan_abc123");
        server.join().unwrap();

        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].command, "echo");
    }

    #[test]
    fn get_plan_validates_plan_id() {
        let config = AgqConfig {
            addr: "127.0.0.1:6380".to_string(),
            session_key: None,
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        // Test invalid characters
        let result = client.get_plan("plan\n123");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid plan_id"));

        // Test empty
        let result = client.get_plan("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));

        // Test too long
        let long_id = "a".repeat(129);
        let result = client.get_plan(&long_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));

        // Test valid
        let result = client.get_plan("plan_abc-123");
        // Will fail to connect, but validation should pass
        assert!(result.is_err());
        assert!(!result.unwrap_err().contains("invalid plan_id"));
    }

    #[test]
    fn get_plan_handles_agq_error() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().unwrap().0;
            let mut reader = BufReader::new(&mut stream);
            let _auth_req = read_resp_value(&mut reader);
            reader.get_mut().write_all(b"+OK\r\n").unwrap();
            let _get_req = read_resp_value(&mut reader);

            // Respond with error (plan not found)
            reader
                .get_mut()
                .write_all(b"-ERR plan not found\r\n")
                .unwrap();
        });

        let config = AgqConfig {
            addr: format!("127.0.0.1:{}", addr.port()),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        let result = client.get_plan("plan_nonexistent");
        server.join().unwrap();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("plan not found"));
    }

    #[test]
    fn action_submit_full_workflow() {
        // Integration test for full ACTION submit workflow:
        // 1. Client calls PLAN.GET to retrieve plan
        // 2. Client calls ACTION.SUBMIT with plan-id and inputs
        // 3. Server responds with ActionEnvelope containing job_ids

        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return,
        };
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            // First connection: PLAN.GET
            {
                let mut stream = listener.accept().unwrap().0;
                let mut reader = BufReader::new(&mut stream);

                // AUTH request
                let _auth_req = read_resp_value(&mut reader);
                reader.get_mut().write_all(b"+OK\r\n").unwrap();

                // PLAN.GET request
                let _get_req = read_resp_value(&mut reader);
                let plan_response = r#"{"tasks":[{"task_number":1,"command":"echo","args":["hello"],"timeout_secs":300}]}"#;
                reader
                    .get_mut()
                    .write_all(format!("${}\r\n{}\r\n", plan_response.len(), plan_response).as_bytes())
                    .unwrap();
            }

            // Second connection: ACTION.SUBMIT
            {
                let mut stream = listener.accept().unwrap().0;
                let mut reader = BufReader::new(&mut stream);

                // AUTH request
                let _auth_req = read_resp_value(&mut reader);
                reader.get_mut().write_all(b"+OK\r\n").unwrap();

                // ACTION.SUBMIT request
                let _submit_req = read_resp_value(&mut reader);
                let action_response = r#"{"action_id":"action_123","plan_id":"plan_abc","plan_description":"test","jobs_created":1,"job_ids":["job_xyz789"]}"#;
                reader
                    .get_mut()
                    .write_all(format!("${}\r\n{}\r\n", action_response.len(), action_response).as_bytes())
                    .unwrap();
            }
        });

        let config = AgqConfig {
            addr: format!("127.0.0.1:{}", addr.port()),
            session_key: Some("secret".to_string()),
            timeout: Duration::from_secs(5),
        };
        let client = AgqClient::new(config);

        // Step 1: Get plan (validates it exists)
        let plan_result = client.get_plan("plan_abc");
        assert!(plan_result.is_ok());

        // Step 2: Submit action with plan-id and inputs
        let action_json = r#"{"action_id":"action_123","plan_id":"plan_abc","inputs":[{"key":"value"}]}"#;
        let submit_result = client.submit_action(action_json);

        server.join().unwrap();

        assert!(submit_result.is_ok());
        let response = submit_result.unwrap();
        assert_eq!(response.plan_id, "plan_abc");
        assert_eq!(response.jobs_created, 1);
        assert_eq!(response.job_ids.len(), 1);
        assert_eq!(response.job_ids[0], "job_xyz789");
    }
}
