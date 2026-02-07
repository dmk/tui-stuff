use futures_util::StreamExt;
use serde_json::Value;

use crate::llm::{ChatMessage, LlmClient, LlmError, LlmRequest};

pub struct OllamaClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
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
impl LlmClient for OllamaClient {
    async fn stream_chat(
        &self,
        request: &LlmRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": Self::messages_payload(&request.messages),
            "stream": request.stream,
            "format": "json",
        });

        let response = self
            .client
            .post(url)
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
                .get("message")
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
                if line.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    let done = value
                        .get("done")
                        .and_then(|d| d.as_bool())
                        .unwrap_or(false);
                    if let Some(content) = value
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !content.is_empty() {
                            on_chunk(content.to_string());
                            full.push_str(content);
                        }
                    }
                    if done {
                        return Ok(full);
                    }
                }
            }
        }

        Ok(full)
    }
}
