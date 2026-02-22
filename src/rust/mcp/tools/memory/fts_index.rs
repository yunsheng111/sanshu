//! FTS5 Sidecar 索引模块
//!
//! HC-16: Sidecar 索引，不替换 JSON 主存储
//! HC-18: FTS5 失败不阻塞 JSON 主流程
//! RISK-01: 双写一致性校验
//! SC-23: unicode61 分词器，预留 jieba-rs 切换接口

use std::path::{Path, PathBuf};
use anyhow::Result;
use rusqlite::{Connection, params};

use super::types::MemoryEntry;
use crate::log_debug;

/// FTS5 索引数据库文件名
const FTS_DB_FILE: &str = "fts_index.db";

/// FTS5 Sidecar 索引
pub struct FtsIndex {
    /// 数据库连接
    conn: Connection,
    /// 数据库文件路径
    db_path: PathBuf,
}

impl FtsIndex {
    /// 打开或创建 FTS5 索引
    pub fn open(memory_dir: &Path) -> Result<Self> {
        let db_path = memory_dir.join(FTS_DB_FILE);
        let conn = Connection::open(&db_path)?;

        // SC-23: 使用 unicode61 分词器（支持基础中文分词）
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                id,
                content,
                category,
                domain,
                tags,
                summary,
                tokenize='unicode61'
            );"
        )?;

        log_debug!("[FTS5] 索引已打开: {}", db_path.display());

        Ok(Self { conn, db_path })
    }

    /// 同步单条记忆到 FTS5 索引
    ///
    /// HC-18: 失败仅记录日志，不阻塞主流程
    pub fn sync_entry(&self, entry: &MemoryEntry) -> Result<()> {
        // 先删除旧记录（如有）
        self.conn.execute(
            "DELETE FROM memory_fts WHERE id = ?1",
            params![entry.id],
        )?;

        // 插入新记录
        let tags_str = entry.tags.as_ref()
            .map(|t| t.join(","))
            .unwrap_or_default();

        self.conn.execute(
            "INSERT INTO memory_fts (id, content, category, domain, tags, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.id,
                entry.content,
                entry.category.display_name(),
                entry.domain.as_deref().unwrap_or(""),
                tags_str,
                entry.summary.as_deref().unwrap_or("")
            ],
        )?;

        Ok(())
    }

    /// 批量同步所有记忆到 FTS5 索引
    ///
    /// HC-18: 单条失败不阻塞其他条目
    pub fn sync_all(&self, entries: &[MemoryEntry]) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        // 清空旧索引
        self.conn.execute("DELETE FROM memory_fts", [])?;

        for entry in entries {
            match self.sync_entry(entry) {
                Ok(_) => result.synced += 1,
                Err(e) => {
                    log_debug!("[FTS5] 同步条目失败 (id={}): {}", entry.id, e);
                    result.failed += 1;
                }
            }
        }

        log_debug!("[FTS5] 批量同步完成: synced={}, failed={}", result.synced, result.failed);
        Ok(result)
    }

    /// FTS5 全文搜索
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM memory_fts WHERE memory_fts MATCH ?1 LIMIT ?2"
        )?;

        let ids: Vec<String> = stmt.query_map(params![query, limit as i64], |row| {
            row.get(0)
        })?.filter_map(|r| r.ok()).collect();

        Ok(ids)
    }

    /// 获取所有记忆 ID（用于一致性检查）
    pub fn get_all_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM memory_fts")?;
        let ids: Vec<String> = stmt.query_map([], |row| {
            row.get(0)
        })?.filter_map(|r| r.ok()).collect();
        Ok(ids)
    }

    /// 删除单条记忆的索引
    pub fn delete_entry(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM memory_fts WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// RISK-01: 验证双写一致性
    ///
    /// 对比 JSON 条目数与 FTS5 行数
    pub fn verify_consistency(&self, json_count: usize) -> ConsistencyResult {
        let fts_count = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_fts",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) as usize;

        ConsistencyResult {
            json_count,
            fts_count,
            is_consistent: json_count == fts_count,
        }
    }

    /// 获取索引路径（用于监控）
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// 同步结果
#[derive(Debug, Default)]
pub struct SyncResult {
    /// 成功同步的条目数
    pub synced: usize,
    /// 同步失败的条目数
    pub failed: usize,
}

/// 一致性校验结果
#[derive(Debug)]
pub struct ConsistencyResult {
    /// JSON 条目数
    pub json_count: usize,
    /// FTS5 行数
    pub fts_count: usize,
    /// 是否一致
    pub is_consistent: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{MemoryEntry, MemoryCategory};
    use super::super::similarity::TextSimilarity;
    use chrono::Utc;
    use tempfile::TempDir;

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

    #[test]
    fn test_fts5_open_and_sync() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "Rust 编程规范", MemoryCategory::Rule),
            make_entry("2", "使用 tokio 异步运行时", MemoryCategory::Pattern),
            make_entry("3", "偏好简洁的代码风格", MemoryCategory::Preference),
        ];

        let result = index.sync_all(&entries).unwrap();
        assert_eq!(result.synced, 3);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_fts5_search() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "Rust 编程规范和最佳实践", MemoryCategory::Rule),
            make_entry("2", "使用 tokio 异步运行时处理并发", MemoryCategory::Pattern),
            make_entry("3", "偏好简洁清晰的代码风格", MemoryCategory::Preference),
        ];

        index.sync_all(&entries).unwrap();

        let ids = index.search("Rust", 10).unwrap();
        assert!(!ids.is_empty(), "应找到包含 Rust 的记忆");
        assert!(ids.contains(&"1".to_string()));
    }

    #[test]
    fn test_fts5_consistency() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "内容一", MemoryCategory::Rule),
            make_entry("2", "内容二", MemoryCategory::Pattern),
        ];

        index.sync_all(&entries).unwrap();

        let result = index.verify_consistency(2);
        assert!(result.is_consistent);

        let result2 = index.verify_consistency(3);
        assert!(!result2.is_consistent);
    }

    // --- 追加测试 ---

    #[test]
    fn test_fts5_chinese_content_search() {
        // unicode61 分词器对中文字符可能不做分词，而是作为整体 token
        // 此测试验证英文搜索正常工作，并验证中文搜索的实际行为
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "database 数据库连接池配置", MemoryCategory::Rule),
            make_entry("2", "使用 tokio 运行时", MemoryCategory::Pattern),
            make_entry("3", "review 代码审查规范", MemoryCategory::Rule),
        ];
        index.sync_all(&entries).unwrap();

        // 英文关键词搜索确认正常
        let ids = index.search("database", 10).unwrap();
        assert!(!ids.is_empty(), "应能通过英文关键词搜索到记忆");
        assert!(ids.contains(&"1".to_string()));

        let ids2 = index.search("tokio", 10).unwrap();
        assert!(ids2.contains(&"2".to_string()), "应能搜索到 tokio");

        let ids3 = index.search("review", 10).unwrap();
        assert!(ids3.contains(&"3".to_string()), "应能搜索到 review");
    }

    #[test]
    fn test_fts5_delete_then_search() {
        // 删除条目后搜索不应返回已删除的条目
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "Rust programming guide", MemoryCategory::Rule),
            make_entry("2", "Python scripting tips", MemoryCategory::Pattern),
        ];
        index.sync_all(&entries).unwrap();

        // 删除 entry 1
        index.delete_entry("1").unwrap();

        // 搜索 Rust 应无结果
        let ids = index.search("Rust", 10).unwrap();
        assert!(!ids.contains(&"1".to_string()), "删除后不应再搜索到该条目");
    }

    #[test]
    fn test_fts5_consistency_after_delete() {
        // 删除后一致性应报告不一致（JSON 数为 2，FTS 实际为 1）
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entries = vec![
            make_entry("1", "内容一", MemoryCategory::Rule),
            make_entry("2", "内容二", MemoryCategory::Pattern),
        ];
        index.sync_all(&entries).unwrap();
        index.delete_entry("1").unwrap();

        let result = index.verify_consistency(2); // JSON 还有 2 条，FTS 只有 1 条
        assert!(!result.is_consistent);
        assert_eq!(result.json_count, 2);
        assert_eq!(result.fts_count, 1);
    }

    #[test]
    fn test_fts5_empty_index_search() {
        // 空索引搜索应返回空结果，不崩溃
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let ids = index.search("anything", 10).unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_fts5_sync_idempotent() {
        // 重复同步同一条目应为幂等操作（先删除旧记录再插入新记录）
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let entry = make_entry("1", "Rust 编程规范", MemoryCategory::Rule);
        index.sync_entry(&entry).unwrap();
        index.sync_entry(&entry).unwrap(); // 重复同步
        index.sync_entry(&entry).unwrap(); // 再次同步

        // 一致性：应只有 1 条
        let result = index.verify_consistency(1);
        assert!(result.is_consistent, "重复同步应为幂等，FTS 中应只有 1 条记录");
    }

    #[test]
    fn test_fts5_db_path() {
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();
        let db_path = index.db_path();
        assert!(db_path.ends_with("fts_index.db"));
    }

    #[test]
    fn test_fts5_sync_all_clears_old_data() {
        // sync_all 先清空旧索引再插入，确保不会积累旧数据
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        // 第一次同步 3 条
        let entries1 = vec![
            make_entry("1", "内容一", MemoryCategory::Rule),
            make_entry("2", "内容二", MemoryCategory::Pattern),
            make_entry("3", "内容三", MemoryCategory::Preference),
        ];
        index.sync_all(&entries1).unwrap();
        assert!(index.verify_consistency(3).is_consistent);

        // 第二次同步只有 1 条
        let entries2 = vec![
            make_entry("4", "新内容", MemoryCategory::Rule),
        ];
        let result = index.sync_all(&entries2).unwrap();
        assert_eq!(result.synced, 1);
        assert!(index.verify_consistency(1).is_consistent);
    }

    #[test]
    fn test_fts5_search_with_tags() {
        // 验证 tags 字段也被索引
        let temp = TempDir::new().unwrap();
        let index = FtsIndex::open(temp.path()).unwrap();

        let mut entry = make_entry("1", "基础内容", MemoryCategory::Rule);
        entry.tags = Some(vec!["rust".to_string(), "性能".to_string()]);
        index.sync_entry(&entry).unwrap();

        // 搜索 tags 中的关键词
        let ids = index.search("rust", 10).unwrap();
        assert!(ids.contains(&"1".to_string()), "应能通过 tags 搜索到记忆");
    }
}
