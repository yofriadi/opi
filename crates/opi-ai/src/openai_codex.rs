use crate::http::HttpClient;
use crate::message::{AssistantContent, OutputContent};
use crate::provider::{EventStream, ModelInfo, Provider, ProviderError, Request};
use crate::stream::AssistantStreamEvent;
use std::sync::Arc;

pub struct OpenAiCodexProvider {
    access_token: String,
    account_id: String,
    base_url: String,
    session_id: String,
    models: Vec<ModelInfo>,
    client: Arc<HttpClient>,
}

impl std::fmt::Debug for OpenAiCodexProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCodexProvider")
            .field("access_token", &"[REDACTED]")
            .field("account_id", &self.account_id)
            .field("base_url", &self.base_url)
            .field("session_id", &self.session_id)
            .field("models", &self.models)
            .field("client", &"<HttpClient>")
            .finish()
    }
}

impl OpenAiCodexProvider {
    pub fn new(
        access_token: String,
        account_id: String,
        mut base_url: String,
        client: Arc<HttpClient>,
    ) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        if base_url.ends_with('/') {
            base_url.pop();
        }
        let models = vec![
            ModelInfo {
                id: "gpt-5.3-codex-spark".to_string(),
                display_name: "gpt-5.3-codex-spark".to_string(),
                context_window: 128_000,
                max_output_tokens: 32_000,
                supports_images: false,
                supports_streaming: true,
                supports_thinking: true,
            },
            ModelInfo {
                id: "gpt-5.4".to_string(),
                display_name: "gpt-5.4".to_string(),
                context_window: 1_050_000,
                max_output_tokens: 128_000,
                supports_images: false,
                supports_streaming: true,
                supports_thinking: true,
            },
            ModelInfo {
                id: "gpt-5.4-mini".to_string(),
                display_name: "gpt-5.4-mini".to_string(),
                context_window: 400_000,
                max_output_tokens: 128_000,
                supports_images: false,
                supports_streaming: true,
                supports_thinking: true,
            },
            ModelInfo {
                id: "gpt-5.5".to_string(),
                display_name: "gpt-5.5".to_string(),
                context_window: 1_050_000,
                max_output_tokens: 128_000,
                supports_images: false,
                supports_streaming: true,
                supports_thinking: true,
            },
        ];

        Self {
            access_token,
            account_id,
            base_url,
            session_id,
            models,
            client,
        }
    }

    pub(crate) fn build_request_body(&self, request: &Request) -> serde_json::Value {
        let model_id = request
            .model
            .split_once(':')
            .map(|(_, id)| id)
            .unwrap_or(&request.model);

        let mut input = Vec::new();

        for msg in &request.messages {
            match msg {
                crate::message::Message::User(u) => {
                    let content: Vec<serde_json::Value> = u
                        .content
                        .iter()
                        .map(|c| match c {
                            crate::message::InputContent::Text { text } => {
                                serde_json::json!({"type": "input_text", "text": text})
                            }
                            crate::message::InputContent::Image { source, media_type } => {
                                let image_url = match source {
                                    crate::message::ImageSource::Url { url } => url.clone(),
                                    crate::message::ImageSource::Base64 { data } => {
                                        format!("data:{};base64,{}", media_type.as_str(), data)
                                    }
                                    crate::message::ImageSource::Bytes { data } => {
                                        format!(
                                            "data:{};base64,{}",
                                            media_type.as_str(),
                                            base64::Engine::encode(
                                                &base64::engine::general_purpose::STANDARD,
                                                data,
                                            )
                                        )
                                    }
                                };
                                serde_json::json!({
                                    "type": "input_image",
                                    "image_url": image_url,
                                })
                            }
                        })
                        .collect();
                    if let Some(text_val) = (content.len() == 1)
                        .then(|| content[0].get("text"))
                        .flatten()
                    {
                        input.push(serde_json::json!({
                            "role": "user",
                            "content": text_val,
                        }));
                        continue;
                    }
                    input.push(serde_json::json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                crate::message::Message::Assistant(a) => {
                    let mut tool_calls_json = Vec::new();
                    let mut text_parts = Vec::new();
                    for c in &a.content {
                        match c {
                            AssistantContent::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            AssistantContent::ToolCall { tool_call } => {
                                tool_calls_json.push(serde_json::json!({
                                    "type": "function_call",
                                    "id": tool_call.id,
                                    "call_id": tool_call.id,
                                    "name": tool_call.name,
                                    "arguments": tool_call.arguments,
                                }));
                            }
                            AssistantContent::Thinking { .. } => {}
                        }
                    }
                    if !text_parts.is_empty() {
                        input.push(serde_json::json!({
                            "role": "assistant",
                            "content": text_parts.join(""),
                        }));
                    }
                    for tc in tool_calls_json {
                        input.push(tc);
                    }
                }
                crate::message::Message::ToolResult(t) => {
                    let content_text: String = t
                        .content
                        .iter()
                        .map(|c| match c {
                            OutputContent::Text { text } => text.clone(),
                            OutputContent::Image { media_type, .. } => {
                                format!("[image: {}]", media_type.as_str())
                            }
                        })
                        .collect();
                    input.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": t.tool_call_id,
                        "output": content_text,
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "model": model_id,
            "store": false,
            "stream": true,
            "input": input,
            "include": ["reasoning.encrypted_content"],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "prompt_cache_key": self.session_id,
        });

        if let Some(sys) = &request.system {
            body["instructions"] = serde_json::Value::String(sys.clone());
        }

        if !request.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(
                request
                    .tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        })
                    })
                    .collect(),
            );
        }

        if request.thinking.enabled {
            let effort = match request.thinking.budget_tokens {
                Some(b) if b < 1024 => "low",
                Some(b) if (1024..4096).contains(&b) => "medium",
                Some(b) if b >= 4096 => "high",
                _ => "medium",
            };
            body["reasoning"] = serde_json::json!({
                "effort": effort,
                "summary": "auto",
            });
        }

        body
    }

    #[allow(clippy::too_many_arguments)]
    async fn stream_http(
        http_client: reqwest::Client,
        access_token: String,
        account_id: String,
        session_id: String,
        base_url: String,
        body: &serde_json::Value,
        cancel: tokio_util::sync::CancellationToken,
        tx: &tokio::sync::mpsc::Sender<Result<AssistantStreamEvent, ProviderError>>,
    ) -> Result<(), ProviderError> {
        let user_agent = get_user_agent();

        let response = http_client
            .post(format!("{}/codex/responses", base_url))
            .header("authorization", format!("Bearer {access_token}"))
            .header("chatgpt-account-id", &account_id)
            .header("originator", "pi")
            .header("user-agent", &user_agent)
            .header("openai-beta", "responses=experimental")
            .header("accept", "text/event-stream")
            .header("content-type", "application/json")
            .header("session-id", &session_id)
            .header("x-client-request-id", &session_id)
            .body(serde_json::to_string(body).expect("body is a valid Value"))
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let error_body = response.text().await.unwrap_or_default();
            return Err(crate::openai_responses::map_http_status(
                status,
                &error_body,
                &headers,
            ));
        }

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut mapper = crate::openai_responses::ResponsesMapper::new("openai-codex");

        use futures_util::StreamExt;
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => {
                    return Ok(());
                }
                chunk = byte_stream.next() => {
                    match chunk {
                        Some(c) => c,
                        None => break,
                    }
                }
            };

            let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            for frame in crate::openai_responses::drain_sse_frames(&mut buffer) {
                match crate::openai_responses::ResponsesEvent::try_from_frame(&frame) {
                    crate::openai_responses::ParsedEvent::Valid(event) => {
                        for stream_event in mapper.process(event) {
                            if tx.send(Ok(stream_event)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                    crate::openai_responses::ParsedEvent::Malformed { data, error } => {
                        let err = ProviderError::StreamError(format!(
                            "malformed SSE data: {error} (data: {data:.80})"
                        ));
                        if tx.send(Err(err)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }

        if !mapper.saw_done {
            let err = ProviderError::StreamError("stream ended without a terminal event".into());
            let _ = tx.send(Err(err)).await;
        }

        Ok(())
    }
}

impl Provider for OpenAiCodexProvider {
    fn id(&self) -> &str {
        "openai-codex"
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn stream(&self, request: Request) -> EventStream {
        let access_token = self.access_token.clone();
        let account_id = self.account_id.clone();
        let base_url = self.base_url.clone();
        let body = self.build_request_body(&request);
        let cancel = request.cancel.clone();
        let http_client = self.client.client().clone();
        let session_id = self.session_id.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            if let Err(e) = Self::stream_http(
                http_client,
                access_token,
                account_id,
                session_id,
                base_url,
                &body,
                cancel,
                &tx,
            )
            .await
            {
                let _ = tx.send(Err(e)).await;
            }
        });

        Box::pin(ReceiverStream { rx })
    }
}

struct ReceiverStream {
    rx: tokio::sync::mpsc::Receiver<Result<AssistantStreamEvent, ProviderError>>,
}

impl futures_core::Stream for ReceiverStream {
    type Item = Result<AssistantStreamEvent, ProviderError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

static OS_RELEASE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    if cfg!(target_os = "windows") {
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(start) = stdout.find("[Version ") {
                let rest = &stdout[start + 9..];
                if let Some(end) = rest.find(']') {
                    return rest[..end].to_string();
                }
            }
        }
        "10.0.0".to_string()
    } else {
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                return stdout;
            }
        }
        if cfg!(target_os = "macos") {
            "25.5.0".to_string()
        } else {
            "6.1.0".to_string()
        }
    }
});

fn get_user_agent() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    let release = &*OS_RELEASE;
    format!("opi ({} {}; {})", os, release, arch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_request_body_shape() {
        let client = Arc::new(HttpClient::new());
        let provider = OpenAiCodexProvider::new(
            "test-token".to_string(),
            "test-account".to_string(),
            "https://chatgpt.com/backend-api".to_string(),
            client,
        );

        let req = Request {
            model: "openai-codex:gpt-5.5".to_string(),
            messages: vec![crate::message::Message::User(crate::message::UserMessage {
                content: vec![crate::message::InputContent::Text {
                    text: "hello".to_string(),
                }],
                timestamp_ms: 0,
            })],
            system: Some("system-prompt".to_string()),
            max_tokens: Some(100),
            temperature: Some(0.7),
            tools: vec![],
            thinking: crate::provider::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(2048),
            },
            stop_sequences: vec![],
            metadata: None,
            cancel: CancellationToken::new(),
        };

        let body = provider.build_request_body(&req);
        assert_eq!(body["model"], "gpt-5.5");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["instructions"], "system-prompt");
        assert_eq!(
            body["include"],
            serde_json::json!(["reasoning.encrypted_content"])
        );
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(body["parallel_tool_calls"], true);
        assert_eq!(body["reasoning"]["effort"], "medium");
        assert_eq!(body["reasoning"]["summary"], "auto");
    }
}
