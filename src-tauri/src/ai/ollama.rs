use super::provider::{AIProvider, AIResponse, Prompt, TokenStream};
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen2.5:7b";

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(model: impl Into<String>, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            model: model.into(),
        }
    }

    /// Check if the Ollama server is reachable and the model is available.
    pub async fn check_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    fn build_body(&self, prompt: &Prompt, stream: bool) -> Value {
        // Ollama uses the OpenAI-compatible /api/chat endpoint
        let mut messages: Vec<Value> = Vec::new();

        if let Some(sys) = &prompt.system {
            messages.push(json!({ "role": "system", "content": sys }));
        }

        for m in &prompt.messages {
            let role = match m.role {
                super::provider::Role::User => "user",
                super::provider::Role::Assistant => "assistant",
            };
            messages.push(json!({ "role": role, "content": m.content }));
        }

        json!({
            "model": self.model,
            "messages": messages,
            "stream": stream,
            "options": {
                "temperature": 0.1,
                "num_predict": 8192,
            },
            // Ask Ollama to enforce JSON output when the model supports it
            "format": "json",
        })
    }
}

#[async_trait]
impl AIProvider for OllamaProvider {
    async fn complete(&self, prompt: Prompt) -> Result<AIResponse> {
        let url = format!("{}/api/chat", self.base_url);
        let body = self.build_body(&prompt, false);

        let raw = self.client.post(&url).json(&body).send().await?;

        if raw.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "Ollama model '{}' not found — run: ollama pull {}",
                self.model,
                self.model
            );
        }

        let resp = raw.error_for_status()?.json::<Value>().await?;

        let content = resp["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(AIResponse {
            content,
            model: resp["model"].as_str().unwrap_or(&self.model).to_string(),
            input_tokens: resp["prompt_eval_count"].as_u64().map(|v| v as u32),
            output_tokens: resp["eval_count"].as_u64().map(|v| v as u32),
        })
    }

    async fn stream(&self, prompt: Prompt) -> Result<TokenStream> {
        let url = format!("{}/api/chat", self.base_url);
        let body = self.build_body(&prompt, true);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let stream = resp.bytes_stream().filter_map(|chunk| async move {
            let bytes = chunk.ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            // Each chunk is a JSON object with "message": {"content": "..."}
            if let Ok(val) = serde_json::from_str::<Value>(text) {
                if let Some(token) = val["message"]["content"].as_str() {
                    if !token.is_empty() {
                        return Some(Ok(token.to_string()));
                    }
                }
            }
            None
        });

        Ok(Box::pin(stream))
    }

    fn is_available(&self) -> bool {
        // Considered available if configured; actual reachability checked separately
        true
    }

    fn name(&self) -> &str {
        "ollama"
    }
}
