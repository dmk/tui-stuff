mod ollama;
mod openai;
pub mod prompt;
pub mod schema;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use ollama::OllamaClient;
pub use openai::OpenAiClient;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Openai,
    Ollama,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LlmRequest {
    pub id: u64,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_stream")]
    pub stream: bool,
}

fn default_stream() -> bool {
    false
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("missing OpenAI API key")]
    MissingApiKey,
    #[error("request failed: {0}")]
    Request(String),
    #[error("response parse error: {0}")]
    Parse(String),
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn stream_chat(
        &self,
        request: &LlmRequest,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, LlmError>;
}

pub fn client_for(
    provider: Provider,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<Box<dyn LlmClient>, LlmError> {
    match provider {
        Provider::Openai => {
            let key = api_key.ok_or(LlmError::MissingApiKey)?;
            Ok(Box::new(OpenAiClient::new(key, model)))
        }
        Provider::Ollama => {
            let base = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
            Ok(Box::new(OllamaClient::new(base, model)))
        }
    }
}
