use serde::{Deserialize, Serialize};
use std::fmt;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 16000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[derive(Debug)]
pub enum ClientError {
    Http(reqwest::Error),
    Parse(serde_json::Error),
    Api {
        status: u16,
        error_type: String,
        message: String,
    },
    /// The agentic loop in `turn::run_turn` stopped without a final answer
    /// after making this many tool-use round trips — not an API-level
    /// error, so it's kept out of the `Api` variant's real HTTP statuses.
    ToolLoopLimit(usize),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Http(e) => write!(f, "request to the Anthropic API failed: {e}"),
            ClientError::Parse(e) => write!(f, "could not parse the Anthropic API response: {e}"),
            ClientError::Api {
                status,
                error_type,
                message,
            } => write!(f, "Anthropic API error ({status} {error_type}): {message}"),
            ClientError::ToolLoopLimit(limit) => write!(
                f,
                "stopped after {limit} tool-use round trips without a final answer"
            ),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<reqwest::Error> for ClientError {
    fn from(e: reqwest::Error) -> Self {
        ClientError::Http(e)
    }
}

/// Abstraction over "send a Messages API request" so the agentic loop in
/// `turn` can be tested against a fake implementation instead of the real
/// network.
pub trait MessagesApi {
    fn send(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: Option<&str>,
    ) -> Result<MessagesResponse, ClientError>;
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::blocking::Client,
    api_key: String,
    model: String,
    api_url: String,
}

impl Client {
    pub fn new(api_key: String, model: String) -> Self {
        Self::with_base_url(API_URL.to_string(), api_key, model)
    }

    /// Same as `new`, but talks to `api_url` instead of the real Anthropic
    /// endpoint. Exists for pointing at a local mock server in tests; real
    /// callers should use `new`.
    pub fn with_base_url(api_url: String, api_key: String, model: String) -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            api_key,
            model,
            api_url,
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    tools: &'a [ToolDefinition],
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
}

impl MessagesApi for Client {
    fn send(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: Option<&str>,
    ) -> Result<MessagesResponse, ClientError> {
        let body = MessagesRequest {
            model: &self.model,
            max_tokens: MAX_TOKENS,
            messages,
            tools,
            system,
        };
        let response = self
            .http
            .post(&self.api_url)
            .header("content-type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()?;

        let status = response.status();
        let text = response.text()?;

        if !status.is_success() {
            return Err(match serde_json::from_str::<ErrorResponse>(&text) {
                Ok(parsed) => ClientError::Api {
                    status: status.as_u16(),
                    error_type: parsed.error.error_type,
                    message: parsed.error.message,
                },
                Err(_) => ClientError::Api {
                    status: status.as_u16(),
                    error_type: "unknown".to_string(),
                    message: text,
                },
            });
        }

        serde_json::from_str(&text).map_err(ClientError::Parse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_text_content_block() {
        let msg = Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: "hello".to_string(),
            }],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            })
        );
    }

    #[test]
    fn serializes_tool_result_content_block_omitting_is_error_when_none() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "toolu_1".to_string(),
            content: "ok".to_string(),
            is_error: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "tool_result",
                "tool_use_id": "toolu_1",
                "content": "ok"
            })
        );
    }

    #[test]
    fn deserializes_tool_use_response_from_the_documented_shape() {
        // Matches the Anthropic docs' tool-use response example.
        let raw = r#"{
            "id": "msg_123",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check the weather."},
                {"type": "tool_use", "id": "toolu_abc123", "name": "get_weather", "input": {"location": "Paris"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        }"#;
        let parsed: MessagesResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.id, "msg_123");
        assert_eq!(parsed.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(parsed.content.len(), 2);
        assert_eq!(
            parsed.content[1],
            ContentBlock::ToolUse {
                id: "toolu_abc123".to_string(),
                name: "get_weather".to_string(),
                input: serde_json::json!({"location": "Paris"}),
            }
        );
    }

    #[test]
    fn deserializes_error_response() {
        let raw = r#"{"type": "error", "error": {"type": "invalid_request_error", "message": "bad input"}}"#;
        let parsed: ErrorResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.error.error_type, "invalid_request_error");
        assert_eq!(parsed.error.message, "bad input");
    }

    // The tests above cover serialization/parsing in isolation. These two
    // drive `Client::send` itself against a real (local) HTTP server, so
    // the request construction (URL, headers, JSON body) and response
    // handling are exercised end-to-end, not just the types.

    /// A minimal single-request HTTP/1.1 server: accepts one connection,
    /// reads the request (headers + Content-Length body), hands it to
    /// `handle`, and writes back whatever `handle` returns as the raw
    /// bytes after the request line. Runs on a background thread; the
    /// caller must make exactly one request against the returned address.
    fn spawn_mock_server(
        handle: impl FnOnce(&str, &str) -> String + Send + 'static,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let join = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let mut headers_end = None;
            while headers_end.is_none() {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0, "connection closed before headers were complete");
                buf.extend_from_slice(&chunk[..n]);
                headers_end = buf.windows(4).position(|w| w == b"\r\n\r\n");
            }
            let headers_end = headers_end.unwrap();
            let header_text = String::from_utf8_lossy(&buf[..headers_end]).into_owned();
            let content_length: usize = header_text
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap_or(0);
            let mut body = buf[headers_end + 4..].to_vec();
            while body.len() < content_length {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0, "connection closed before body was complete");
                body.extend_from_slice(&chunk[..n]);
            }
            let body_text = String::from_utf8_lossy(&body).into_owned();
            let response = handle(&header_text, &body_text);
            stream.write_all(response.as_bytes()).unwrap();
        });
        (addr, join)
    }

    #[test]
    fn client_send_makes_a_real_http_request_with_the_right_headers_and_body() {
        let (addr, join) = spawn_mock_server(|headers, body| {
            assert!(headers.to_lowercase().contains("x-api-key: test-key"));
            assert!(headers
                .to_lowercase()
                .contains("anthropic-version: 2023-06-01"));
            let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(parsed["model"], "test-model");
            assert_eq!(parsed["messages"][0]["content"][0]["text"], "hi");

            let response_body = r#"{"id":"msg_x","role":"assistant","content":[{"type":"text","text":"hello back"}],"stop_reason":"end_turn","usage":{"input_tokens":3,"output_tokens":4}}"#;
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            )
        });

        let client = Client::with_base_url(
            format!("http://{addr}/v1/messages"),
            "test-key".to_string(),
            "test-model".to_string(),
        );
        let messages = vec![Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: "hi".to_string(),
            }],
        }];
        let response = client.send(&messages, &[], None).unwrap();

        assert_eq!(response.id, "msg_x");
        assert_eq!(
            response.content,
            vec![ContentBlock::Text {
                text: "hello back".to_string()
            }]
        );
        assert_eq!(response.usage.input_tokens, 3);
        join.join().unwrap();
    }

    #[test]
    fn client_send_turns_a_4xx_response_into_an_api_error() {
        let (addr, join) = spawn_mock_server(|_headers, _body| {
            let response_body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
            format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            )
        });

        let client = Client::with_base_url(
            format!("http://{addr}/v1/messages"),
            "bad-key".to_string(),
            "test-model".to_string(),
        );
        let err = client.send(&[], &[], None).unwrap_err();
        match err {
            ClientError::Api {
                status,
                error_type,
                message,
            } => {
                assert_eq!(status, 401);
                assert_eq!(error_type, "authentication_error");
                assert!(message.contains("invalid x-api-key"));
            }
            other => panic!("expected ClientError::Api, got {other:?}"),
        }
        join.join().unwrap();
    }
}
