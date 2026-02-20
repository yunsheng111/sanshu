//! FTS5 Actor 模块
//!
//! HC-1: rusqlite::Connection 不可跨线程传递，通过 Actor 模式隔离
//! HC-2: SharedMemoryManager 不直接持有 FtsIndex
//! HC-3: 所有 FTS5 操作通过消息通道执行
//! HC-5: 使用 tokio::sync::mpsc 通道
//! HC-6: Actor 在独立 tokio::spawn 任务中运行

use tokio::sync::{mpsc, oneshot};
use anyhow::Result;

use super::types::MemoryEntry;
use super::fts_index::{FtsIndex, SyncResult};
use crate::log_debug;

/// FTS5 Actor 消息类型
#[derive(Debug)]
pub enum FtsMessage {
    /// 同步单条记忆到 FTS5 索引
    ///
    /// 参数:
    /// - `MemoryEntry`: 要同步的记忆条目
    Sync(MemoryEntry),

    /// 删除单条记忆的 FTS5 索引
    ///
    /// 参数:
    /// - `String`: 记忆 ID
    Delete(String),

    /// FTS5 全文搜索
    ///
    /// 参数:
    /// - `SearchRequest`: 搜索请求
    /// - `oneshot::Sender<Result<Vec<String>>>`: 结果返回通道
    Search(SearchRequest, oneshot::Sender<Result<Vec<String>>>),

    /// 批量同步所有记忆到 FTS5 索引
    ///
    /// 参数:
    /// - `Vec<MemoryEntry>`: 所有记忆条目
    /// - `oneshot::Sender<Result<SyncResult>>`: 结果返回通道
    SyncAll(Vec<MemoryEntry>, oneshot::Sender<Result<SyncResult>>),

    /// 关闭 Actor（优雅退出）
    Shutdown,
}

/// FTS5 搜索请求
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// 搜索查询字符串（FTS5 查询语法）
    pub query: String,

    /// 返回结果数量限制
    pub limit: usize,
}

/// 启动 FTS5 Actor
///
/// HC-6: 在独立 tokio::spawn 任务中运行
/// HC-1: FtsIndex 被 move 进 Actor 任务，不跨线程传递
///
/// # 参数
/// - `fts_index`: FTS5 索引实例（将被 move 进 Actor）
///
/// # 返回
/// - `mpsc::UnboundedSender<FtsMessage>`: 消息发送通道
pub fn spawn_fts_actor(fts_index: FtsIndex) -> mpsc::UnboundedSender<FtsMessage> {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        log_debug!("[FTS Actor] 已启动");

        while let Some(msg) = rx.recv().await {
            match msg {
                // HC-3, HC-4: Fire-and-forget 消息（非阻塞）
                FtsMessage::Sync(entry) => {
                    if let Err(e) = fts_index.sync_entry(&entry) {
                        log_debug!("[FTS Actor] 同步失败 (id={}): {}", entry.id, e);
                    }
                }

                FtsMessage::Delete(id) => {
                    if let Err(e) = fts_index.delete_entry(&id) {
                        log_debug!("[FTS Actor] 删除失败 (id={}): {}", id, e);
                    }
                }

                // HC-7: 搜索操作使用 oneshot 返回结果
                FtsMessage::Search(request, response_tx) => {
                    let result = fts_index.search(&request.query, request.limit);
                    let _ = response_tx.send(result);
                }

                // HC-8: 批量同步使用 oneshot 返回结果
                FtsMessage::SyncAll(entries, response_tx) => {
                    let result = fts_index.sync_all(&entries);
                    let _ = response_tx.send(result);
                }

                // HC-6: 优雅退出
                FtsMessage::Shutdown => {
                    log_debug!("[FTS Actor] 收到关闭信号，退出");
                    break;
                }
            }
        }

        log_debug!("[FTS Actor] 已停止");
    });

    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{MemoryEntry, MemoryCategory};
    use super::super::similarity::TextSimilarity;
    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};

    fn make_entry(id: &str, content: &str, category: MemoryCategory) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            content_normalized: TextSimilarity::normalize(content),
            category,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: Some("core".to_string()),
            tags: Some(vec!["test".to_string()]),
            vitality_score: Some(1.5),
            last_accessed_at: Some(now),
            summary: None,
        }
    }

    #[tokio::test]
    async fn test_actor_sync_and_search() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步记忆
        let entry = make_entry("1", "Rust 编程规范", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).unwrap();

        // 等待异步处理完成
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "Rust".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).unwrap();

        let result = timeout(Duration::from_secs(1), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(!result.is_empty(), "应找到包含 Rust 的记忆");
        assert!(result.contains(&"1".to_string()));

        // 关闭 Actor
        tx.send(FtsMessage::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn test_actor_delete() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步记忆
        let entry = make_entry("1", "Rust 编程规范", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 删除记忆
        tx.send(FtsMessage::Delete("1".to_string())).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索应无结果
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "Rust".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).unwrap();

        let result = timeout(Duration::from_secs(1), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(!result.contains(&"1".to_string()), "删除后不应搜索到该记忆");

        tx.send(FtsMessage::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn test_actor_sync_all() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 批量同步
        let entries = vec![
            make_entry("1", "Rust 编程规范", MemoryCategory::Rule),
            make_entry("2", "使用 tokio 异步运行时", MemoryCategory::Pattern),
            make_entry("3", "偏好简洁的代码风格", MemoryCategory::Preference),
        ];

        let (sync_tx, sync_rx) = oneshot::channel();
        tx.send(FtsMessage::SyncAll(entries, sync_tx)).unwrap();

        let result = timeout(Duration::from_secs(1), sync_rx)
            .await
            .expect("同步超时")
            .expect("接收失败")
            .expect("同步失败");

        assert_eq!(result.synced, 3);
        assert_eq!(result.failed, 0);

        tx.send(FtsMessage::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn test_actor_fire_and_forget() {
        // 验证 Sync 和 Delete 是 fire-and-forget（不阻塞）
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);

        // 发送多条消息不应阻塞
        for i in 0..10 {
            let mut e = entry.clone();
            e.id = format!("id_{}", i);
            tx.send(FtsMessage::Sync(e)).unwrap();
        }

        for i in 0..10 {
            tx.send(FtsMessage::Delete(format!("id_{}", i))).unwrap();
        }

        // 等待处理完成
        tokio::time::sleep(Duration::from_millis(200)).await;

        tx.send(FtsMessage::Shutdown).unwrap();
    }
}
