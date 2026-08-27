use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::rate_limits::parse_all_rate_limits;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::protocol::TokenUsage;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

const OPENAI_MODEL_HEADER: &str = "openai-model";
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Spawn a Chat Completions SSE stream, converting events to the canonical
/// `ResponseEvent` type used by the agent core.
pub fn spawn_chat_completions_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let rate_limit_snapshots = parse_all_rate_limits(&stream_response.headers);
    let server_model = stream_response
        .headers
        .get(OPENAI_MODEL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    let upstream_request_id = stream_response
        .headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);

    tokio::spawn(async move {
        if let Some(model) = server_model {
            let _ = tx_event.send(Ok(ResponseEvent::ServerModel(model))).await;
        }
        for snapshot in rate_limit_snapshots {
            let _ = tx_event.send(Ok(ResponseEvent::RateLimits(snapshot))).await;
        }

        let stream = stream_response.bytes;
        let mut event_stream = stream.eventsource();
        let mut response_id: Option<String> = None;
        let mut finish_reason: Option<String> = None;
        let mut usage: Option<TokenUsage> = None;

        loop {
            let event = {
                match timeout(idle_timeout, event_stream.next()).await {
                    Ok(Some(Ok(event))) => event,
                    Ok(Some(Err(e))) => {
                        debug!("SSE stream error: {e}");
                        let _ = tx_event
                            .send(Err(ApiError::Stream(format!("SSE error: {e}"))))
                            .await;
                        break;
                    }
                    Ok(None) => {
                        debug!("SSE stream ended");
                        break;
                    }
                    Err(_) => {
                        debug!("SSE stream idle timeout");
                        let _ = tx_event
                            .send(Err(ApiError::Stream("stream idle timeout".to_string())))
                            .await;
                        break;
                    }
                }
            };

            let event_type = if event.event.is_empty() {
                "message"
            } else {
                event.event.as_str()
            };
            let data_str = event.data.as_str();

            trace!("SSE event: type={event_type} data={data_str}");

            match event_type {
                "message" | "" => {
                    if data_str == "[DONE]" {
                        // Stream complete
                        let resp_id = response_id
                            .clone()
                            .unwrap_or_else(|| format!("chatcmpl-{}", uuid::Uuid::new_v4()));
                        let _ = tx_event
                            .send(Ok(ResponseEvent::Completed {
                                response_id: resp_id,
                                token_usage: usage,
                                end_turn: finish_reason
                                    .as_deref()
                                    .map(|r| r == "stop" || r == "eos"),
                            }))
                            .await;
                        break;
                    }

                    match serde_json::from_str::<ChatCompletionChunk>(data_str) {
                        Ok(chunk) => {
                            if response_id.is_none() {
                                response_id = Some(chunk.id.clone());
                                let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
                            }

                            if let Some(usage_data) = chunk.usage {
                                usage = Some(TokenUsage {
                                    input_tokens: usage_data.prompt_tokens as i64,
                                    output_tokens: usage_data.completion_tokens as i64,
                                    total_tokens: usage_data.total_tokens as i64,
                                    ..TokenUsage::default()
                                });
                            }

                            for choice in &chunk.choices {
                                if let Some(reason) = &choice.finish_reason {
                                    finish_reason = Some(reason.clone());
                                }

                                if let Some(ref delta) = choice.delta {
                                    // Text content
                                    if let Some(ref content) = delta.content {
                                        if !content.is_empty() {
                                            let _ = tx_event
                                                .send(Ok(ResponseEvent::OutputTextDelta(
                                                    content.clone(),
                                                )))
                                                .await;
                                        }
                                    }

                                    // Tool calls
                                    if let Some(ref tool_calls) = delta.tool_calls {
                                        for tc in tool_calls {
                                            let item_id =
                                                format!("chatcmpl-{}-tc-{}", chunk.id, tc.index);
                                            let _ = tx_event
                                                .send(Ok(ResponseEvent::ToolCallInputDelta {
                                                    item_id,
                                                    call_id: tc.id.clone(),
                                                    delta: tc
                                                        .function
                                                        .as_ref()
                                                        .and_then(|f| f.arguments.clone())
                                                        .unwrap_or_default(),
                                                }))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Failed to parse Chat Completions chunk: {e}");
                        }
                    }
                }
                _ => {
                    trace!("Ignoring SSE event type: {event_type}");
                }
            }
        }

        drop(telemetry);
    });

    ResponseStream {
        rx_event,
        upstream_request_id,
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    id: String,
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<ChunkUsage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    delta: Option<ChunkDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkToolCallFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCallFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}
