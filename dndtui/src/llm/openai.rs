use futures_util::StreamExt;
use serde_json::Value;

use crate::llm::{ChatMessage, LlmClient, LlmError, LlmRequest};

pub struct OpenAiClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    fn messages_payload(messages: &[ChatMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                })
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    async fn stream_chat(
        &self,
        request: &LlmRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, LlmError> {
        let url = "https://api.openai.com/v1/chat/completions";
        let body = serde_json::json!({
            "model": self.model,
            "messages": Self::messages_payload(&request.messages),
            "temperature": 0.7,
            "stream": request.stream,
            "response_format": {"type": "json_object"},
        });

        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?
            .error_for_status()
            .map_err(|e| LlmError::Request(e.to_string()))?;

        if !request.stream {
            let value: Value = response
                .json()
                .await
                .map_err(|e| LlmError::Parse(e.to_string()))?;
            let content = value
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .ok_or_else(|| LlmError::Parse("missing content".to_string()))?;
            return Ok(content.to_string());
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut full = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| LlmError::Request(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();
                if !line.starts_with("data:") {
                    continue;
                }
                let data = line.trim_start_matches("data:").trim();
                if data == "[DONE]" {
                    return Ok(full);
                }
                if data.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    if let Some(delta) = value
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !delta.is_empty() {
                            on_chunk(delta.to_string());
                            full.push_str(delta);
                        }
                    }
                }
            }
        }

        Ok(full)
    }
}
