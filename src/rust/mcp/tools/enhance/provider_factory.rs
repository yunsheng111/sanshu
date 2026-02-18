// 提供者工厂：从 McpConfig 构建 ChatClient，实现三级降级链
// L1: Ollama 本地 → L2: OpenAI 兼容 → L3: 规则引擎

use std::time::Duration;
use crate::config::settings::McpConfig;
use super::chat_client::{ChatClient, ChatProvider};

/// 同步版本：从配置构建 ChatClient（不检测 Ollama 可用性）
/// 适用于不需要异步的场景，按 L1→L2→L3 优先级选择
pub fn build_enhance_client(config: &McpConfig) -> ChatClient {
    // L1：Ollama 本地（同步版本不检测可用性，直接返回）
    if let Some(ollama_url) = config.enhance_ollama_url.as_ref() {
        if !ollama_url.is_empty() {
            let model = config
                .enhance_ollama_model
                .clone()
                .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
            return ChatClient::new(
                ChatProvider::Ollama,
                ollama_url.clone(),
                None,
                model,
            );
        }
    }

    // L2：OpenAI 兼容（SiliconFlow / Groq 等）
    if let (Some(base_url), Some(api_key)) = (
        config.enhance_base_url.as_ref(),
        config.enhance_api_key.as_ref(),
    ) {
        if !base_url.is_empty() && !api_key.is_empty() {
            let model = config
                .enhance_model
                .clone()
                .unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string());
            return ChatClient::new(
                ChatProvider::OpenAICompat,
                base_url.clone(),
                Some(api_key.clone()),
                model,
            );
        }
    }

    // L3：规则引擎（无需 API）
    ChatClient::new(
        ChatProvider::RuleEngine,
        String::new(),
        None,
        String::new(),
    )
}

/// 异步版本：从配置构建 ChatClient，L1 会检测 Ollama 可用性
pub async fn build_enhance_client_async(config: &McpConfig) -> ChatClient {
    // L1：Ollama 本地（异步检测可用性）
    if let Some(ollama_url) = config.enhance_ollama_url.as_ref() {
        if !ollama_url.is_empty() && is_ollama_available(ollama_url).await {
            let model = config
                .enhance_ollama_model
                .clone()
                .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
            return ChatClient::new(
                ChatProvider::Ollama,
                ollama_url.clone(),
                None,
                model,
            );
        }
    }

    // L2：OpenAI 兼容
    if let (Some(base_url), Some(api_key)) = (
        config.enhance_base_url.as_ref(),
        config.enhance_api_key.as_ref(),
    ) {
        if !base_url.is_empty() && !api_key.is_empty() {
            let model = config
                .enhance_model
                .clone()
                .unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string());
            return ChatClient::new(
                ChatProvider::OpenAICompat,
                base_url.clone(),
                Some(api_key.clone()),
                model,
            );
        }
    }

    // L3：规则引擎
    ChatClient::new(
        ChatProvider::RuleEngine,
        String::new(),
        None,
        String::new(),
    )
}

/// 异步检测 Ollama 是否可用（超时 3s）
async fn is_ollama_available(base_url: &str) -> bool {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(3_000))
        .timeout(Duration::from_millis(3_000))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}
