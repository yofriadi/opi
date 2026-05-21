//! Behavioral tests for task 1.3: Anthropic SSE provider.
//!
//! DoD: "fixtures cover text, tool call, usage, error"
//! All tests use fixture strings — no live provider calls (red flag #10).

use opi_ai::anthropic::{AnthropicEvent, AnthropicMapper, ParsedEvent, parse_sse_events};
use opi_ai::message::AssistantContent;
use opi_ai::provider::Provider;
use opi_ai::stream::{AssistantStreamEvent, StopReason};

/// Helper: parse fixture, extract valid events, and map through a stateful mapper.
fn map_fixture(input: &str) -> Vec<AssistantStreamEvent> {
    let events: Vec<AnthropicEvent> = parse_sse_events(input)
        .filter_map(|p| match p {
            ParsedEvent::Valid(e) => Some(e),
            ParsedEvent::Malformed { .. } => None,
        })
        .collect();
    let mut mapper = AnthropicMapper::new();
    events.into_iter().flat_map(|e| mapper.process(e)).collect()
}

/// Helper: collect valid AnthropicEvents from parsed output.
fn collect_valid_events(input: &str) -> Vec<AnthropicEvent> {
    parse_sse_events(input)
        .filter_map(|p| match p {
            ParsedEvent::Valid(e) => Some(e),
            ParsedEvent::Malformed { .. } => None,
        })
        .collect()
}

// --- SSE Parsing Tests ---

#[test]
fn sse_parse_empty_input_yields_no_events() {
    let events = collect_valid_events("");
    assert!(events.is_empty());
}

#[test]
fn sse_parse_single_event() {
    let input = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-5-20250514","stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

"#;
    let events = collect_valid_events(input);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AnthropicEvent::MessageStart { .. }));
}

#[test]
fn sse_parse_ignores_comments() {
    let input = ": this is a comment\n\n";
    let events = collect_valid_events(input);
    assert!(events.is_empty());
}

#[test]
fn sse_parse_skips_unknown_event_types() {
    let input = "event: ping\ndata: {}\n\nevent: done\ndata: [DONE]\n\n";
    let events = collect_valid_events(input);
    assert!(events.is_empty());
}

// --- Text Fixture ---

fn text_fixture() -> &'static str {
    r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_abc","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-5-20250514","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}

event: message_stop
data: {"type":"message_stop"}

"#
}

#[test]
fn text_fixture_yields_all_events() {
    let events = collect_valid_events(text_fixture());
    assert_eq!(events.len(), 7);
}

#[test]
fn text_fixture_maps_to_stream_events() {
    let stream_events = map_fixture(text_fixture());

    // Expected: Start, TextStart, TextDelta("Hello"), TextDelta(" world"), TextEnd, Done
    assert!(matches!(
        stream_events[0],
        AssistantStreamEvent::Start { .. }
    ));
    assert!(matches!(
        stream_events[1],
        AssistantStreamEvent::TextStart { .. }
    ));

    if let AssistantStreamEvent::TextDelta { delta, .. } = &stream_events[2] {
        assert_eq!(delta, "Hello");
    } else {
        panic!("expected TextDelta at index 2");
    }

    if let AssistantStreamEvent::TextDelta { delta, .. } = &stream_events[3] {
        assert_eq!(delta, " world");
    } else {
        panic!("expected TextDelta at index 3");
    }

    assert!(matches!(
        stream_events[4],
        AssistantStreamEvent::TextEnd { .. }
    ));
    assert!(matches!(
        &stream_events[5],
        AssistantStreamEvent::Done { reason, .. } if *reason == StopReason::Stop
    ));
}

#[test]
fn text_fixture_done_event_has_full_content() {
    let stream_events = map_fixture(text_fixture());

    if let AssistantStreamEvent::Done { message, .. } = &stream_events[5] {
        let text_content: Vec<_> = message
            .content
            .iter()
            .filter_map(|c| match c {
                AssistantContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text_content, vec!["Hello world"]);
    } else {
        panic!("expected Done event");
    }
}

// --- Tool Call Fixture ---

fn tool_call_fixture() -> &'static str {
    r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_tool","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-5-20250514","stop_reason":null,"usage":{"input_tokens":50,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_abc","name":"read_file","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"/tmp/test\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":100}}

event: message_stop
data: {"type":"message_stop"}

"#
}

#[test]
fn tool_call_fixture_yields_tool_events() {
    let events = collect_valid_events(tool_call_fixture());
    assert_eq!(events.len(), 6);
}

#[test]
fn tool_call_fixture_maps_to_stream_events() {
    let stream_events = map_fixture(tool_call_fixture());

    // Start, ToolCallStart, ToolCallDelta, ToolCallEnd, Done(tool_use)
    assert!(matches!(
        stream_events[0],
        AssistantStreamEvent::Start { .. }
    ));
    assert!(matches!(
        stream_events[1],
        AssistantStreamEvent::ToolCallStart { .. }
    ));

    if let AssistantStreamEvent::ToolCallDelta { delta, .. } = &stream_events[2] {
        assert!(delta.contains("path"));
    } else {
        panic!("expected ToolCallDelta at index 2");
    }

    if let AssistantStreamEvent::ToolCallEnd { tool_call, .. } = &stream_events[3] {
        assert_eq!(tool_call.name, "read_file");
        assert_eq!(tool_call.id, "toolu_abc");
    } else {
        panic!("expected ToolCallEnd at index 3");
    }

    if let AssistantStreamEvent::Done { reason, .. } = &stream_events[4] {
        assert_eq!(*reason, StopReason::ToolUse);
    } else {
        panic!("expected Done at index 4");
    }
}

// --- Usage Fixture ---

#[test]
fn usage_captured_from_message_start() {
    let stream_events = map_fixture(text_fixture());

    if let AssistantStreamEvent::Start { partial } = &stream_events[0] {
        assert_eq!(partial.usage.input_tokens, 25);
    } else {
        panic!("expected Start event");
    }
}

#[test]
fn usage_updated_in_done_event() {
    let stream_events = map_fixture(text_fixture());

    if let AssistantStreamEvent::Done { message, .. } = &stream_events[5] {
        assert_eq!(message.usage.output_tokens, 15);
        assert_eq!(message.usage.input_tokens, 25);
    } else {
        panic!("expected Done event");
    }
}

#[test]
fn tool_call_usage_tracked() {
    let stream_events = map_fixture(tool_call_fixture());

    if let AssistantStreamEvent::Done { message, .. } = &stream_events[4] {
        assert_eq!(message.usage.input_tokens, 50);
        assert_eq!(message.usage.output_tokens, 100);
    } else {
        panic!("expected Done event");
    }
}

// --- Error Fixture ---

fn error_fixture() -> &'static str {
    r#"event: error
data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}

"#
}

#[test]
fn error_fixture_parsed_as_error() {
    let events = collect_valid_events(error_fixture());
    assert!(matches!(events[0], AnthropicEvent::Error { .. }));
}

#[test]
fn error_event_maps_to_stream_error() {
    let stream_events = map_fixture(error_fixture());

    assert_eq!(stream_events.len(), 1);
    assert!(matches!(
        &stream_events[0],
        AssistantStreamEvent::Error { reason, .. } if *reason == StopReason::Error
    ));
}

// --- Stop Reason Mapping Tests ---

#[test]
fn stop_reason_end_turn_maps_to_stop() {
    let stream_events = map_fixture(text_fixture());

    if let AssistantStreamEvent::Done { reason, .. } = &stream_events[5] {
        assert_eq!(*reason, StopReason::Stop);
    } else {
        panic!("expected Done with StopReason::Stop");
    }
}

#[test]
fn stop_reason_tool_use_maps_correctly() {
    let stream_events = map_fixture(tool_call_fixture());

    if let AssistantStreamEvent::Done { reason, .. } = &stream_events[4] {
        assert_eq!(*reason, StopReason::ToolUse);
    } else {
        panic!("expected Done with StopReason::ToolUse");
    }
}

// --- AnthropicProvider Tests ---

#[test]
fn anthropic_provider_id() {
    let provider = opi_ai::anthropic::AnthropicProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "anthropic");
}

#[test]
fn anthropic_provider_models_not_empty() {
    let provider = opi_ai::anthropic::AnthropicProvider::new("test-key".into(), None);
    assert!(!provider.models().is_empty());
}

// --- Mixed text + tool call fixture ---

fn mixed_fixture() -> &'static str {
    r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_mix","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-5-20250514","stop_reason":null,"usage":{"input_tokens":30,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Let me read that file."}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_123","name":"read_file","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"src/main.rs\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":45}}

event: message_stop
data: {"type":"message_stop"}

"#
}

#[test]
fn mixed_fixture_produces_text_then_tool_call() {
    let stream_events = map_fixture(mixed_fixture());

    // Start, TextStart, TextDelta, TextEnd, ToolCallStart, ToolCallDelta, ToolCallEnd, Done
    assert_eq!(stream_events.len(), 8);
    assert!(matches!(
        stream_events[1],
        AssistantStreamEvent::TextStart { .. }
    ));
    assert!(matches!(
        stream_events[4],
        AssistantStreamEvent::ToolCallStart { .. }
    ));

    if let AssistantStreamEvent::Done { message, reason } = &stream_events[7] {
        assert_eq!(*reason, StopReason::ToolUse);
        assert_eq!(message.content.len(), 2);
    } else {
        panic!("expected Done event");
    }
}

// --- Malformed SSE Tests ---

#[test]
fn malformed_sse_data_produces_malformed_event() {
    let input = r#"event: message_start
data: {invalid json here}

"#;
    let parsed: Vec<_> = parse_sse_events(input).collect();
    assert_eq!(parsed.len(), 1);
    assert!(
        matches!(&parsed[0], ParsedEvent::Malformed { event_type, .. } if event_type == "message_start"),
        "expected Malformed event for invalid JSON data"
    );
}

#[test]
fn malformed_and_valid_events_coexist() {
    let input = r#"event: message_start
data: {bad json}

event: message_stop
data: {"type":"message_stop"}

"#;
    let parsed: Vec<_> = parse_sse_events(input).collect();
    assert_eq!(parsed.len(), 2);
    assert!(matches!(parsed[0], ParsedEvent::Malformed { .. }));
    assert!(matches!(parsed[1], ParsedEvent::Valid(_)));
}

// --- CRLF SSE Tests ---

#[test]
fn sse_parse_handles_crlf_line_endings() {
    // Simulate real HTTP SSE with CRLF line endings
    let input = "event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-5-20250514\",\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\r\n\r\n";

    let events = collect_valid_events(input);
    assert_eq!(events.len(), 1);
    assert!(
        matches!(events[0], AnthropicEvent::MessageStart { .. }),
        "CRLF-delimited SSE should parse correctly"
    );
}

#[test]
fn sse_parse_handles_crlf_full_fixture() {
    // Build a CRLF version of the text fixture
    let lf_fixture = text_fixture();
    let crlf_fixture = lf_fixture.replace('\n', "\r\n");

    let events = collect_valid_events(&crlf_fixture);
    assert_eq!(
        events.len(),
        7,
        "CRLF fixture should parse same as LF fixture"
    );
}

#[tokio::test]
async fn drain_sse_events_handles_crlf_stream() {
    // Build a CRLF SSE stream with event separator
    let lf_input = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_crlf\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-5-20250514\",\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n";
    let crlf_input = lf_input.replace('\n', "\r\n");

    let provider = opi_ai::anthropic::AnthropicProvider::new("test-key".into(), None);
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut stream = provider.stream_from_sse(&crlf_input, cancel);

    use futures_util::StreamExt;
    let first = stream.next().await.expect("should have an event");
    assert!(first.is_ok(), "CRLF SSE should produce valid stream events");
}
