//! FTS 与 JSON 数据一致性验证工具
//!
//! HC-08: FTS 索引与 JSON 存储必须保持一致
//! SC-05: 提供一致性检测和修复工具
//!
//! ## 核心功能
//! - `verify_consistency`: 检测 FTS 索引与 JSON 存储的差异
//! - `repair_consistency`: 自动修复不一致（补齐缺失项、删除孤立项）
//!
//! ## 使用场景
//! - 启动时自动检测（可选）
//! - 手动触发修复（通过 MCP 工具）
//! - 定期后台校验（未来扩展）

use super::manager::SharedMemoryManager;
use super::fts_actor::{FtsMessage, SearchRequest};
use anyhow::Result;
use tokio::sync::oneshot;
use std::collections::HashSet;

/// 一致性检测报告
#[derive(Debug, Clone)]
pub struct ConsistencyReport {
    /// JSON 中的记忆总数
    pub json_count: usize,
    /// FTS 索引中的记忆总数
    pub fts_count: usize,
    /// JSON 中存在但 FTS 中缺失的记忆 ID 列表
    pub missing_in_fts: Vec<String>,
    /// FTS 中存在但 JSON 中不存在的孤立记忆 ID 列表
    pub orphaned_in_fts: Vec<String>,
}

impl ConsistencyReport {
    /// 判断是否一致（无差异）
    pub fn is_consistent(&self) -> bool {
        self.missing_in_fts.is_empty() && self.orphaned_in_fts.is_empty()
    }

    /// 获取差异总数
    pub fn total_inconsistencies(&self) -> usize {
        self.missing_in_fts.len() + self.orphaned_in_fts.len()
    }
}

/// 验证 FTS 索引与 JSON 存储的一致性
///
/// # 参数
/// - `manager`: SharedMemoryManager 实例（用于读取 JSON 数据）
/// - `fts_tx`: FTS Actor 消息发送通道
///
/// # 返回
/// - `Ok(ConsistencyReport)`: 一致性检测报告
/// - `Err(...)`: 检测失败（如 FTS Actor 不可用）
///
/// # 实现逻辑
/// 1. 从 JSON 获取所有记忆 ID（通过 `get_all_memories`）
/// 2. 从 FTS 获取所有 ID（通过空查询 + limit=10000）
/// 3. 计算差集：
///    - `missing_in_fts` = JSON IDs - FTS IDs
///    - `orphaned_in_fts` = FTS IDs - JSON IDs
pub async fn verify_consistency(
    manager: &SharedMemoryManager,
    fts_tx: &tokio::sync::mpsc::Sender<FtsMessage>,
) -> Result<ConsistencyReport> {
    // 1. 从 JSON 获取所有记忆 IDs
    let json_entries = manager.get_all_memories()
        .map_err(|e| anyhow::anyhow!("获取 JSON 记忆列表失败: {}", e))?;
    let json_ids: HashSet<String> = json_entries.iter().map(|e| e.id.clone()).collect();

    // 2. 从 FTS 获取所有 IDs（通过空查询）
    let (tx, rx) = oneshot::channel();
    let request = SearchRequest {
        query: String::new(), // 空查询匹配所有记录
        limit: 10000,         // 足够大的限制（假设记忆总数 < 10000）
    };

    fts_tx.send(FtsMessage::Search(request, tx))
        .await
        .map_err(|e| anyhow::anyhow!("发送 FTS 搜索请求失败: {}", e))?;

    let fts_results = rx.await
        .map_err(|e| anyhow::anyhow!("接收 FTS 搜索结果失败: {}", e))??;

    let fts_ids: HashSet<String> = fts_results.into_iter().collect();

    // 3. 计算差异
    let missing_in_fts: Vec<String> = json_ids.difference(&fts_ids).cloned().collect();
    let orphaned_in_fts: Vec<String> = fts_ids.difference(&json_ids).cloned().collect();

    Ok(ConsistencyReport {
        json_count: json_ids.len(),
        fts_count: fts_ids.len(),
        missing_in_fts,
        orphaned_in_fts,
    })
}

/// 修复 FTS 索引与 JSON 存储的不一致
///
/// # 参数
/// - `manager`: SharedMemoryManager 实例（用于读取记忆内容）
/// - `fts_tx`: FTS Actor 消息发送通道
/// - `report`: 一致性检测报告（由 `verify_consistency` 生成）
///
/// # 返回
/// - `Ok(())`: 修复成功
/// - `Err(...)`: 修复失败
///
/// # 实现逻辑
/// 1. 补齐 FTS 缺失项：
///    - 遍历 `report.missing_in_fts`
///    - 从 JSON 读取完整 MemoryEntry
///    - 发送 `FtsMessage::Sync` 同步到 FTS
/// 2. 删除 FTS 孤立项：
///    - 遍历 `report.orphaned_in_fts`
///    - 发送 `FtsMessage::Delete` 删除索引
///
/// # 注意事项
/// - 修复操作是异步的（fire-and-forget）
/// - 不保证立即生效（需等待 FTS Actor 处理）
/// - 修复失败不会回滚（仅记录日志）
pub async fn repair_consistency(
    manager: &SharedMemoryManager,
    fts_tx: &tokio::sync::mpsc::Sender<FtsMessage>,
    report: &ConsistencyReport,
) -> Result<()> {
    use crate::log_debug;

    // 1. 补齐 FTS 缺失项
    if !report.missing_in_fts.is_empty() {
        log_debug!(
            "[Consistency] 开始补齐 FTS 缺失项: {} 条",
            report.missing_in_fts.len()
        );

        let all_entries = manager.get_all_memories()
            .map_err(|e| anyhow::anyhow!("获取 JSON 记忆列表失败: {}", e))?;

        for missing_id in &report.missing_in_fts {
            // 查找对应的 MemoryEntry
            if let Some(entry) = all_entries.iter().find(|e| &e.id == missing_id) {
                // 发送同步消息（fire-and-forget）
                if let Err(e) = fts_tx.send(FtsMessage::Sync(entry.clone())).await {
                    log_debug!(
                        "[Consistency] 补齐失败: memory_id={}, error={}",
                        missing_id,
                        e
                    );
                }
            } else {
                log_debug!(
                    "[Consistency] 警告: JSON 中未找到记忆 ID={}（可能已被删除）",
                    missing_id
                );
            }
        }
    }

    // 2. 删除 FTS 孤立项
    if !report.orphaned_in_fts.is_empty() {
        log_debug!(
            "[Consistency] 开始删除 FTS 孤立项: {} 条",
            report.orphaned_in_fts.len()
        );

        for orphaned_id in &report.orphaned_in_fts {
            // 发送删除消息（fire-and-forget）
            if let Err(e) = fts_tx.send(FtsMessage::Delete(orphaned_id.clone())).await {
                log_debug!(
                    "[Consistency] 删除失败: memory_id={}, error={}",
                    orphaned_id,
                    e
                );
            }
        }
    }

    log_debug!(
        "[Consistency] 修复完成: 补齐 {} 条，删除 {} 条",
        report.missing_in_fts.len(),
        report.orphaned_in_fts.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::manager::MemoryManager;
    use super::super::types::{MemoryEntry, MemoryCategory};
    use super::super::similarity::TextSimilarity;
    use super::super::fts_index::FtsIndex;
    use super::super::fts_actor::spawn_fts_actor;
    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};
    use std::fs;

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
    async fn test_verify_consistency_all_consistent() {
        // 场景：JSON 和 FTS 完全一致
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建独立的 FTS Actor
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let fts_tx = spawn_fts_actor(fts_index);

        // 创建 MemoryManager 并添加记忆
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        let id1 = shared_manager.add_memory("测试内容1", MemoryCategory::Rule).unwrap().unwrap();
        let id2 = shared_manager.add_memory("测试内容2", MemoryCategory::Pattern).unwrap().unwrap();

        // 手动同步到 FTS
        let entries = shared_manager.get_all_memories().unwrap();
        for entry in entries {
            fts_tx.send(FtsMessage::Sync(entry)).await.unwrap();
        }

        // 等待 FTS Actor 同步
        sleep(Duration::from_millis(200)).await;

        // 验证一致性
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();

        assert_eq!(report.json_count, 2);
        assert_eq!(report.fts_count, 2);
        assert!(report.missing_in_fts.is_empty());
        assert!(report.orphaned_in_fts.is_empty());
        assert!(report.is_consistent());
        assert_eq!(report.total_inconsistencies(), 0);

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }

    #[tokio::test]
    async fn test_verify_consistency_missing_in_fts() {
        // 场景：JSON 中有记忆，但 FTS 中缺失
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建独立的 FTS Actor（空索引）
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let fts_tx = spawn_fts_actor(fts_index);

        // 创建 MemoryManager 并添加记忆（不同步到 FTS）
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        let id1 = shared_manager.add_memory("测试内容1", MemoryCategory::Rule).unwrap().unwrap();
        let id2 = shared_manager.add_memory("测试内容2", MemoryCategory::Pattern).unwrap().unwrap();

        // 验证一致性
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();

        assert_eq!(report.json_count, 2);
        assert_eq!(report.fts_count, 0);
        assert_eq!(report.missing_in_fts.len(), 2);
        assert!(report.missing_in_fts.contains(&id1));
        assert!(report.missing_in_fts.contains(&id2));
        assert!(report.orphaned_in_fts.is_empty());
        assert!(!report.is_consistent());
        assert_eq!(report.total_inconsistencies(), 2);

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }

    #[tokio::test]
    async fn test_verify_consistency_orphaned_in_fts() {
        // 场景：FTS 中有索引，但 JSON 中不存在
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建 FTS Actor 并添加记忆
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let entry1 = make_entry("orphan-1", "孤立内容1", MemoryCategory::Rule);
        let entry2 = make_entry("orphan-2", "孤立内容2", MemoryCategory::Pattern);
        fts_index.sync_entry(&entry1).unwrap();
        fts_index.sync_entry(&entry2).unwrap();

        let fts_tx = spawn_fts_actor(fts_index);

        // 创建空的 MemoryManager
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        // 等待 FTS Actor 启动
        sleep(Duration::from_millis(100)).await;

        // 验证一致性
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();

        assert_eq!(report.json_count, 0);
        assert_eq!(report.fts_count, 2);
        assert!(report.missing_in_fts.is_empty());
        assert_eq!(report.orphaned_in_fts.len(), 2);
        assert!(report.orphaned_in_fts.contains(&"orphan-1".to_string()));
        assert!(report.orphaned_in_fts.contains(&"orphan-2".to_string()));
        assert!(!report.is_consistent());
        assert_eq!(report.total_inconsistencies(), 2);

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }

    #[tokio::test]
    async fn test_repair_consistency_add_missing() {
        // 场景：修复 FTS 缺失项
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建独立的 FTS Actor（空索引）
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let fts_tx = spawn_fts_actor(fts_index);

        // 创建 MemoryManager 并添加记忆（不同步到 FTS）
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        let id1 = shared_manager.add_memory("测试内容1", MemoryCategory::Rule).unwrap().unwrap();
        let id2 = shared_manager.add_memory("测试内容2", MemoryCategory::Pattern).unwrap().unwrap();

        // 验证一致性（应有缺失）
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report.missing_in_fts.len(), 2);

        // 修复一致性
        repair_consistency(&shared_manager, &fts_tx, &report).await.unwrap();

        // 等待 FTS Actor 处理
        sleep(Duration::from_millis(300)).await;

        // 再次验证（应已一致）
        let report2 = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report2.json_count, 2);
        assert_eq!(report2.fts_count, 2);
        assert!(report2.is_consistent());

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }

    #[tokio::test]
    async fn test_repair_consistency_remove_orphaned() {
        // 场景：修复 FTS 孤立项
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建 FTS Actor 并添加孤立记忆
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let entry1 = make_entry("orphan-1", "孤立内容1", MemoryCategory::Rule);
        let entry2 = make_entry("orphan-2", "孤立内容2", MemoryCategory::Pattern);
        fts_index.sync_entry(&entry1).unwrap();
        fts_index.sync_entry(&entry2).unwrap();

        let fts_tx = spawn_fts_actor(fts_index);

        // 创建空的 MemoryManager
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        sleep(Duration::from_millis(100)).await;

        // 验证一致性（应有孤立项）
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report.orphaned_in_fts.len(), 2);

        // 修复一致性
        repair_consistency(&shared_manager, &fts_tx, &report).await.unwrap();

        // 等待 FTS Actor 处理
        sleep(Duration::from_millis(300)).await;

        // 再次验证（应已一致）
        let report2 = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report2.json_count, 0);
        assert_eq!(report2.fts_count, 0);
        assert!(report2.is_consistent());

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }

    #[tokio::test]
    async fn test_repair_consistency_mixed_issues() {
        // 场景：同时存在缺失项和孤立项
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // 创建独立的 FTS Actor 并添加孤立记忆
        let fts_index = FtsIndex::open(temp.path()).unwrap();
        let orphan = make_entry("orphan-1", "孤立内容", MemoryCategory::Rule);
        fts_index.sync_entry(&orphan).unwrap();

        let fts_tx = spawn_fts_actor(fts_index);

        // 创建 MemoryManager 并添加记忆（不同步到 FTS）
        let manager = MemoryManager::new(temp.path().to_str().unwrap()).unwrap();
        let shared_manager = SharedMemoryManager::from_arc(
            std::sync::Arc::new(std::sync::RwLock::new(manager))
        );

        let id1 = shared_manager.add_memory("测试内容1", MemoryCategory::Rule).unwrap().unwrap();

        sleep(Duration::from_millis(100)).await;

        // 验证一致性（应有 1 个缺失 + 1 个孤立）
        let report = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report.json_count, 1);
        assert_eq!(report.fts_count, 1);
        assert_eq!(report.missing_in_fts.len(), 1);
        assert_eq!(report.orphaned_in_fts.len(), 1);
        assert_eq!(report.total_inconsistencies(), 2);

        // 修复一致性
        repair_consistency(&shared_manager, &fts_tx, &report).await.unwrap();

        // 等待 FTS Actor 处理
        sleep(Duration::from_millis(300)).await;

        // 再次验证（应已一致）
        let report2 = verify_consistency(&shared_manager, &fts_tx).await.unwrap();
        assert_eq!(report2.json_count, 1);
        assert_eq!(report2.fts_count, 1);
        assert!(report2.is_consistent());

        // 清理
        fts_tx.send(FtsMessage::Shutdown).await.ok();
    }
}
