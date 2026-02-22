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
    OpenAICompat,  // OpenAI / Grok(xAI) / DeepSeek / SiliconFlow / Groq / Cloudflare 等
    Gemini,        // Google Gemini 原生格式
    Anthropic,     // Anthropic Claude 原生格式
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
            ChatProvider::Ollama => (5_000, 90_000, 300_000),  // 连接超时 3s -> 5s，请求超时 60s -> 90s
            ChatProvider::OpenAICompat | ChatProvider::Gemini | ChatProvider::Anthropic => (10_000, 45_000, 120_000),  // 请求超时 30s -> 45s
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
            ChatProvider::Gemini => self.chat_gemini(messages).await,
            ChatProvider::Anthropic => self.chat_anthropic(messages).await,
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

    async fn chat_gemini(&self, messages: &[Message]) -> Result<String> {
        let client = self.build_client(false)?;
        let base = self.base_url.trim_end_matches('/');
        let url = format!("{}/models/{}:generateContent", base, self.model);

        // 将 system 消息合并为第一条 user 消息的前缀
        let mut system_text = String::new();
        let mut user_messages: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            if msg.role == "system" {
                system_text.push_str(&msg.content);
                system_text.push('\n');
            } else {
                let role = if msg.role == "assistant" { "model" } else { "user" };
                user_messages.push(json!({
                    "role": role,
                    "parts": [{"text": msg.content}]
                }));
            }
        }
        // 将 system 前缀注入第一条 user 消息
        if !system_text.is_empty() {
            if let Some(first) = user_messages.first_mut() {
                let orig = first["parts"][0]["text"].as_str().unwrap_or("").to_string();
                first["parts"][0]["text"] = json!(format!("{}{}", system_text, orig));
            } else {
                user_messages.push(json!({
                    "role": "user",
                    "parts": [{"text": system_text.trim()}]
                }));
            }
        }

        let payload = json!({ "contents": user_messages });
        let mut req = client.post(&url).header(CONTENT_TYPE, "application/json");
        if let Some(key) = &self.api_key {
            req = req.header("x-goog-api-key", key.as_str());
        }
        let resp = req.json(&payload).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Gemini HTTP {} - {}", status, body));
        }
        let json: serde_json::Value = resp.json().await?;
        let content = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(content)
    }

    async fn chat_anthropic(&self, messages: &[Message]) -> Result<String> {
        let client = self.build_client(false)?;
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));

        // 提取 system 消息
        let system_text: String = messages.iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let non_system: Vec<serde_json::Value> = messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let mut payload = json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": non_system
        });
        if !system_text.is_empty() {
            payload["system"] = json!(system_text);
        }

        let mut req = client.post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header("anthropic-version", "2023-06-01");
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key.as_str());
        }
        let resp = req.json(&payload).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Anthropic HTTP {} - {}", status, body));
        }
        let json: serde_json::Value = resp.json().await?;
        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(content)
    }

    /// 带指数退避重试的 chat（最多 2 次，第 2 次等待 1s）
    pub async fn chat_with_retry(&self, messages: &[Message]) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..2 {
            match self.chat(messages).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == 0 {
                        log::warn!("chat 第 1 次失败，1 秒后重试: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
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
            ChatProvider::Gemini | ChatProvider::Anthropic => {
                // Gemini/Anthropic 不提供简单健康检查端点，直接视为可用
                return true;
            }
            _ => format!("{}/models", self.base_url.trim_end_matches('/')),
        };
        client.get(&health_url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 正常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_message_constructors() {
        // Arrange & Act
        let sys = Message::system("你是一个助手");
        let usr = Message::user("帮我写代码");
        let ast = Message::assistant("好的");

        // Assert
        assert_eq!(sys.role, "system");
        assert_eq!(sys.content, "你是一个助手");
        assert_eq!(usr.role, "user");
        assert_eq!(usr.content, "帮我写代码");
        assert_eq!(ast.role, "assistant");
        assert_eq!(ast.content, "好的");
    }

    #[test]
    fn test_chat_client_ollama_timeouts() {
        // Arrange & Act
        let client = ChatClient::new(
            ChatProvider::Ollama,
            "http://localhost:11434".to_string(),
            None,
            "qwen2.5-coder:7b".to_string(),
        );

        // Assert - Ollama 超时配置：connect=5s, request=90s, stream=300s
        assert_eq!(client.connect_timeout_ms, 5_000);
        assert_eq!(client.request_timeout_ms, 90_000);
        assert_eq!(client.stream_timeout_ms, 300_000);
        assert_eq!(client.provider, ChatProvider::Ollama);
    }

    #[test]
    fn test_chat_client_openai_compat_timeouts() {
        // Arrange & Act
        let client = ChatClient::new(
            ChatProvider::OpenAICompat,
            "https://api.siliconflow.cn/v1".to_string(),
            Some("sk-test".to_string()),
            "Qwen/Qwen2.5-Coder-7B-Instruct".to_string(),
        );

        // Assert - OpenAICompat 超时配置：connect=10s, request=45s, stream=120s
        assert_eq!(client.connect_timeout_ms, 10_000);
        assert_eq!(client.request_timeout_ms, 45_000);
        assert_eq!(client.stream_timeout_ms, 120_000);
        assert_eq!(client.provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_chat_client_rule_engine_zero_timeouts() {
        // Arrange & Act
        let client = ChatClient::new(
            ChatProvider::RuleEngine,
            String::new(),
            None,
            String::new(),
        );

        // Assert - RuleEngine 超时全为 0（不走网络）
        assert_eq!(client.connect_timeout_ms, 0);
        assert_eq!(client.request_timeout_ms, 0);
        assert_eq!(client.stream_timeout_ms, 0);
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }

    #[tokio::test]
    async fn test_rule_engine_chat_returns_empty_without_network() {
        // Arrange - RuleEngine 不走网络，直接返回空字符串
        let client = ChatClient::new(
            ChatProvider::RuleEngine,
            String::new(),
            None,
            String::new(),
        );
        let messages = vec![
            Message::system("你是助手"),
            Message::user("帮我优化代码"),
        ];

        // Act
        let result = client.chat(&messages).await;

        // Assert - 应成功返回空字符串，不发网络请求
        assert!(result.is_ok(), "RuleEngine chat 应返回 Ok");
        assert_eq!(result.unwrap(), "", "RuleEngine chat 应返回空字符串");
    }

    #[tokio::test]
    async fn test_rule_engine_chat_with_retry_returns_empty() {
        // Arrange
        let client = ChatClient::new(
            ChatProvider::RuleEngine,
            String::new(),
            None,
            String::new(),
        );
        let messages = vec![Message::user("测试重试")];

        // Act
        let result = client.chat_with_retry(&messages).await;

        // Assert - RuleEngine 不会失败，重试逻辑应直接返回成功
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_rule_engine_is_available_always_true() {
        // Arrange
        let client = ChatClient::new(
            ChatProvider::RuleEngine,
            String::new(),
            None,
            String::new(),
        );

        // Act
        let available = client.is_available().await;

        // Assert - RuleEngine 始终可用，不走网络
        assert!(available, "RuleEngine is_available 应始终返回 true");
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_chat_client_with_empty_strings() {
        // Arrange & Act - 空字符串参数
        let client = ChatClient::new(
            ChatProvider::Ollama,
            "".to_string(),
            None,
            "".to_string(),
        );

        // Assert - 空字符串应被接受，不 panic
        assert_eq!(client.base_url, "");
        assert_eq!(client.model, "");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn test_chat_client_with_api_key() {
        // Arrange & Act
        let client = ChatClient::new(
            ChatProvider::OpenAICompat,
            "https://api.example.com/v1".to_string(),
            Some("sk-secret-key".to_string()),
            "gpt-4".to_string(),
        );

        // Assert
        assert_eq!(client.api_key, Some("sk-secret-key".to_string()));
    }

    #[tokio::test]
    async fn test_rule_engine_chat_empty_messages() {
        // Arrange - 空消息列表
        let client = ChatClient::new(
            ChatProvider::RuleEngine,
            String::new(),
            None,
            String::new(),
        );
        let messages: Vec<Message> = vec![];

        // Act
        let result = client.chat(&messages).await;

        // Assert - 空消息列表应正常处理，不 panic
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_equality() {
        // Arrange & Act & Assert
        assert_eq!(ChatProvider::Ollama, ChatProvider::Ollama);
        assert_eq!(ChatProvider::RuleEngine, ChatProvider::RuleEngine);
        assert_ne!(ChatProvider::Ollama, ChatProvider::RuleEngine);
        assert_ne!(ChatProvider::OpenAICompat, ChatProvider::RuleEngine);
    }

    #[test]
    fn test_provider_clone() {
        // Arrange
        let provider = ChatProvider::OpenAICompat;

        // Act
        let cloned = provider.clone();

        // Assert
        assert_eq!(provider, cloned);
    }

    #[test]
    fn test_message_clone() {
        // Arrange
        let msg = Message::user("原始消息");

        // Act
        let cloned = msg.clone();

        // Assert
        assert_eq!(cloned.role, msg.role);
        assert_eq!(cloned.content, msg.content);
    }

    #[tokio::test]
    async fn test_ollama_is_available_returns_false_on_unreachable_host() {
        // Arrange - 使用不可达的地址（端口 1 通常不监听）
        let client = ChatClient::new(
            ChatProvider::Ollama,
            "http://127.0.0.1:1".to_string(),
            None,
            "test-model".to_string(),
        );

        // Act - 连接超时应返回 false，不 panic
        let available = client.is_available().await;

        // Assert
        assert!(!available, "不可达主机应返回 false");
    }
}
