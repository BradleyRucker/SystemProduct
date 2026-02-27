use super::provider::{AIProvider, AIResponse, Prompt, TokenStream};
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_key,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn build_body(&self, prompt: &Prompt, stream: bool) -> Value {
        let messages: Vec<Value> = prompt
            .messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "max_tokens": prompt.max_tokens.unwrap_or(1024),
            "messages": messages,
        });

        if let Some(sys) = &prompt.system {
            body["system"] = json!(sys);
        }

        if stream {
            body["stream"] = json!(true);
        }

        body
    }
}

#[async_trait]
impl AIProvider for AnthropicProvider {
    async fn complete(&self, prompt: Prompt) -> Result<AIResponse> {
        let body = self.build_body(&prompt, false);

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let content = resp["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(AIResponse {
            content,
            model: resp["model"].as_str().unwrap_or(&self.model).to_string(),
            input_tokens: resp["usage"]["input_tokens"].as_u64().map(|v| v as u32),
            output_tokens: resp["usage"]["output_tokens"].as_u64().map(|v| v as u32),
        })
    }

    async fn stream(&self, prompt: Prompt) -> Result<TokenStream> {
        let body = self.build_body(&prompt, true);

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let stream = resp.bytes_stream().filter_map(|chunk| async move {
            let bytes = chunk.ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;

            // SSE lines: "data: {...}"
            for line in text.lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        return None;
                    }
                    if let Ok(val) = serde_json::from_str::<Value>(data) {
                        if val["type"] == "content_block_delta" {
                            if let Some(token) = val["delta"]["text"].as_str() {
                                return Some(Ok(token.to_string()));
                            }
                        }
                    }
                }
            }
            None
        });

        Ok(Box::pin(stream))
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}
