use crate::auth::SharedAuthProvider;
use crate::common::ResponseStream;
use crate::common::ResponsesApiRequest;
use crate::endpoint::chat_completions_sse::spawn_chat_completions_stream;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::Compression;
use crate::requests::headers::build_session_headers;
use crate::requests::headers::insert_header;
use crate::requests::headers::subagent_header;
use crate::telemetry::SseTelemetry;
use codex_client::EncodedJsonBody;
use codex_client::HttpTransport;
use codex_client::RequestCompression;
use codex_client::RequestTelemetry;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::sync::OnceLock;
use tracing::instrument;

/// Chat Completions message format.
#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatToolCallFunction,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Tool definition in Chat Completions format.
#[derive(Debug, Serialize)]
pub struct ChatToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatFunctionDefinition,
}

#[derive(Debug, Serialize)]
pub struct ChatFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Request body for the Chat Completions API.
#[derive(Debug, Serialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    pub parallel_tool_calls: bool,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatStreamOptions {
    pub include_usage: bool,
}

pub struct ChatCompletionsClient<T: HttpTransport> {
    session: EndpointSession<T>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport> ChatCompletionsClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        self,
        _request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            session: self.session.with_request_telemetry(_request),
            sse_telemetry: sse,
        }
    }

    fn path() -> &'static str {
        "chat/completions"
    }

    /// Stream a Chat Completions request. The `ResponsesApiRequest` is the
    /// canonical format used by the agent core; we convert it here.
    #[instrument(
        name = "chat_completions.stream_request",
        level = "info",
        skip_all,
        fields(
            transport = "chat_completions_http",
            http.method = "POST",
            api.path = "chat/completions"
        )
    )]
    pub async fn stream_request(
        &self,
        request: ResponsesApiRequest,
        options: ChatCompletionsOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ChatCompletionsOptions {
            session_id,
            thread_id,
            session_source,
            extra_headers,
            compression,
            turn_state: _,
        } = options;

        let chat_request = responses_to_chat_request(&request);
        let body = EncodedJsonBody::encode(&chat_request).map_err(|e| {
            ApiError::Stream(format!("failed to encode chat completions request: {e}"))
        })?;

        let mut headers = extra_headers;
        if let Some(ref thread_id) = thread_id {
            insert_header(&mut headers, "x-client-request-id", thread_id);
        }
        headers.extend(build_session_headers(session_id, thread_id));
        if let Some(subagent) = subagent_header(&session_source) {
            insert_header(&mut headers, "x-openai-subagent", &subagent);
        }

        let request_compression = match compression {
            Compression::None => RequestCompression::None,
            Compression::Zstd => RequestCompression::Zstd,
        };

        let stream_response = self
            .session
            .stream_encoded_json_with(Method::POST, Self::path(), headers, Some(body), |req| {
                req.headers.insert(
                    http::header::ACCEPT,
                    HeaderValue::from_static("text/event-stream"),
                );
                req.compression = request_compression;
            })
            .await?;

        Ok(spawn_chat_completions_stream(
            stream_response,
            self.session.provider().stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}

#[derive(Default)]
pub struct ChatCompletionsOptions {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub session_source: Option<SessionSource>,
    pub extra_headers: HeaderMap,
    pub compression: Compression,
    pub turn_state: Option<Arc<OnceLock<String>>>,
}

/// Convert a Responses API request to Chat Completions format.
pub fn responses_to_chat_request(request: &ResponsesApiRequest) -> ChatCompletionsRequest {
    let messages = responses_input_to_messages(&request.input, &request.instructions);
    let tools = request.tools.as_ref().and_then(|raw| {
        let value: Value = serde_json::from_str(raw.as_raw_value().get()).ok()?;
        let arr = value.as_array()?;
        let mut chat_tools = Vec::new();
        for tool in arr {
            if let Some(func) = tool.get("function") {
                let name = func.get("name")?.as_str()?.to_string();
                let description = func
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let parameters = func
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));
                chat_tools.push(ChatToolDefinition {
                    tool_type: "function".to_string(),
                    function: ChatFunctionDefinition {
                        name,
                        description,
                        parameters,
                    },
                });
            }
        }
        (!chat_tools.is_empty()).then_some(chat_tools)
    });

    let tool_choice = match request.tool_choice.as_str() {
        "auto" => Some("auto".to_string()),
        "required" => Some("required".to_string()),
        "none" => None,
        _ => Some(request.tool_choice.clone()),
    };

    ChatCompletionsRequest {
        model: request.model.clone(),
        messages,
        tools,
        tool_choice,
        parallel_tool_calls: request.parallel_tool_calls,
        stream: true,
        stream_options: Some(ChatStreamOptions {
            include_usage: true,
        }),
        temperature: None,
        max_tokens: None,
        service_tier: request.service_tier.clone(),
    }
}

/// Convert Responses API input items + instructions into Chat Completions messages.
fn responses_input_to_messages(
    input: &[codex_protocol::models::ResponseItem],
    instructions: &str,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    if !instructions.is_empty() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(instructions.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for item in input {
        match item {
            codex_protocol::models::ResponseItem::Message { role, content, .. } => {
                let text = content
                    .iter()
                    .filter_map(|part| match part {
                        codex_protocol::models::ContentItem::OutputText { text } => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");

                messages.push(ChatMessage {
                    role: role.to_string(),
                    content: Some(text),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            codex_protocol::models::ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: call_id.clone(),
                        call_type: "function".to_string(),
                        function: ChatToolCallFunction {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }]),
                    tool_call_id: None,
                });
            }
            codex_protocol::models::ResponseItem::FunctionCallOutput {
                call_id, output, ..
            } => {
                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(output.to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: call_id.clone(),
                });
            }
            _ => {
                // Other item types (reasoning, etc.) are skipped for now.
            }
        }
    }

    messages
}

#[cfg(test)]
#[path = "chat_completions_tests.rs"]
mod tests;
