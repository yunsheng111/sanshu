//! 会话工具观察存储模块
//!
//! SC-25: 会话自动捕获工具调用的输入/输出摘要
//! DEP-07: 依赖 P0 Registry + P2 FTS5
//! RISK-10: 噪音控制（可配置跳过列表 + FIFO 淘汰）

use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use tokio::sync::mpsc;

use crate::log_debug;

/// 观察数据库文件名
const OBSERVATIONS_DB_FILE: &str = "observations.db";

/// 默认最大观察条数（FIFO 淘汰）
const DEFAULT_MAX_OBSERVATIONS: usize = 5000;

/// RISK-10: 默认跳过列表（高频只读工具）
const DEFAULT_SKIP_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Bash"];

/// 工具调用观察记录
#[derive(Debug, Clone)]
pub struct Observation {
    /// 唯一标识符
    pub id: String,
    /// 工具名称
    pub tool_name: String,
    /// 输入摘要（截断到 200 字符）
    pub input_summary: String,
    /// 输出摘要（截断到 200 字符）
    pub output_summary: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 标签
    pub tags: Vec<String>,
}

/// 异步观察写入消息
#[derive(Debug)]
enum ObservationMessage {
    /// 记录一条新观察
    Record(Observation),
}

/// 会话观察存储
pub struct ObservationStore {
    /// 数据库连接
    conn: Connection,
    /// 数据库文件路径
    db_path: PathBuf,
    /// 最大观察条数
    max_observations: usize,
    /// 跳过列表
    skip_tools: Vec<String>,
}

impl ObservationStore {
    /// 打开或创建观察存储
    pub fn open(memory_dir: &Path) -> Result<Self> {
        let db_path = memory_dir.join(OBSERVATIONS_DB_FILE);
        let conn = Connection::open(&db_path)?;

        // 创建观察表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS observations (
                id TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                input_summary TEXT NOT NULL,
                output_summary TEXT NOT NULL,
                created_at TEXT NOT NULL,
                tags TEXT DEFAULT ''
            );"
        )?;

        log_debug!("[ObservationStore] 已打开: {}", db_path.display());

        Ok(Self {
            conn,
            db_path,
            max_observations: DEFAULT_MAX_OBSERVATIONS,
            skip_tools: DEFAULT_SKIP_TOOLS.iter().map(|s| s.to_string()).collect(),
        })
    }

    /// RISK-10: 检查工具是否应跳过
    pub fn should_skip(&self, tool_name: &str) -> bool {
        self.skip_tools.iter().any(|s| s.eq_ignore_ascii_case(tool_name))
    }

    /// 记录一条观察
    pub fn record(&self, observation: &Observation) -> Result<()> {
        // RISK-10: 跳过列表检查
        if self.should_skip(&observation.tool_name) {
            log_debug!("[ObservationStore] 跳过: tool={}", observation.tool_name);
            return Ok(());
        }

        let tags_str = observation.tags.join(",");

        self.conn.execute(
            "INSERT OR REPLACE INTO observations (id, tool_name, input_summary, output_summary, created_at, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                observation.id,
                observation.tool_name,
                observation.input_summary,
                observation.output_summary,
                observation.created_at.to_rfc3339(),
                tags_str
            ],
        )?;

        // FIFO 淘汰：超出上限时删除最旧记录
        self.enforce_limit()?;

        Ok(())
    }

    /// FIFO 淘汰：保持观察条数不超过上限
    fn enforce_limit(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM observations",
            [],
            |row| row.get(0),
        )?;

        if count as usize > self.max_observations {
            let excess = count as usize - self.max_observations;
            self.conn.execute(
                "DELETE FROM observations WHERE id IN (
                    SELECT id FROM observations ORDER BY created_at ASC LIMIT ?1
                )",
                params![excess as i64],
            )?;
            log_debug!("[ObservationStore] FIFO 淘汰: removed={}", excess);
        }

        Ok(())
    }

    /// 查询最近的观察记录
    pub fn recent(&self, limit: usize) -> Result<Vec<Observation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tool_name, input_summary, output_summary, created_at, tags
             FROM observations ORDER BY created_at DESC LIMIT ?1"
        )?;

        let observations = stmt.query_map(params![limit as i64], |row| {
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> = if tags_str.is_empty() {
                Vec::new()
            } else {
                tags_str.split(',').map(|s| s.to_string()).collect()
            };

            Ok(Observation {
                id: row.get(0)?,
                tool_name: row.get(1)?,
                input_summary: row.get(2)?,
                output_summary: row.get(3)?,
                created_at: {
                    let s: String = row.get(4)?;
                    s.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now())
                },
                tags,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(observations)
    }

    /// 获取观察总数
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM observations",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// 获取数据库路径
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// 截断文本到指定长度
    pub fn truncate_summary(text: &str, max_len: usize) -> String {
        let trimmed = text.trim();
        if trimmed.len() <= max_len {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(max_len).collect();
            format!("{}...", truncated)
        }
    }
}

/// 异步观察写入器
///
/// SC-25: 异步写入不阻塞 MCP 主流程
pub struct AsyncObservationWriter {
    /// 发送端
    tx: mpsc::UnboundedSender<ObservationMessage>,
}

impl AsyncObservationWriter {
    /// 创建异步写入器和后台任务
    pub fn new(store: ObservationStore) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<ObservationMessage>();

        // 后台写入任务
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ObservationMessage::Record(obs) => {
                        if let Err(e) = store.record(&obs) {
                            log_debug!("[ObservationStore] 异步写入失败: {}", e);
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    /// 异步记录观察（不阻塞）
    pub fn record_async(&self, observation: Observation) {
        if let Err(e) = self.tx.send(ObservationMessage::Record(observation)) {
            log_debug!("[ObservationStore] 异步发送失败: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_observation(tool_name: &str, input: &str, output: &str) -> Observation {
        Observation {
            id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            input_summary: input.to_string(),
            output_summary: output.to_string(),
            created_at: Utc::now(),
            tags: vec!["test".to_string()],
        }
    }

    #[test]
    fn test_observation_store_open_and_record() {
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        let obs = make_observation("ji", "记忆操作", "成功");
        store.record(&obs).unwrap();

        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn test_skip_tools() {
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        // Read 在跳过列表中
        let obs = make_observation("Read", "读取文件", "文件内容");
        store.record(&obs).unwrap();
        assert_eq!(store.count().unwrap(), 0);

        // ji 不在跳过列表中
        let obs2 = make_observation("ji", "记忆操作", "成功");
        store.record(&obs2).unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn test_recent_query() {
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        for i in 0..5 {
            let obs = make_observation("ji", &format!("输入{}", i), &format!("输出{}", i));
            store.record(&obs).unwrap();
        }

        let recent = store.recent(3).unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_truncate_summary() {
        let short = "短文本";
        assert_eq!(ObservationStore::truncate_summary(short, 200), "短文本");

        let long = "A".repeat(300);
        let truncated = ObservationStore::truncate_summary(&long, 200);
        assert!(truncated.ends_with("..."));
        // 200 chars + "..." = 203
        assert!(truncated.len() <= 210);
    }

    // --- 追加测试 ---

    #[test]
    fn test_fifo_eviction() {
        // FIFO 淘汰：超出上限后最旧的记录被删除
        let temp = TempDir::new().unwrap();
        let mut store = ObservationStore::open(temp.path()).unwrap();
        store.max_observations = 3; // 设置较小的上限便于测试

        // 写入 5 条，前 2 条应被淘汰
        for i in 0..5 {
            let obs = make_observation("ji", &format!("输入{}", i), &format!("输出{}", i));
            store.record(&obs).unwrap();
        }

        let count = store.count().unwrap();
        assert_eq!(count, 3, "超出上限后应只保留最新的 3 条");

        // 最近的 3 条应保留
        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_skip_all_default_tools() {
        // 验证所有默认跳过列表中的工具都被跳过
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        let default_skip = vec!["Read", "Glob", "Grep", "Bash"];
        for tool in &default_skip {
            assert!(store.should_skip(tool), "工具 {} 应被跳过", tool);
        }
    }

    #[test]
    fn test_skip_case_insensitive() {
        // 跳过列表应不区分大小写
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        assert!(store.should_skip("read"), "'read' 应被跳过（大小写不敏感）");
        assert!(store.should_skip("READ"), "'READ' 应被跳过（大小写不敏感）");
        assert!(store.should_skip("Read"), "'Read' 应被跳过");
    }

    #[test]
    fn test_non_skip_tools_recorded() {
        // 不在跳过列表中的工具应正常记录
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        assert!(!store.should_skip("ji"), "ji 不应被跳过");
        assert!(!store.should_skip("enhance"), "enhance 不应被跳过");
        assert!(!store.should_skip("sou"), "sou 不应被跳过");
    }

    #[test]
    fn test_observation_with_empty_tags() {
        // 空标签的观察应正常记录和查询
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        let mut obs = make_observation("ji", "测试输入", "测试输出");
        obs.tags = Vec::new(); // 空标签
        store.record(&obs).unwrap();

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert!(recent[0].tags.is_empty());
    }

    #[test]
    fn test_observation_count_after_multiple_writes() {
        // 批量写入后计数正确
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        for i in 0..10 {
            let obs = make_observation("enhance", &format!("输入{}", i), &format!("输出{}", i));
            store.record(&obs).unwrap();
        }

        assert_eq!(store.count().unwrap(), 10);
    }

    #[test]
    fn test_db_path() {
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();
        assert!(store.db_path().ends_with("observations.db"));
    }

    #[test]
    fn test_recent_order_desc() {
        // recent() 应按时间降序返回
        let temp = TempDir::new().unwrap();
        let store = ObservationStore::open(temp.path()).unwrap();

        for i in 0..3 {
            let mut obs = make_observation("ji", &format!("输入{}", i), &format!("输出{}", i));
            // 手动设置不同时间确保顺序
            obs.created_at = Utc::now() + chrono::Duration::seconds(i as i64);
            store.record(&obs).unwrap();
        }

        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 3);
        // 最新的应在第一个位置（降序）
        assert!(recent[0].created_at >= recent[1].created_at);
        assert!(recent[1].created_at >= recent[2].created_at);
    }

    #[test]
    fn test_truncate_summary_exact_boundary() {
        // 恰好在边界处的文本不应被截断
        let exact = "A".repeat(200);
        let result = ObservationStore::truncate_summary(&exact, 200);
        assert!(!result.ends_with("..."), "恰好 200 字符不应被截断");
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn test_truncate_summary_empty() {
        let result = ObservationStore::truncate_summary("", 200);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_summary_whitespace() {
        // 带空白前后缀的文本应被 trim
        let result = ObservationStore::truncate_summary("  hello  ", 200);
        assert_eq!(result, "hello");
    }
}
