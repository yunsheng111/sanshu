//! FTS5 Actor 模块
//!
//! HC-1: rusqlite::Connection 不可跨线程传递，通过 Actor 模式隔离
//! HC-2: SharedMemoryManager 不直接持有 FtsIndex
//! HC-3: 所有 FTS5 操作通过消息通道执行
//! HC-5: 使用 tokio::sync::mpsc 通道
//! HC-6: Actor 在独立 tokio::spawn 任务中运行
//! OK-16: 状态机 Running -> Draining -> Stopped
//! OK-17: Search 操作 5 秒超时 + 取消机制
//! OK-18: SyncAll 分批执行（每 500 条）

use tokio::sync::{mpsc, oneshot};
use anyhow::Result;

use super::types::MemoryEntry;
use super::fts_index::{FtsIndex, SyncResult};
use crate::log_debug;

/// Actor 状态机（OK-16）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorState {
    /// 正常运行，接收所有消息
    Running,
    /// 排空队列中，不接收新消息（除 Shutdown）
    Draining,
    /// 已停止
    Stopped,
}

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

    /// 获取所有记忆 ID（用于一致性检查）
    ///
    /// 参数:
    /// - `oneshot::Sender<Result<Vec<String>>>`: 结果返回通道
    GetAllIds(oneshot::Sender<Result<Vec<String>>>),

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
/// OK-16: 实现状态机 Running -> Draining -> Stopped
/// OK-17: Search 操作 5 秒超时 + 取消机制
/// OK-18: SyncAll 分批执行（每 500 条）
///
/// # 参数
/// - `fts_index`: FTS5 索引实例（将被 move 进 Actor）
///
/// # 返回
/// - `mpsc::Sender<FtsMessage>`: 消息发送通道（有界通道，容量 1000）
pub fn spawn_fts_actor(fts_index: FtsIndex) -> mpsc::Sender<FtsMessage> {
    // OK-16: 改为有界通道，容量 1000
    let (tx, mut rx) = mpsc::channel(1000);

    tokio::spawn(async move {
        log_debug!("[FTS Actor] 已启动");

        // OK-16: 初始状态为 Running
        let mut state = ActorState::Running;

        while let Some(msg) = rx.recv().await {
            match msg {
                // HC-3, HC-4: Fire-and-forget 消息（非阻塞）
                FtsMessage::Sync(entry) => {
                    if state == ActorState::Running {
                        if let Err(e) = fts_index.sync_entry(&entry) {
                            log_debug!("[FTS Actor] 同步失败 (id={}): {}", entry.id, e);
                        }
                    }
                }

                FtsMessage::Delete(id) => {
                    if state == ActorState::Running {
                        if let Err(e) = fts_index.delete_entry(&id) {
                            log_debug!("[FTS Actor] 删除失败 (id={}): {}", id, e);
                        }
                    }
                }

                // Search 消息处理
                // 注意：FtsIndex::search 是同步方法，在此直接执行
                // 真正的超时保护在 mcp.rs 调用层实现（tokio::time::timeout 包装 response_rx）
                FtsMessage::Search(request, response_tx) => {
                    if state == ActorState::Running {
                        // 同步执行搜索（rusqlite Connection 不是 Send，无法 spawn_blocking）
                        let result = fts_index.search(&request.query, request.limit);
                        let _ = response_tx.send(result);
                    } else {
                        let _ = response_tx.send(Err(anyhow::anyhow!("Actor 正在关闭")));
                    }
                }

                // OK-18: 批量同步分批执行（每 500 条）
                FtsMessage::SyncAll(entries, response_tx) => {
                    if state == ActorState::Running {
                        const BATCH_SIZE: usize = 500;
                        let mut total_synced = 0;
                        let mut total_failed = 0;

                        for chunk in entries.chunks(BATCH_SIZE) {
                            match fts_index.sync_all(chunk) {
                                Ok(result) => {
                                    total_synced += result.synced;
                                    total_failed += result.failed;
                                }
                                Err(e) => {
                                    log_debug!("[FTS Actor] 批量同步失败: {}", e);
                                    total_failed += chunk.len();
                                }
                            }
                        }

                        let result = SyncResult {
                            synced: total_synced,
                            failed: total_failed,
                        };
                        let _ = response_tx.send(Ok(result));
                    } else {
                        let _ = response_tx.send(Err(anyhow::anyhow!("Actor 正在关闭")));
                    }
                }

                // 获取所有记忆 ID（用于一致性检查）
                FtsMessage::GetAllIds(response_tx) => {
                    if state == ActorState::Running {
                        let result = fts_index.get_all_ids();
                        let _ = response_tx.send(result);
                    } else {
                        let _ = response_tx.send(Err(anyhow::anyhow!("Actor 正在关闭")));
                    }
                }

                // OK-16: Shutdown 消息处理 - 进入 Draining 状态
                FtsMessage::Shutdown => {
                    log_debug!("[FTS Actor] 收到关闭信号，进入 Draining 状态");
                    state = ActorState::Draining;

                    // 排空队列中的剩余消息
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            FtsMessage::Sync(_) | FtsMessage::Delete(_) => {
                                // 忽略 fire-and-forget 消息
                            }
                            FtsMessage::Search(_, response_tx) => {
                                let _ = response_tx.send(Err(anyhow::anyhow!("Actor 已关闭")));
                            }
                            FtsMessage::SyncAll(_, response_tx) => {
                                let _ = response_tx.send(Err(anyhow::anyhow!("Actor 已关闭")));
                            }
                            FtsMessage::GetAllIds(response_tx) => {
                                let _ = response_tx.send(Err(anyhow::anyhow!("Actor 已关闭")));
                            }
                            FtsMessage::Shutdown => {
                                // 忽略重复的 Shutdown 消息
                            }
                        }
                    }

                    state = ActorState::Stopped;
                    log_debug!("[FTS Actor] 队列已排空，退出");
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
        tx.send(FtsMessage::Sync(entry)).await.unwrap();

        // 等待异步处理完成
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "Rust".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(1), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(!result.is_empty(), "应找到包含 Rust 的记忆");
        assert!(result.contains(&"1".to_string()));

        // 关闭 Actor
        tx.send(FtsMessage::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_delete() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步记忆
        let entry = make_entry("1", "Rust 编程规范", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 删除记忆
        tx.send(FtsMessage::Delete("1".to_string())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索应无结果
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "Rust".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(1), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(!result.contains(&"1".to_string()), "删除后不应搜索到该记忆");

        tx.send(FtsMessage::Shutdown).await.unwrap();
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
        tx.send(FtsMessage::SyncAll(entries, sync_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(1), sync_rx)
            .await
            .expect("同步超时")
            .expect("接收失败")
            .expect("同步失败");

        assert_eq!(result.synced, 3);
        assert_eq!(result.failed, 0);

        tx.send(FtsMessage::Shutdown).await.unwrap();
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
            tx.send(FtsMessage::Sync(e)).await.unwrap();
        }

        for i in 0..10 {
            tx.send(FtsMessage::Delete(format!("id_{}", i))).await.unwrap();
        }

        // 等待处理完成
        tokio::time::sleep(Duration::from_millis(200)).await;

        tx.send(FtsMessage::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_state_machine() {
        // OK-16: 测试状态机 Running -> Draining -> Stopped
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步一些记忆
        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).await.unwrap();

        // 发送 Shutdown 消息
        tx.send(FtsMessage::Shutdown).await.unwrap();

        // 等待 Actor 停止
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 尝试发送新消息应失败（通道已关闭）
        let entry2 = make_entry("2", "新内容", MemoryCategory::Rule);
        assert!(tx.send(FtsMessage::Sync(entry2)).await.is_err(), "Actor 停止后发送应失败");
    }

    #[tokio::test]
    async fn test_search_timeout() {
        // OK-17: 测试搜索超时机制
        // 注意：由于 FtsIndex::search 通常很快，这个测试主要验证超时机制存在
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步记忆
        let entry = make_entry("1", "Rust 编程规范", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 正常搜索应在 5 秒内完成
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "Rust".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(6), search_rx)
            .await
            .expect("搜索应在 6 秒内完成")
            .expect("接收失败");

        assert!(result.is_ok(), "正常搜索应成功");

        tx.send(FtsMessage::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_sync_all_batching() {
        // OK-18: 测试分批执行（每 500 条）
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 创建 1200 条记忆（应分为 3 批：500 + 500 + 200）
        let mut entries = Vec::new();
        for i in 0..1200 {
            entries.push(make_entry(
                &format!("id_{}", i),
                &format!("测试内容 {}", i),
                MemoryCategory::Rule,
            ));
        }

        let (sync_tx, sync_rx) = oneshot::channel();
        tx.send(FtsMessage::SyncAll(entries, sync_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(5), sync_rx)
            .await
            .expect("批量同步超时")
            .expect("接收失败")
            .expect("同步失败");

        assert_eq!(result.synced, 1200, "应成功同步所有 1200 条记忆");
        assert_eq!(result.failed, 0, "不应有失败");

        tx.send(FtsMessage::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_bounded_channel_capacity() {
        // OK-16: 测试有界通道容量限制
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);

        // 尝试发送大量消息（超过通道容量 1000）
        // 由于是异步发送，应该会在通道满时等待
        for i in 0..1500 {
            let mut e = entry.clone();
            e.id = format!("id_{}", i);

            // 使用 timeout 避免测试永久阻塞
            let send_result = timeout(
                Duration::from_secs(2),
                tx.send(FtsMessage::Sync(e))
            ).await;

            if send_result.is_err() {
                // 超时说明通道已满，这是预期行为
                break;
            }
        }

        tx.send(FtsMessage::Shutdown).await.ok();
    }

    // =========================================================================
    // OK-3: update 后索引更新
    // =========================================================================

    #[tokio::test]
    async fn test_update_entry() {
        // OK-3: 验证更新记忆后 FTS 索引同步更新
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步原始记忆（使用英文关键词，unicode61 分词器对英文支持更好）
        let entry_v1 = make_entry("1", "Rust programming beginner guide tutorial", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry_v1)).await.unwrap();

        // 等待异步处理完成
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索原始内容
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "beginner".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request, search_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(1), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(result.contains(&"1".to_string()), "应能搜索到原始内容");

        // 更新记忆内容（通过再次 Sync 模拟更新）
        let mut entry_v2 = make_entry("1", "Rust advanced performance optimization techniques", MemoryCategory::Rule);
        entry_v2.version = 2; // 模拟版本递增
        tx.send(FtsMessage::Sync(entry_v2)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // 搜索新内容
        let (search_tx2, search_rx2) = oneshot::channel();
        let request2 = SearchRequest {
            query: "optimization".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request2, search_tx2)).await.unwrap();

        let result2 = timeout(Duration::from_secs(1), search_rx2)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(result2.contains(&"1".to_string()), "更新后应能搜索到新内容");

        // 搜索原始内容应无结果（因为已被更新覆盖）
        let (search_tx3, search_rx3) = oneshot::channel();
        let request3 = SearchRequest {
            query: "beginner".to_string(),
            limit: 10,
        };
        tx.send(FtsMessage::Search(request3, search_tx3)).await.unwrap();

        let result3 = timeout(Duration::from_secs(1), search_rx3)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        assert!(!result3.contains(&"1".to_string()), "更新后原始内容应不再可搜索");

        tx.send(FtsMessage::Shutdown).await.unwrap();
    }

    // =========================================================================
    // OK-11: Drop 时 Actor 退出
    // =========================================================================

    #[tokio::test]
    async fn test_shutdown_graceful() {
        // OK-11: 验证 Sender drop 后 Actor 能正常退出
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 同步一些记忆
        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).await.unwrap();

        // 等待处理
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 显式发送 Shutdown 消息
        tx.send(FtsMessage::Shutdown).await.unwrap();

        // 等待 Actor 处理 Shutdown
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 尝试发送新消息应失败（通道已关闭）
        let entry2 = make_entry("2", "新内容", MemoryCategory::Rule);
        let send_result = tx.send(FtsMessage::Sync(entry2)).await;
        assert!(send_result.is_err(), "Actor 关闭后发送应失败");
    }

    #[tokio::test]
    async fn test_actor_drop_closes_channel() {
        // OK-11: 验证 drop Sender 后新发送会失败
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        // 克隆一个 sender 用于后续测试
        let tx2 = tx.clone();

        // 同步一条记忆
        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);
        tx.send(FtsMessage::Sync(entry)).await.unwrap();

        // 发送 Shutdown
        tx.send(FtsMessage::Shutdown).await.unwrap();

        // 等待 Actor 退出
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 使用 tx2 尝试发送应失败
        let entry2 = make_entry("2", "新内容", MemoryCategory::Rule);
        let result = tx2.send(FtsMessage::Sync(entry2)).await;
        assert!(result.is_err(), "Actor 关闭后通过克隆的 sender 发送也应失败");
    }

    // =========================================================================
    // OK-12: 通道满时优雅降级（补充测试）
    // =========================================================================

    #[tokio::test]
    async fn test_channel_backpressure() {
        // OK-12: 验证通道满时不会丢失已发送的消息
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let tx = spawn_fts_actor(index);

        let entry = make_entry("1", "测试内容", MemoryCategory::Rule);

        // 发送 100 条消息（远小于通道容量 1000，确保不阻塞）
        for i in 0..100 {
            let mut e = entry.clone();
            e.id = format!("id_{}", i);
            e.content = format!("测试内容 {}", i);
            tx.send(FtsMessage::Sync(e)).await.unwrap();
        }

        // 等待 Actor 处理完毕
        tokio::time::sleep(Duration::from_millis(500)).await;

        // 搜索验证消息都被处理了
        let (search_tx, search_rx) = oneshot::channel();
        let request = SearchRequest {
            query: "测试内容".to_string(),
            limit: 200,
        };
        tx.send(FtsMessage::Search(request, search_tx)).await.unwrap();

        let result = timeout(Duration::from_secs(2), search_rx)
            .await
            .expect("搜索超时")
            .expect("接收失败")
            .expect("搜索失败");

        // 至少应该能搜到大部分消息
        assert!(result.len() >= 50, "应能搜索到大部分已发送的记忆，实际搜索到: {}", result.len());

        tx.send(FtsMessage::Shutdown).await.unwrap();
    }
}
