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

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = LocalIndexManager::compute_hash("hello");
        let hash2 = LocalIndexManager::compute_hash("hello");
        let hash3 = LocalIndexManager::compute_hash("world");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
