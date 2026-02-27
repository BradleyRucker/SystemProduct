use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub model: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

pub type TokenStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

// ── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn complete(&self, prompt: Prompt) -> Result<AIResponse>;
    async fn stream(&self, prompt: Prompt) -> Result<TokenStream>;
    fn is_available(&self) -> bool;
    fn name(&self) -> &str;
}

// ── No-op provider (default when nothing is configured) ───────────────────────

pub struct NullProvider;

#[async_trait]
impl AIProvider for NullProvider {
    async fn complete(&self, _prompt: Prompt) -> Result<AIResponse> {
        anyhow::bail!("no AI provider configured")
    }

    async fn stream(&self, _prompt: Prompt) -> Result<TokenStream> {
        anyhow::bail!("no AI provider configured")
    }

    fn is_available(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "none"
    }
}
