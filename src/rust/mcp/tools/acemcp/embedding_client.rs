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
