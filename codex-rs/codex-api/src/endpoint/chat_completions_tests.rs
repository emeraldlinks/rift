use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn test_responses_to_chat_request_basic() {
    let request = ResponsesApiRequest {
        model: "gpt-4".to_string(),
        instructions: "You are a helpful assistant.".to_string(),
        input: vec![ResponseItem::Message {
            id: Some("msg-1".to_string()),
            role: "user".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Hello!".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }],
        tools: None,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
        access_programs: None,
    };

    let chat_request = responses_to_chat_request(&request);

    assert_eq!(chat_request.model, "gpt-4");
    assert!(chat_request.stream);
    assert_eq!(chat_request.messages.len(), 2);
    assert_eq!(chat_request.messages[0].role, "system");
    assert_eq!(
        chat_request.messages[0].content.as_deref(),
        Some("You are a helpful assistant.")
    );
    assert_eq!(chat_request.messages[1].role, "user");
    assert_eq!(chat_request.messages[1].content.as_deref(), Some("Hello!"));
    assert!(chat_request.tools.is_none());
}

#[test]
fn test_responses_to_chat_request_with_function_call() {
    let request = ResponsesApiRequest {
        model: "gpt-4".to_string(),
        instructions: String::new(),
        input: vec![
            ResponseItem::Message {
                id: Some("msg-1".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Run ls".to_string(),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::FunctionCall {
                id: "fc-1".to_string(),
                call_id: "call-1".to_string(),
                name: "shell".to_string(),
                arguments: "{\"command\":\"ls\"}".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                id: "fco-1".to_string(),
                call_id: "call-1".to_string(),
                output: "file1.txt\nfile2.txt".to_string(),
            },
        ],
        tools: None,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
        access_programs: None,
    };

    let chat_request = responses_to_chat_request(&request);

    // No system message since instructions is empty
    assert_eq!(chat_request.messages.len(), 3);

    // User message
    assert_eq!(chat_request.messages[0].role, "user");
    assert_eq!(chat_request.messages[0].content.as_deref(), Some("Run ls"));

    // Assistant tool call
    assert_eq!(chat_request.messages[1].role, "assistant");
    assert!(chat_request.messages[1].content.is_none());
    let tool_calls = chat_request.messages[1].tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call-1");
    assert_eq!(tool_calls[0].function.name, "shell");
    assert_eq!(tool_calls[0].function.arguments, "{\"command\":\"ls\"}");

    // Tool output
    assert_eq!(chat_request.messages[2].role, "tool");
    assert_eq!(
        chat_request.messages[2].content.as_deref(),
        Some("file1.txt\nfile2.txt")
    );
    assert_eq!(
        chat_request.messages[2].tool_call_id.as_deref(),
        Some("call-1")
    );
}

#[test]
fn test_tool_choice_conversion() {
    // "none" should become None (no tools passed)
    let request = ResponsesApiRequest {
        model: "gpt-4".to_string(),
        instructions: String::new(),
        input: vec![],
        tools: None,
        tool_choice: "none".to_string(),
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
        access_programs: None,
    };

    let chat_request = responses_to_chat_request(&request);
    assert!(chat_request.tool_choice.is_none());

    // "required" should become Some("required")
    let mut request = request;
    request.tool_choice = "required".to_string();
    let chat_request = responses_to_chat_request(&request);
    assert_eq!(chat_request.tool_choice.as_deref(), Some("required"));
}
