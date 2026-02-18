// 统一 Chat 客户端
// 支持 Ollama / OpenAI 兼容 / 规则引擎三级降级链

use std::time::Duration;
use anyhow::Result;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 支持的 Chat 提供者
#[derive(Debug, Clone, PartialEq)]
pub enum ChatProvider {
    Ollama,
    OpenAICompat,  // SiliconFlow / Groq / Gemini / Cloudflare 等
    RuleEngine,    // 无 API 时的纯规则降级
}

/// 统一消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

/// 统一 Chat 客户端（三段超时 + 懒构建 reqwest::Client）
pub struct ChatClient {
    pub provider: ChatProvider,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub stream_timeout_ms: u64,
}

impl ChatClient {
    pub fn new(
        provider: ChatProvider,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        let (connect_ms, request_ms, stream_ms) = match provider {
            ChatProvider::Ollama => (3_000, 60_000, 300_000),
            ChatProvider::OpenAICompat => (10_000, 30_000, 120_000),
            ChatProvider::RuleEngine => (0, 0, 0),
        };
        Self {
            provider,
            base_url,
            api_key,
            model,
            connect_timeout_ms: connect_ms,
            request_timeout_ms: request_ms,
            stream_timeout_ms: stream_ms,
        }
    }

    fn build_client(&self, is_stream: bool) -> Result<reqwest::Client> {
        let timeout_ms = if is_stream {
            self.stream_timeout_ms
        } else {
            self.request_timeout_ms
        };
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(self.connect_timeout_ms))
            .timeout(Duration::from_millis(timeout_ms))
            .build()?;
        Ok(client)
    }

    /// 按 provider 类型自动选择请求格式
    pub async fn chat(&self, messages: &[Message]) -> Result<String> {
        match self.provider {
            ChatProvider::RuleEngine => {
                // 规则引擎不走网络，直接返回空（由 RuleEnhancer 处理）
                Ok(String::new())
            }
            ChatProvider::Ollama => self.chat_ollama(messages).await,
            ChatProvider::OpenAICompat => self.chat_openai_compat(messages).await,
        }
    }

    async fn chat_ollama(&self, messages: &[Message]) -> Result<String> {
        let client = self.build_client(false)?;
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": false
        });
        let mut req = client.post(&url).header(CONTENT_TYPE, "application/json");
        if let Some(key) = &self.api_key {
            req = req.header(AUTHORIZATION, format!("Bearer {}", key));
        }
        let resp = req.json(&payload).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Ollama HTTP {} - {}", status, body));
        }
        let json: serde_json::Value = resp.json().await?;
        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(content)
    }

    async fn chat_openai_compat(&self, messages: &[Message]) -> Result<String> {
        let client = self.build_client(false)?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": false
        });
        let mut req = client.post(&url).header(CONTENT_TYPE, "application/json");
        if let Some(key) = &self.api_key {
            req = req.header(AUTHORIZATION, format!("Bearer {}", key));
        }
        let resp = req.json(&payload).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenAI compat HTTP {} - {}", status, body));
        }
        let json: serde_json::Value = resp.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(content)
    }

    /// 带简单重试的 chat（最多 2 次）
    pub async fn chat_with_retry(&self, messages: &[Message]) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..2 {
            match self.chat(messages).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == 0 {
                        log::debug!("chat 第 1 次失败，重试: {}", e);
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("chat_with_retry 未知错误")))
    }

    /// 检测服务是否可用（超时 3s）
    pub async fn is_available(&self) -> bool {
        if self.provider == ChatProvider::RuleEngine {
            return true;
        }
        let client = match reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(3))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        let health_url = match self.provider {
            ChatProvider::Ollama => format!("{}/api/tags", self.base_url.trim_end_matches('/')),
            _ => format!("{}/models", self.base_url.trim_end_matches('/')),
        };
        client.get(&health_url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
