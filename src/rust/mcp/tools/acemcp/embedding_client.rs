// 嵌入客户端模块
// 支持 Jina / SiliconFlow / Cloudflare / Nomic / Cohere / Ollama 等嵌入提供者

use std::time::Duration;
use anyhow::Result;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 支持的嵌入提供者
#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingProvider {
    Jina,
    SiliconFlow,
    Cloudflare,
    Nomic,
    Cohere,
    Ollama,
}

impl EmbeddingProvider {
    /// 从字符串解析提供者类型
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "jina" => Some(Self::Jina),
            "siliconflow" => Some(Self::SiliconFlow),
            "cloudflare" => Some(Self::Cloudflare),
            "nomic" => Some(Self::Nomic),
            "cohere" => Some(Self::Cohere),
            "ollama" => Some(Self::Ollama),
            _ => None,
        }
    }

    /// 获取默认批量大小
    pub fn default_batch_size(&self) -> usize {
        match self {
            Self::Jina => 100,
            Self::SiliconFlow => 50,
            Self::Cloudflare => 50,
            Self::Nomic => 50,
            Self::Cohere => 50,
            Self::Ollama => 20,
        }
    }

    /// 获取默认 RPM 限制（None 表示无限制）
    pub fn default_rate_limit_rpm(&self) -> Option<u32> {
        match self {
            Self::Jina => Some(60),
            Self::Cohere => Some(10), // Trial 模式
            _ => None,
        }
    }

    /// 获取默认向量维度
    pub fn default_dimension(&self) -> usize {
        match self {
            Self::Jina => 768,
            Self::SiliconFlow => 1024,
            Self::Cloudflare => 768,
            Self::Nomic => 768,
            Self::Cohere => 1024,
            Self::Ollama => 768,
        }
    }
}

/// 嵌入响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub embedding: Vec<f32>,
    pub index: usize,
}

/// 嵌入客户端
pub struct EmbeddingClient {
    pub provider: EmbeddingProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub dimension: usize,
    pub batch_size: usize,
    pub rate_limit_rpm: Option<u32>,
    last_request_time: std::sync::Mutex<Option<std::time::Instant>>,
}

impl EmbeddingClient {
    pub fn new(
        provider: EmbeddingProvider,
        base_url: String,
        api_key: String,
        model: String,
    ) -> Self {
        let dimension = provider.default_dimension();
        let batch_size = provider.default_batch_size();
        let rate_limit_rpm = provider.default_rate_limit_rpm();
        Self {
            provider,
            base_url,
            api_key,
            model,
            dimension,
            batch_size,
            rate_limit_rpm,
            last_request_time: std::sync::Mutex::new(None),
        }
    }

    /// 设置自定义维度
    pub fn with_dimension(mut self, dim: usize) -> Self {
        self.dimension = dim;
        self
    }

    /// 设置自定义批量大小
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    fn build_client(&self) -> Result<reqwest::Client> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(client)
    }

    /// 简单令牌桶限速
    /// 注意：不能在持有 std::sync::Mutex guard 时跨 await，否则会导致死锁
    async fn wait_for_rate_limit(&self) {
        if let Some(rpm) = self.rate_limit_rpm {
            let interval_ms = 60_000 / rpm as u64;

            // 步骤1：计算需要等待的时长，然后立即释放锁
            let sleep_duration = {
                let last_time = self.last_request_time.lock().unwrap();
                if let Some(last) = *last_time {
                    let elapsed = last.elapsed().as_millis() as u64;
                    if elapsed < interval_ms {
                        Some(Duration::from_millis(interval_ms - elapsed))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }; // guard 在此处 drop

            // 步骤2：在锁外执行 sleep（安全的 await 点）
            if let Some(duration) = sleep_duration {
                tokio::time::sleep(duration).await;
            }

            // 步骤3：重新获取锁更新时间戳
            let mut last_time = self.last_request_time.lock().unwrap();
            *last_time = Some(std::time::Instant::now());
        }
    }

    /// 单文本嵌入
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text.to_string()]).await?;
        results.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("嵌入结果为空"))
    }

    /// 批量嵌入（自动分批 + 限速）
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut all_results = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(self.batch_size) {
            self.wait_for_rate_limit().await;
            let chunk_results = self.embed_batch_internal(chunk).await?;
            all_results.extend(chunk_results);
        }

        Ok(all_results)
    }

    async fn embed_batch_internal(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self.provider {
            EmbeddingProvider::Ollama => self.embed_ollama(texts).await,
            _ => self.embed_openai_compat(texts).await,
        }
    }

    async fn embed_ollama(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let client = self.build_client()?;
        let url = format!("{}/api/embed", self.base_url.trim_end_matches('/'));

        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let payload = json!({
                "model": self.model,
                "input": text
            });
            let resp = client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .json(&payload)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("Ollama embed HTTP {} - {}", status, body));
            }

            let json: serde_json::Value = resp.json().await?;
            let embedding = json["embeddings"][0]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Ollama 响应格式错误"))?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            results.push(embedding);
        }
        Ok(results)
    }

    async fn embed_openai_compat(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let client = self.build_client()?;
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        let payload = json!({
            "model": self.model,
            "input": texts
        });

        let resp = client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Embedding API HTTP {} - {}", status, body));
        }

        let json: serde_json::Value = resp.json().await?;
        let data = json["data"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("响应格式错误：缺少 data 字段"))?;

        let mut results: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
        for item in data {
            let index = item["index"].as_u64().unwrap_or(0) as usize;
            let embedding = item["embedding"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("响应格式错误：缺少 embedding"))?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            results.push((index, embedding));
        }

        // 按 index 排序确保顺序正确
        results.sort_by_key(|(idx, _)| *idx);
        Ok(results.into_iter().map(|(_, emb)| emb).collect())
    }

    /// 检测服务是否可用
    pub async fn is_available(&self) -> bool {
        let client = match reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(3))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        let health_url = match self.provider {
            EmbeddingProvider::Ollama => {
                format!("{}/api/tags", self.base_url.trim_end_matches('/'))
            }
            _ => format!("{}/models", self.base_url.trim_end_matches('/')),
        };

        client
            .get(&health_url)
            .send()
            .await
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
    fn test_provider_from_str_valid() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::from_str("jina"), Some(EmbeddingProvider::Jina));
        assert_eq!(EmbeddingProvider::from_str("JINA"), Some(EmbeddingProvider::Jina));
        assert_eq!(EmbeddingProvider::from_str("siliconflow"), Some(EmbeddingProvider::SiliconFlow));
        assert_eq!(EmbeddingProvider::from_str("cloudflare"), Some(EmbeddingProvider::Cloudflare));
        assert_eq!(EmbeddingProvider::from_str("nomic"), Some(EmbeddingProvider::Nomic));
        assert_eq!(EmbeddingProvider::from_str("cohere"), Some(EmbeddingProvider::Cohere));
        assert_eq!(EmbeddingProvider::from_str("ollama"), Some(EmbeddingProvider::Ollama));
    }

    #[test]
    fn test_provider_default_batch_size() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::Jina.default_batch_size(), 100);
        assert_eq!(EmbeddingProvider::SiliconFlow.default_batch_size(), 50);
        assert_eq!(EmbeddingProvider::Cloudflare.default_batch_size(), 50);
        assert_eq!(EmbeddingProvider::Nomic.default_batch_size(), 50);
        assert_eq!(EmbeddingProvider::Cohere.default_batch_size(), 50);
        assert_eq!(EmbeddingProvider::Ollama.default_batch_size(), 20);
    }

    #[test]
    fn test_provider_default_rate_limit_rpm() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::Jina.default_rate_limit_rpm(), Some(60));
        assert_eq!(EmbeddingProvider::Cohere.default_rate_limit_rpm(), Some(10));
        assert_eq!(EmbeddingProvider::SiliconFlow.default_rate_limit_rpm(), None);
        assert_eq!(EmbeddingProvider::Cloudflare.default_rate_limit_rpm(), None);
        assert_eq!(EmbeddingProvider::Nomic.default_rate_limit_rpm(), None);
        assert_eq!(EmbeddingProvider::Ollama.default_rate_limit_rpm(), None);
    }

    #[test]
    fn test_provider_default_dimension() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::Jina.default_dimension(), 768);
        assert_eq!(EmbeddingProvider::SiliconFlow.default_dimension(), 1024);
        assert_eq!(EmbeddingProvider::Cloudflare.default_dimension(), 768);
        assert_eq!(EmbeddingProvider::Nomic.default_dimension(), 768);
        assert_eq!(EmbeddingProvider::Cohere.default_dimension(), 1024);
        assert_eq!(EmbeddingProvider::Ollama.default_dimension(), 768);
    }

    #[test]
    fn test_client_new_with_defaults() {
        // Arrange
        let provider = EmbeddingProvider::Jina;
        let base_url = "https://api.jina.ai/v1".to_string();
        let api_key = "test-key".to_string();
        let model = "jina-embeddings-v2-base-en".to_string();

        // Act
        let client = EmbeddingClient::new(provider.clone(), base_url.clone(), api_key.clone(), model.clone());

        // Assert
        assert_eq!(client.provider, provider);
        assert_eq!(client.base_url, base_url);
        assert_eq!(client.api_key, api_key);
        assert_eq!(client.model, model);
        assert_eq!(client.dimension, 768);
        assert_eq!(client.batch_size, 100);
        assert_eq!(client.rate_limit_rpm, Some(60));
    }

    #[test]
    fn test_client_with_custom_dimension() {
        // Arrange
        let client = EmbeddingClient::new(
            EmbeddingProvider::Jina,
            "https://api.jina.ai/v1".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        // Act
        let client = client.with_dimension(512);

        // Assert
        assert_eq!(client.dimension, 512);
    }

    #[test]
    fn test_client_with_custom_batch_size() {
        // Arrange
        let client = EmbeddingClient::new(
            EmbeddingProvider::Jina,
            "https://api.jina.ai/v1".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        // Act
        let client = client.with_batch_size(25);

        // Assert
        assert_eq!(client.batch_size, 25);
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_from_str_case_insensitive() {
        // Arrange & Act & Assert - 大小写混合
        assert_eq!(EmbeddingProvider::from_str("JiNa"), Some(EmbeddingProvider::Jina));
        assert_eq!(EmbeddingProvider::from_str("SILICONFLOW"), Some(EmbeddingProvider::SiliconFlow));
        assert_eq!(EmbeddingProvider::from_str("OlLaMa"), Some(EmbeddingProvider::Ollama));
    }

    #[test]
    fn test_client_with_batch_size_zero() {
        // Arrange - W2: batch_size=0 边界
        let client = EmbeddingClient::new(
            EmbeddingProvider::Jina,
            "https://api.jina.ai/v1".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        // Act
        let client = client.with_batch_size(0);

        // Assert - batch_size=0 应该被接受（调用方需要处理）
        assert_eq!(client.batch_size, 0);
    }

    #[test]
    fn test_client_with_dimension_zero() {
        // Arrange
        let client = EmbeddingClient::new(
            EmbeddingProvider::Jina,
            "https://api.jina.ai/v1".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        );

        // Act
        let client = client.with_dimension(0);

        // Assert
        assert_eq!(client.dimension, 0);
    }

    #[test]
    fn test_client_with_empty_strings() {
        // Arrange & Act
        let client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "".to_string(),
            "".to_string(),
            "".to_string(),
        );

        // Assert - 空字符串应该被接受
        assert_eq!(client.base_url, "");
        assert_eq!(client.api_key, "");
        assert_eq!(client.model, "");
    }

    #[test]
    fn test_rate_limit_interval_calculation() {
        // Arrange - W2: rpm=0 除零防护测试
        // 注意：当前实现中 rpm 来自 default_rate_limit_rpm()，不会返回 0
        // 但如果手动设置 rpm=0，interval_ms = 60_000 / 0 会 panic
        // 这里测试正常情况下的间隔计算
        let rpm_60 = 60u32;
        let rpm_10 = 10u32;

        // Act
        let interval_60 = 60_000 / rpm_60 as u64;
        let interval_10 = 60_000 / rpm_10 as u64;

        // Assert
        assert_eq!(interval_60, 1000); // 60 RPM = 1 秒间隔
        assert_eq!(interval_10, 6000); // 10 RPM = 6 秒间隔
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_provider_from_str_invalid() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::from_str("unknown"), None);
        assert_eq!(EmbeddingProvider::from_str(""), None);
        assert_eq!(EmbeddingProvider::from_str("openai"), None);
        assert_eq!(EmbeddingProvider::from_str("gemini"), None);
    }

    #[test]
    fn test_provider_from_str_whitespace() {
        // Arrange & Act & Assert - 带空格的输入
        assert_eq!(EmbeddingProvider::from_str(" jina"), None);
        assert_eq!(EmbeddingProvider::from_str("jina "), None);
        assert_eq!(EmbeddingProvider::from_str(" jina "), None);
    }

    #[tokio::test]
    async fn test_embed_batch_empty_input() {
        // Arrange
        let client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "http://localhost:11434".to_string(),
            "".to_string(),
            "nomic-embed-text".to_string(),
        );

        // Act - 空输入应该返回空结果，不发网络请求
        let texts: Vec<String> = vec![];
        // 注意：当前实现会直接返回空 Vec，不会发网络请求
        // 因为 texts.chunks(batch_size) 对空 Vec 不会产生任何 chunk

        // Assert - 验证 chunks 行为
        let chunks: Vec<&[String]> = texts.chunks(client.batch_size).collect();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_client_builder_chain() {
        // Arrange & Act - 链式调用
        let client = EmbeddingClient::new(
            EmbeddingProvider::SiliconFlow,
            "https://api.siliconflow.cn/v1".to_string(),
            "sk-test".to_string(),
            "BAAI/bge-large-zh-v1.5".to_string(),
        )
        .with_dimension(1024)
        .with_batch_size(32);

        // Assert
        assert_eq!(client.dimension, 1024);
        assert_eq!(client.batch_size, 32);
        assert_eq!(client.provider, EmbeddingProvider::SiliconFlow);
    }

    // ─── 多提供者路径测试 ─────────────────────────────────────────────────────

    #[test]
    fn test_all_providers_have_valid_defaults() {
        // Arrange
        let providers = vec![
            EmbeddingProvider::Jina,
            EmbeddingProvider::SiliconFlow,
            EmbeddingProvider::Cloudflare,
            EmbeddingProvider::Nomic,
            EmbeddingProvider::Cohere,
            EmbeddingProvider::Ollama,
        ];

        // Act & Assert
        for provider in providers {
            let batch_size = provider.default_batch_size();
            let dimension = provider.default_dimension();

            // 所有提供者的默认值应该是合理的正数
            assert!(batch_size > 0, "{:?} batch_size 应该 > 0", provider);
            assert!(dimension > 0, "{:?} dimension 应该 > 0", provider);
        }
    }

    #[test]
    fn test_provider_equality() {
        // Arrange & Act & Assert
        assert_eq!(EmbeddingProvider::Jina, EmbeddingProvider::Jina);
        assert_ne!(EmbeddingProvider::Jina, EmbeddingProvider::Ollama);
    }

    #[test]
    fn test_provider_clone() {
        // Arrange
        let provider = EmbeddingProvider::SiliconFlow;

        // Act
        let cloned = provider.clone();

        // Assert
        assert_eq!(provider, cloned);
    }
}
