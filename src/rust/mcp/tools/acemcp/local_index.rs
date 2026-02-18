// 本地向量索引管理模块
// 支持增量更新、并发安全、原子写入

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::embedding_client::EmbeddingClient;

/// 文件索引条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// 文件路径（相对于项目根）
    pub path: String,
    /// 文件内容 hash（用于增量更新判断）
    pub content_hash: String,
    /// 嵌入向量
    pub embedding: Vec<f32>,
    /// 最后更新时间戳
    pub updated_at: u64,
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
}

/// 索引文件格式
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexFile {
    /// 版本号（用于格式迁移）
    pub version: u32,
    /// 嵌入模型名称
    pub model: String,
    /// 向量维度
    pub dimension: usize,
    /// 文件索引映射
    pub entries: HashMap<String, IndexEntry>,
}

impl IndexFile {
    pub fn new(model: &str, dimension: usize) -> Self {
        Self {
            version: 1,
            model: model.to_string(),
            dimension,
            entries: HashMap::new(),
        }
    }
}

/// 本地索引管理器
pub struct LocalIndexManager {
    /// 并发安全锁
    index_lock: RwLock<()>,
    /// 索引文件路径
    index_path: PathBuf,
    /// 嵌入客户端
    embedding_client: EmbeddingClient,
    /// 项目根路径
    project_root: PathBuf,
}

impl LocalIndexManager {
    pub fn new(
        index_path: PathBuf,
        embedding_client: EmbeddingClient,
        project_root: PathBuf,
    ) -> Self {
        Self {
            index_lock: RwLock::new(()),
            index_path,
            embedding_client,
            project_root,
        }
    }

    /// 计算文件内容 hash（使用 SHA-256 确保跨版本稳定）
    fn compute_hash(content: &str) -> String {
        use ring::digest::{Context, SHA256};
        let mut context = Context::new(&SHA256);
        context.update(content.as_bytes());
        let digest = context.finish();
        hex::encode(digest.as_ref())
    }

    /// 加载现有索引
    fn load_index(&self) -> Result<IndexFile> {
        if !self.index_path.exists() {
            return Ok(IndexFile::new(
                &self.embedding_client.model,
                self.embedding_client.dimension,
            ));
        }
        let content = fs::read(&self.index_path)?;
        let index: IndexFile = bincode::deserialize(&content)?;
        Ok(index)
    }

    /// 原子写入索引
    fn save_index(&self, index: &IndexFile) -> Result<()> {
        // 确保父目录存在
        if let Some(parent) = self.index_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 先写入临时文件
        let tmp_path = self.index_path.with_extension("tmp");
        let serialized = bincode::serialize(index)?;
        fs::write(&tmp_path, &serialized)?;

        // Windows 下 fs::rename 不能覆盖已存在的文件，需要先删除
        if self.index_path.exists() {
            fs::remove_file(&self.index_path)?;
        }
        fs::rename(&tmp_path, &self.index_path)?;
        Ok(())
    }

    /// 增量更新索引
    pub async fn update_index(&self, files: &[PathBuf]) -> Result<usize> {
        let _guard = self.index_lock.write().await;

        let mut index = self.load_index()?;
        let mut updated_count = 0;

        // 收集需要更新的文件
        let mut to_embed: Vec<(String, String)> = Vec::new();

        for file_path in files {
            let rel_path = file_path
                .strip_prefix(&self.project_root)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let hash = Self::compute_hash(&content);

            // 检查是否需要更新
            if let Some(entry) = index.entries.get(&rel_path) {
                if entry.content_hash == hash {
                    continue; // 文件未变更，跳过
                }
            }

            to_embed.push((rel_path, content));
        }

        if to_embed.is_empty() {
            return Ok(0);
        }

        // 批量生成嵌入
        let texts: Vec<String> = to_embed.iter().map(|(_, c)| c.clone()).collect();
        let embeddings = self.embedding_client.embed_batch(&texts).await?;

        // 更新索引
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for ((rel_path, content), embedding) in to_embed.into_iter().zip(embeddings) {
            let hash = Self::compute_hash(&content);
            index.entries.insert(
                rel_path.clone(),
                IndexEntry {
                    path: rel_path,
                    content_hash: hash,
                    embedding,
                    updated_at: now,
                },
            );
            updated_count += 1;
        }

        // 原子写入
        self.save_index(&index)?;

        Ok(updated_count)
    }

    /// 向量搜索
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        let _guard = self.index_lock.read().await;

        let index = self.load_index()?;
        if index.entries.is_empty() {
            return Ok(Vec::new());
        }

        // 生成查询向量
        let query_embedding = self.embedding_client.embed(query).await?;

        // 计算余弦相似度
        let mut scores: Vec<(String, f32)> = index
            .entries
            .iter()
            .map(|(path, entry)| {
                let score = cosine_similarity(&query_embedding, &entry.embedding);
                (path.clone(), score)
            })
            .collect();

        // 按分数降序排序
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 取 top_k
        let results: Vec<SearchResult> = scores
            .into_iter()
            .take(top_k)
            .map(|(path, score)| SearchResult { path, score })
            .collect();

        Ok(results)
    }

    /// 删除索引中的文件
    pub async fn remove_files(&self, paths: &[String]) -> Result<usize> {
        let _guard = self.index_lock.write().await;

        let mut index = self.load_index()?;
        let mut removed_count = 0;

        for path in paths {
            if index.entries.remove(path).is_some() {
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            self.save_index(&index)?;
        }

        Ok(removed_count)
    }

    /// 获取索引统计信息
    pub async fn stats(&self) -> Result<IndexStats> {
        let _guard = self.index_lock.read().await;
        let index = self.load_index()?;
        Ok(IndexStats {
            file_count: index.entries.len(),
            model: index.model,
            dimension: index.dimension,
        })
    }
}

/// 索引统计信息
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub file_count: usize,
    pub model: String,
    pub dimension: usize,
}

/// 计算余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ─── 辅助函数 ─────────────────────────────────────────────────────────────

    /// 创建测试用的临时目录和索引管理器（不依赖网络）
    fn make_test_index_file(dir: &TempDir) -> IndexFile {
        let model = "test-model";
        let dimension = 4;
        IndexFile::new(model, dimension)
    }

    // ─── 正常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        // Arrange
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        // Act
        let score = cosine_similarity(&a, &b);

        // Assert - 相同向量余弦相似度应为 1.0
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        // Arrange
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        // Act
        let score = cosine_similarity(&a, &c);

        // Assert - 正交向量余弦相似度应为 0.0
        assert!((score - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        // Arrange
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];

        // Act
        let score = cosine_similarity(&a, &b);

        // Assert - 反向向量余弦相似度应为 -1.0
        assert!((score - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_compute_hash_stability() {
        // Arrange - W1: SHA-256 跨版本稳定性
        let content = "hello world";

        // Act
        let hash1 = LocalIndexManager::compute_hash(content);
        let hash2 = LocalIndexManager::compute_hash(content);

        // Assert - 相同内容 hash 必须完全一致
        assert_eq!(hash1, hash2);
        // SHA-256 输出为 64 位十六进制字符串
        assert_eq!(hash1.len(), 64);
        // 验证是合法的十六进制字符串
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_compute_hash_different_content() {
        // Arrange
        let content_a = "hello";
        let content_b = "world";

        // Act
        let hash_a = LocalIndexManager::compute_hash(content_a);
        let hash_b = LocalIndexManager::compute_hash(content_b);

        // Assert - 不同内容 hash 必须不同
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_compute_hash_known_value() {
        // Arrange - 验证 SHA-256 实现正确性（已知值）
        // echo -n "hello" | sha256sum = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let content = "hello";

        // Act
        let hash = LocalIndexManager::compute_hash(content);

        // Assert
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn test_index_file_new() {
        // Arrange & Act
        let index = IndexFile::new("test-model", 768);

        // Assert
        assert_eq!(index.version, 1);
        assert_eq!(index.model, "test-model");
        assert_eq!(index.dimension, 768);
        assert!(index.entries.is_empty());
    }

    #[test]
    fn test_save_and_load_index_atomic_write() {
        // Arrange - 原子写入测试
        use crate::mcp::tools::acemcp::embedding_client::{EmbeddingClient, EmbeddingProvider};
        let tmp = TempDir::new().unwrap();
        let index_path = tmp.path().join("test.idx");
        let project_root = tmp.path().to_path_buf();

        let embedding_client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "http://localhost:11434".to_string(),
            "".to_string(),
            "nomic-embed-text".to_string(),
        );
        let manager = LocalIndexManager::new(index_path.clone(), embedding_client, project_root);

        let mut index = IndexFile::new("test-model", 4);
        index.entries.insert(
            "src/main.rs".to_string(),
            IndexEntry {
                path: "src/main.rs".to_string(),
                content_hash: "abc123".to_string(),
                embedding: vec![0.1, 0.2, 0.3, 0.4],
                updated_at: 1000,
            },
        );

        // Act - 原子写入
        manager.save_index(&index).unwrap();

        // Assert - 文件存在，临时文件不存在
        assert!(index_path.exists());
        let tmp_path = index_path.with_extension("tmp");
        assert!(!tmp_path.exists(), "原子写入后临时文件应被删除");

        // 验证可以重新加载
        let loaded = manager.load_index().unwrap();
        assert_eq!(loaded.model, "test-model");
        assert_eq!(loaded.dimension, 4);
        assert!(loaded.entries.contains_key("src/main.rs"));
    }

    #[test]
    fn test_load_index_when_file_not_exists() {
        // Arrange
        use crate::mcp::tools::acemcp::embedding_client::{EmbeddingClient, EmbeddingProvider};
        let tmp = TempDir::new().unwrap();
        let index_path = tmp.path().join("nonexistent.idx");
        let project_root = tmp.path().to_path_buf();

        let embedding_client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "http://localhost:11434".to_string(),
            "".to_string(),
            "nomic-embed-text".to_string(),
        );
        let manager = LocalIndexManager::new(index_path, embedding_client, project_root);

        // Act - 文件不存在时应返回空索引
        let index = manager.load_index().unwrap();

        // Assert
        assert!(index.entries.is_empty());
        assert_eq!(index.model, "nomic-embed-text");
        assert_eq!(index.dimension, 768); // Ollama 默认维度
    }

    #[test]
    fn test_save_index_creates_parent_directory() {
        // Arrange
        use crate::mcp::tools::acemcp::embedding_client::{EmbeddingClient, EmbeddingProvider};
        let tmp = TempDir::new().unwrap();
        // 嵌套目录，父目录不存在
        let index_path = tmp.path().join("nested").join("deep").join("test.idx");
        let project_root = tmp.path().to_path_buf();

        let embedding_client = EmbeddingClient::new(
            EmbeddingProvider::Jina,
            "https://api.jina.ai/v1".to_string(),
            "test-key".to_string(),
            "jina-embeddings-v2-base-en".to_string(),
        );
        let manager = LocalIndexManager::new(index_path.clone(), embedding_client, project_root);
        let index = IndexFile::new("test-model", 768);

        // Act - 应自动创建父目录
        let result = manager.save_index(&index);

        // Assert
        assert!(result.is_ok(), "应自动创建父目录并写入成功");
        assert!(index_path.exists());
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        // Arrange - W3: 空向量边界
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];

        // Act
        let score = cosine_similarity(&a, &b);

        // Assert - 空向量应返回 0.0，不 panic
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vectors() {
        // Arrange - 零向量（范数为 0）
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        // Act
        let score = cosine_similarity(&a, &b);

        // Assert - 零向量应返回 0.0，不 panic（除零防护）
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        // Arrange - 维度不匹配
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        // Act
        let score = cosine_similarity(&a, &b);

        // Assert - 维度不匹配应返回 0.0，不 panic
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_compute_hash_empty_string() {
        // Arrange - 空字符串 hash
        // echo -n "" | sha256sum = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let content = "";

        // Act
        let hash = LocalIndexManager::compute_hash(content);

        // Assert
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_compute_hash_unicode_content() {
        // Arrange - W1: UTF-8 多字节字符 hash 稳定性
        let content_zh = "你好世界";
        let content_emoji = "🦀 Rust";

        // Act
        let hash_zh1 = LocalIndexManager::compute_hash(content_zh);
        let hash_zh2 = LocalIndexManager::compute_hash(content_zh);
        let hash_emoji = LocalIndexManager::compute_hash(content_emoji);

        // Assert - 相同 Unicode 内容 hash 必须稳定
        assert_eq!(hash_zh1, hash_zh2);
        assert_ne!(hash_zh1, hash_emoji);
        assert_eq!(hash_zh1.len(), 64);
    }

    #[test]
    fn test_index_file_default() {
        // Arrange & Act
        let index = IndexFile::default();

        // Assert
        assert_eq!(index.version, 0);
        assert!(index.entries.is_empty());
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_save_index_overwrites_existing() {
        // Arrange
        use crate::mcp::tools::acemcp::embedding_client::{EmbeddingClient, EmbeddingProvider};
        let tmp = TempDir::new().unwrap();
        let index_path = tmp.path().join("test.idx");
        let project_root = tmp.path().to_path_buf();

        let embedding_client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "http://localhost:11434".to_string(),
            "".to_string(),
            "nomic-embed-text".to_string(),
        );
        let manager = LocalIndexManager::new(index_path.clone(), embedding_client, project_root);

        // 第一次写入
        let mut index_v1 = IndexFile::new("model-v1", 4);
        index_v1.entries.insert("a.rs".to_string(), IndexEntry {
            path: "a.rs".to_string(),
            content_hash: "hash1".to_string(),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            updated_at: 1000,
        });
        manager.save_index(&index_v1).unwrap();

        // Act - 第二次写入（覆盖）
        let index_v2 = IndexFile::new("model-v2", 8);
        manager.save_index(&index_v2).unwrap();

        // Assert - 读取到的是第二次写入的内容
        let loaded = manager.load_index().unwrap();
        assert_eq!(loaded.model, "model-v2");
        assert_eq!(loaded.dimension, 8);
        assert!(loaded.entries.is_empty());
    }

    #[test]
    fn test_hash_sensitivity_to_whitespace() {
        // Arrange - hash 对空白字符敏感
        let content_a = "hello world";
        let content_b = "hello  world"; // 双空格

        // Act
        let hash_a = LocalIndexManager::compute_hash(content_a);
        let hash_b = LocalIndexManager::compute_hash(content_b);

        // Assert
        assert_ne!(hash_a, hash_b, "空白字符差异应产生不同 hash");
    }

    #[test]
    fn test_hash_sensitivity_to_newline() {
        // Arrange - hash 对换行符敏感（增量更新判断依赖此特性）
        let content_a = "fn main() {}";
        let content_b = "fn main() {}\n";

        // Act
        let hash_a = LocalIndexManager::compute_hash(content_a);
        let hash_b = LocalIndexManager::compute_hash(content_b);

        // Assert
        assert_ne!(hash_a, hash_b, "末尾换行符差异应产生不同 hash");
    }
}
