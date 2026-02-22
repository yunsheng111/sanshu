// 提供者工厂：从 McpConfig 构建 ChatClient 候选列表，支持用户自定义渠道降级顺序
// 默认顺序: L1 Ollama → L2 云端 API → L3 规则引擎
// 用户可通过 enhance_channel_order 自定义，如 ["cloud","ollama","rule_engine"]

use std::time::Duration;
use crate::config::settings::McpConfig;
use super::chat_client::{ChatClient, ChatProvider};

/// 默认渠道顺序
const DEFAULT_CHANNEL_ORDER: &[&str] = &["ollama", "cloud", "rule_engine"];

/// 将 enhance_provider 字符串映射到 ChatProvider 枚举
/// gemini → Gemini, anthropic → Anthropic
/// openai/grok/deepseek/siliconflow/groq/cloudflare/ollama_compat 等 → OpenAICompat
/// ollama → OpenAICompat（兜底，正常情况 Ollama 走 L1 路径不经过此函数）
fn provider_from_str(s: &str) -> ChatProvider {
    match s.to_lowercase().as_str() {
        "gemini" => ChatProvider::Gemini,
        "anthropic" => ChatProvider::Anthropic,
        // 以下均兼容 OpenAI 格式，显式列出避免歧义
        "openai" | "grok" | "deepseek" | "siliconflow" | "groq" | "cloudflare"
        | "ollama" | "openai_compat" => ChatProvider::OpenAICompat,
        _ => ChatProvider::OpenAICompat,
    }
}

/// 解析渠道顺序，过滤无效值，保证 rule_engine 兜底
fn resolve_channel_order(config: &McpConfig) -> Vec<String> {
    let valid = ["ollama", "cloud", "rule_engine"];
    let order = match &config.enhance_channel_order {
        Some(list) if !list.is_empty() => {
            let mut filtered: Vec<String> = list.iter()
                .map(|s| s.to_lowercase())
                .filter(|s| valid.contains(&s.as_str()))
                .collect();
            // 去重
            filtered.dedup();
            // 保证 rule_engine 兜底
            if !filtered.contains(&"rule_engine".to_string()) {
                filtered.push("rule_engine".to_string());
            }
            filtered
        }
        _ => DEFAULT_CHANNEL_ORDER.iter().map(|s| s.to_string()).collect(),
    };
    order
}

/// 尝试构建 Ollama ChatClient（不检测可用性）
fn try_build_ollama(config: &McpConfig) -> Option<ChatClient> {
    let ollama_url = config.enhance_ollama_url.as_ref()?;
    if ollama_url.is_empty() {
        return None;
    }
    let model = config.enhance_ollama_model.clone()
        .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
    Some(ChatClient::new(ChatProvider::Ollama, ollama_url.clone(), None, model))
}

/// 尝试构建云端 API ChatClient
fn try_build_cloud(config: &McpConfig) -> Option<ChatClient> {
    let base_url = config.enhance_base_url.as_ref()?;
    let api_key = config.enhance_api_key.as_ref()?;
    if base_url.is_empty() || api_key.is_empty() {
        return None;
    }
    let provider = provider_from_str(
        config.enhance_provider.as_deref().unwrap_or("openai_compat"),
    );
    let model = config.enhance_model.clone()
        .unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string());
    Some(ChatClient::new(provider, base_url.clone(), Some(api_key.clone()), model))
}

/// 构建规则引擎 ChatClient（始终可用）
fn build_rule_engine() -> ChatClient {
    ChatClient::new(ChatProvider::RuleEngine, String::new(), None, String::new())
}

/// 按渠道顺序构建候选 ChatClient 列表（同步版本，不检测 Ollama 可用性）
pub fn build_enhance_candidates(config: &McpConfig) -> Vec<ChatClient> {
    let order = resolve_channel_order(config);
    let mut candidates = Vec::new();
    for channel in &order {
        match channel.as_str() {
            "ollama" => {
                if let Some(client) = try_build_ollama(config) {
                    candidates.push(client);
                }
            }
            "cloud" => {
                if let Some(client) = try_build_cloud(config) {
                    candidates.push(client);
                }
            }
            "rule_engine" => {
                candidates.push(build_rule_engine());
            }
            _ => {} // 无效值已在 resolve_channel_order 中过滤
        }
    }
    // 安全兜底：如果候选列表为空（理论上不会），加入规则引擎
    if candidates.is_empty() {
        candidates.push(build_rule_engine());
    }
    candidates
}

/// 按渠道顺序构建候选 ChatClient 列表（异步版本，检测 Ollama 可用性）
pub async fn build_enhance_candidates_async(config: &McpConfig) -> Vec<ChatClient> {
    let order = resolve_channel_order(config);
    let mut candidates = Vec::new();
    for channel in &order {
        match channel.as_str() {
            "ollama" => {
                if let Some(ollama_url) = config.enhance_ollama_url.as_ref() {
                    if !ollama_url.is_empty() && is_ollama_available(ollama_url).await {
                        let model = config.enhance_ollama_model.clone()
                            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());
                        candidates.push(ChatClient::new(
                            ChatProvider::Ollama, ollama_url.clone(), None, model,
                        ));
                    }
                }
            }
            "cloud" => {
                if let Some(client) = try_build_cloud(config) {
                    candidates.push(client);
                }
            }
            "rule_engine" => {
                candidates.push(build_rule_engine());
            }
            _ => {}
        }
    }
    if candidates.is_empty() {
        candidates.push(build_rule_engine());
    }
    candidates
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

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::McpConfig;

    /// 构建最小化 McpConfig（所有 enhance 字段为 None）
    fn make_empty_config() -> McpConfig {
        McpConfig {
            tools: std::collections::HashMap::new(),
            acemcp_base_url: None,
            acemcp_token: None,
            acemcp_batch_size: None,
            acemcp_max_lines_per_blob: None,
            acemcp_text_extensions: None,
            acemcp_exclude_patterns: None,
            acemcp_watch_debounce_ms: None,
            acemcp_auto_index_enabled: None,
            acemcp_index_nested_projects: None,
            acemcp_proxy_enabled: None,
            acemcp_proxy_host: None,
            acemcp_proxy_port: None,
            acemcp_proxy_type: None,
            acemcp_proxy_username: None,
            acemcp_proxy_password: None,
            context7_api_key: None,
            skill_python_path: None,
            skill_exec_timeout_secs: None,
            uiux_default_lang: None,
            uiux_output_format: None,
            uiux_max_results_cap: None,
            uiux_beautify_enabled: None,
            icon_default_save_path: None,
            icon_default_format: None,
            icon_default_png_size: None,
            icon_cache_expiry_minutes: None,
            enhance_provider: None,
            enhance_base_url: None,
            enhance_api_key: None,
            enhance_model: None,
            enhance_ollama_url: None,
            enhance_ollama_model: None,
            enhance_channel_order: None,
            sou_embedding_provider: None,
            sou_embedding_base_url: None,
            sou_embedding_api_key: None,
            sou_embedding_model: None,
            sou_mode: None,
            sou_index_path: None,
            // 协调层配置（Step 9 新增）
            retrieval_enable_coordination: None,
            retrieval_force_local: None,
            retrieval_local_weight: None,
            retrieval_remote_weight: None,
            retrieval_score_gap_threshold: None,
            retrieval_coverage_threshold: None,
        }
    }

    // ─── 正常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_enhance_candidates_l1_ollama_selected() {
        // Arrange - 设置 Ollama URL，应选择 L1
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://localhost:11434".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - 应选择 Ollama 提供者
        assert_eq!(client.provider, ChatProvider::Ollama);
        assert_eq!(client.base_url, "http://localhost:11434");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn test_build_enhance_candidates_l1_uses_custom_model() {
        // Arrange - 设置自定义 Ollama 模型
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://localhost:11434".to_string());
        config.enhance_ollama_model = Some("llama3:8b".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::Ollama);
        assert_eq!(client.model, "llama3:8b");
    }

    #[test]
    fn test_build_enhance_candidates_l1_default_model() {
        // Arrange - 未设置 Ollama 模型，应使用默认值
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://localhost:11434".to_string());
        config.enhance_ollama_model = None;

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - 默认模型为 qwen2.5-coder:7b
        assert_eq!(client.model, "qwen2.5-coder:7b");
    }

    #[test]
    fn test_build_enhance_candidates_l2_openai_compat_selected() {
        // Arrange - 无 Ollama URL，设置 OpenAI 兼容配置，应选择 L2
        let mut config = make_empty_config();
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("sk-test-key".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::OpenAICompat);
        assert_eq!(client.base_url, "https://api.siliconflow.cn/v1");
        assert_eq!(client.api_key, Some("sk-test-key".to_string()));
    }

    #[test]
    fn test_build_enhance_candidates_l2_uses_custom_model() {
        // Arrange
        let mut config = make_empty_config();
        config.enhance_base_url = Some("https://api.groq.com/v1".to_string());
        config.enhance_api_key = Some("gsk-test".to_string());
        config.enhance_model = Some("llama-3.1-70b-versatile".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.model, "llama-3.1-70b-versatile");
    }

    #[test]
    fn test_build_enhance_candidates_l2_default_model() {
        // Arrange - 未设置 model，应使用默认值
        let mut config = make_empty_config();
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("sk-test".to_string());
        config.enhance_model = None;

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - 默认模型
        assert_eq!(client.model, "Qwen/Qwen2.5-Coder-7B-Instruct");
    }

    #[test]
    fn test_build_enhance_candidates_l3_rule_engine_fallback() {
        // Arrange - 所有 API 配置均为 None，应降级到 L3
        let config = make_empty_config();

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::RuleEngine);
        assert_eq!(client.base_url, "");
        assert!(client.api_key.is_none());
        assert_eq!(client.model, "");
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_enhance_candidates_l1_empty_url_skips_to_l2() {
        // Arrange - Ollama URL 为空字符串，应跳过 L1 进入 L2
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("".to_string()); // 空字符串
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("sk-test".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - 空 URL 应跳过 L1，选择 L2
        assert_eq!(client.provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_build_enhance_candidates_l2_empty_api_key_skips_to_l3() {
        // Arrange - API key 为空字符串，应跳过 L2 进入 L3
        let mut config = make_empty_config();
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("".to_string()); // 空字符串

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - 空 API key 应跳过 L2，降级到 L3
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }

    #[test]
    fn test_build_enhance_candidates_l2_empty_base_url_skips_to_l3() {
        // Arrange - base_url 为空字符串，应跳过 L2 进入 L3
        let mut config = make_empty_config();
        config.enhance_base_url = Some("".to_string()); // 空字符串
        config.enhance_api_key = Some("sk-test".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }

    #[test]
    fn test_build_enhance_candidates_l1_priority_over_l2() {
        // Arrange - 同时设置 Ollama 和 OpenAI 兼容配置，L1 应优先
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://localhost:11434".to_string());
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("sk-test".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert - L1 优先于 L2
        assert_eq!(client.provider, ChatProvider::Ollama);
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_enhance_candidates_l2_missing_api_key_falls_to_l3() {
        // Arrange - 有 base_url 但无 api_key（None），应降级到 L3
        let mut config = make_empty_config();
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = None; // 完全缺失

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }

    #[test]
    fn test_build_enhance_candidates_l2_missing_base_url_falls_to_l3() {
        // Arrange - 有 api_key 但无 base_url（None），应降级到 L3
        let mut config = make_empty_config();
        config.enhance_base_url = None; // 完全缺失
        config.enhance_api_key = Some("sk-test".to_string());

        // Act
        let candidates = build_enhance_candidates(&config);
        let client = candidates.first().unwrap();

        // Assert
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }

    #[tokio::test]
    async fn test_build_enhance_candidates_async_l2_when_ollama_unreachable() {
        // Arrange - Ollama URL 指向不可达地址，异步版本应跳过 L1 进入 L2
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://127.0.0.1:1".to_string()); // 不可达
        config.enhance_base_url = Some("https://api.siliconflow.cn/v1".to_string());
        config.enhance_api_key = Some("sk-test".to_string());

        // Act - 异步版本会检测 Ollama 可用性，超时后跳过
        let candidates = build_enhance_candidates_async(&config).await;
        let client = candidates.first().unwrap();

        // Assert - Ollama 不可达，应选择 L2
        assert_eq!(client.provider, ChatProvider::OpenAICompat);
    }

    #[tokio::test]
    async fn test_build_enhance_candidates_async_l3_when_all_unavailable() {
        // Arrange - Ollama 不可达，且无 OpenAI 配置
        let mut config = make_empty_config();
        config.enhance_ollama_url = Some("http://127.0.0.1:1".to_string()); // 不可达

        // Act
        let candidates = build_enhance_candidates_async(&config).await;
        let client = candidates.first().unwrap();

        // Assert - 最终降级到 L3
        assert_eq!(client.provider, ChatProvider::RuleEngine);
    }
}

// ─── provider_from_str 专项测试 ───────────────────────────────────────────────

#[cfg(test)]
mod provider_from_str_tests {
    use super::*;

    // ─── 正常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_from_str_openai() {
        // Arrange & Act
        let provider = provider_from_str("openai");

        // Assert
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_grok() {
        let provider = provider_from_str("grok");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_deepseek() {
        let provider = provider_from_str("deepseek");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_siliconflow() {
        let provider = provider_from_str("siliconflow");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_groq() {
        let provider = provider_from_str("groq");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_cloudflare() {
        let provider = provider_from_str("cloudflare");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_ollama() {
        let provider = provider_from_str("ollama");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_openai_compat() {
        let provider = provider_from_str("openai_compat");
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_gemini() {
        // Arrange & Act
        let provider = provider_from_str("gemini");

        // Assert - Gemini 应映射到 Gemini 枚举
        assert_eq!(provider, ChatProvider::Gemini);
    }

    #[test]
    fn test_provider_from_str_anthropic() {
        // Arrange & Act
        let provider = provider_from_str("anthropic");

        // Assert - Anthropic 应映射到 Anthropic 枚举
        assert_eq!(provider, ChatProvider::Anthropic);
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_from_str_case_insensitive_uppercase() {
        // Arrange & Act - 测试大写
        let provider = provider_from_str("GROK");

        // Assert - 应正确映射（大小写不敏感）
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_case_insensitive_mixed() {
        // Arrange & Act - 测试混合大小写
        let provider = provider_from_str("GeMiNi");

        // Assert
        assert_eq!(provider, ChatProvider::Gemini);
    }

    #[test]
    fn test_provider_from_str_case_insensitive_anthropic() {
        // Arrange & Act
        let provider = provider_from_str("ANTHROPIC");

        // Assert
        assert_eq!(provider, ChatProvider::Anthropic);
    }

    #[test]
    fn test_provider_from_str_empty_string() {
        // Arrange & Act - 空字符串应 fallback 到 OpenAICompat
        let provider = provider_from_str("");

        // Assert
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_unknown_provider() {
        // Arrange & Act - 未知供应商应 fallback 到 OpenAICompat
        let provider = provider_from_str("unknown_provider_xyz");

        // Assert
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_from_str_with_whitespace() {
        // Arrange & Act - 带前后空格的字符串
        let provider = provider_from_str("  openai  ");

        // Assert - 当前实现不会 trim，应 fallback
        // 注意：如果未来实现添加了 trim，此测试需要更新
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_special_characters() {
        // Arrange & Act - 特殊字符
        let provider = provider_from_str("open@ai!");

        // Assert - 应 fallback 到 OpenAICompat
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_unicode_characters() {
        // Arrange & Act - Unicode 字符
        let provider = provider_from_str("供应商");

        // Assert - 应 fallback 到 OpenAICompat
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_very_long_string() {
        // Arrange - 极长字符串（1000 字符）
        let long_string = "a".repeat(1000);

        // Act
        let provider = provider_from_str(&long_string);

        // Assert - 应 fallback 到 OpenAICompat
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    #[test]
    fn test_provider_from_str_numeric_string() {
        // Arrange & Act - 纯数字字符串
        let provider = provider_from_str("12345");

        // Assert - 应 fallback 到 OpenAICompat
        assert_eq!(provider, ChatProvider::OpenAICompat);
    }

    // ─── 完整性测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_all_explicit_mappings_are_openai_compat() {
        // Arrange - 所有显式列出的 OpenAI 兼容供应商
        let openai_compat_providers = vec![
            "openai",
            "grok",
            "deepseek",
            "siliconflow",
            "groq",
            "cloudflare",
            "ollama",
            "openai_compat",
        ];

        // Act & Assert - 验证所有供应商都映射到 OpenAICompat
        for provider_str in openai_compat_providers {
            let provider = provider_from_str(provider_str);
            assert_eq!(
                provider,
                ChatProvider::OpenAICompat,
                "Provider '{}' should map to OpenAICompat",
                provider_str
            );
        }
    }

    #[test]
    fn test_non_openai_compat_providers() {
        // Arrange - 非 OpenAI 兼容的供应商
        let non_compat_providers = vec![
            ("gemini", ChatProvider::Gemini),
            ("anthropic", ChatProvider::Anthropic),
        ];

        // Act & Assert
        for (provider_str, expected) in non_compat_providers {
            let provider = provider_from_str(provider_str);
            assert_eq!(
                provider, expected,
                "Provider '{}' should map to {:?}",
                provider_str, expected
            );
        }
    }
}
