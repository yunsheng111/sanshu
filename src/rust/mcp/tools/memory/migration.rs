//! 记忆格式迁移模块
//!
//! 将旧版 Markdown 格式迁移到新版 JSON 格式

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::Utc;

use super::types::{MemoryEntry, MemoryCategory, MemoryStore, MemoryConfig};
use super::dedup::MemoryDeduplicator;
use super::similarity::TextSimilarity;
use crate::log_debug;

/// 迁移结果统计
#[derive(Debug, Clone, Default)]
pub struct MigrationResult {
    /// 是否执行了迁移
    pub migrated: bool,
    /// 从 MD 文件读取的条目数
    pub md_entries_count: usize,
    /// 去重后的条目数
    pub deduped_entries_count: usize,
    /// 移除的重复条目数
    pub removed_duplicates: usize,
    /// 备份的文件列表
    pub backed_up_files: Vec<String>,
}

/// 记忆格式迁移器
pub struct MemoryMigrator;

impl MemoryMigrator {
    /// 旧版 MD 文件名列表
    const OLD_MD_FILES: [(&'static str, MemoryCategory); 4] = [
        ("rules.md", MemoryCategory::Rule),
        ("preferences.md", MemoryCategory::Preference),
        ("patterns.md", MemoryCategory::Pattern),
        ("context.md", MemoryCategory::Context),
    ];

    /// 新版存储文件名
    pub const STORE_FILE: &'static str = "memories.json";

    /// 备份目录名
    const BACKUP_DIR: &'static str = "backup";

    /// 检查是否需要迁移
    ///
    /// 如果存在旧版 MD 文件且不存在新版 JSON 文件，则需要迁移
    pub fn needs_migration(memory_dir: &Path) -> bool {
        let store_path = memory_dir.join(Self::STORE_FILE);

        // 如果已存在新版文件，不需要迁移
        if store_path.exists() {
            return false;
        }

        // 检查是否存在任何旧版 MD 文件
        for (filename, _) in &Self::OLD_MD_FILES {
            let md_path = memory_dir.join(filename);
            if md_path.exists() {
                return true;
            }
        }

        false
    }

    /// 执行迁移
    ///
    /// 1. 读取所有旧版 MD 文件
    /// 2. 解析为 MemoryEntry
    /// 3. 执行去重
    /// 4. 写入新版 JSON 文件
    /// 5. 备份旧文件
    pub fn migrate(memory_dir: &Path, project_path: &str) -> Result<MigrationResult> {
        let mut result = MigrationResult::default();

        if !Self::needs_migration(memory_dir) {
            log_debug!("无需迁移：已存在 memories.json 或没有旧版 MD 文件");
            return Ok(result);
        }

        log_debug!("开始迁移记忆格式...");

        // 1. 读取所有旧版 MD 文件
        let mut all_entries: Vec<MemoryEntry> = Vec::new();

        for (filename, category) in &Self::OLD_MD_FILES {
            let md_path = memory_dir.join(filename);
            if md_path.exists() {
                let entries = Self::parse_md_file(&md_path, *category)?;
                all_entries.extend(entries);
            }
        }

        result.md_entries_count = all_entries.len();
        log_debug!("从 MD 文件读取了 {} 条记忆", result.md_entries_count);

        // 2. 执行去重
        let dedup = MemoryDeduplicator::default();
        let (deduped_entries, dedup_stats) = dedup.deduplicate(all_entries);

        result.deduped_entries_count = deduped_entries.len();
        result.removed_duplicates = dedup_stats.removed_count;
        log_debug!(
            "去重完成：移除 {} 条重复，保留 {} 条",
            result.removed_duplicates,
            result.deduped_entries_count
        );

        // 3. 创建新版存储结构（W7 修复：直接写入 v2.2）
        let mut store = MemoryStore {
            version: "2.2".to_string(),
            project_path: project_path.to_string(),
            entries: deduped_entries,
            last_dedup_at: Utc::now(),
            config: MemoryConfig::default(),
            domains: None,
        };

        // 执行 v2.2 字段填充（确保新字段有默认值）
        for entry in &mut store.entries {
            if entry.vitality_score.is_none() {
                entry.vitality_score = Some(1.5);
            }
            if entry.last_accessed_at.is_none() {
                entry.last_accessed_at = Some(Utc::now());
            }
        }

        // 4. 写入新版 JSON 文件
        let store_path = memory_dir.join(Self::STORE_FILE);
        let json = serde_json::to_string_pretty(&store)?;
        fs::write(&store_path, json)?;
        log_debug!("已写入新版存储文件: {}", store_path.display());

        // 5. 备份旧文件
        let backup_dir = memory_dir.join(Self::BACKUP_DIR);
        fs::create_dir_all(&backup_dir)?;

        for (filename, _) in &Self::OLD_MD_FILES {
            let md_path = memory_dir.join(filename);
            if md_path.exists() {
                let backup_path = backup_dir.join(filename);
                fs::rename(&md_path, &backup_path)?;
                result.backed_up_files.push(filename.to_string());
                log_debug!("已备份: {} -> {}", filename, backup_path.display());
            }
        }

        // 同时备份旧版 metadata.json（如果存在）
        let old_metadata = memory_dir.join("metadata.json");
        if old_metadata.exists() {
            let backup_metadata = backup_dir.join("metadata.json.bak");
            fs::rename(&old_metadata, &backup_metadata)?;
            result.backed_up_files.push("metadata.json".to_string());
        }

        result.migrated = true;
        log_debug!("迁移完成！");

        Ok(result)
    }

    /// 解析旧版 MD 文件
    fn parse_md_file(path: &Path, category: MemoryCategory) -> Result<Vec<MemoryEntry>> {
        let content = fs::read_to_string(path)?;
        let mut entries = Vec::new();

        // 按列表项解析，每个 "- " 开头的行是一个记忆条目
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("- ") && line.len() > 2 {
                let content = line[2..].trim(); // 去掉 "- " 前缀
                if !content.is_empty() {
                    let entry = MemoryEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        content: content.to_string(),
                        content_normalized: TextSimilarity::normalize(content),
                        category,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        version: 1,
                        snapshots: Vec::new(),
                        uri_path: None,
                        domain: None,
                        tags: None,
                        vitality_score: Some(1.5),
                        last_accessed_at: Some(Utc::now()),
                        summary: None,
                    };
                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }

    /// 获取备份目录路径
    pub fn backup_dir(memory_dir: &Path) -> PathBuf {
        memory_dir.join(Self::BACKUP_DIR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_needs_migration() {
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path();

        // 空目录不需要迁移
        assert!(!MemoryMigrator::needs_migration(memory_dir));

        // 创建旧版 MD 文件
        fs::write(memory_dir.join("rules.md"), "# 规则\n- 规则1\n- 规则2\n").unwrap();
        assert!(MemoryMigrator::needs_migration(memory_dir));

        // 创建新版 JSON 文件后不需要迁移
        fs::write(memory_dir.join("memories.json"), "{}").unwrap();
        assert!(!MemoryMigrator::needs_migration(memory_dir));
    }

    #[test]
    fn test_parse_md_file() {
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let content = r#"# 测试规则

- 规则一
- 规则二
- 规则三

这是一些说明文字，不会被解析。
"#;
        fs::write(&md_path, content).unwrap();

        let entries = MemoryMigrator::parse_md_file(&md_path, MemoryCategory::Rule).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].content, "规则一");
        assert_eq!(entries[1].content, "规则二");
        assert_eq!(entries[2].content, "规则三");
    }

    #[test]
    fn test_v21_to_v22_upgrade() {
        use super::super::types::{MemoryStore, MemoryConfig, MemoryEntry, MemoryCategory};
        use chrono::Utc;

        let now = Utc::now();
        let entry = MemoryEntry {
            id: "test-1".to_string(),
            content: "测试内容".to_string(),
            content_normalized: "测试内容".to_string(),
            category: MemoryCategory::Rule,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            // v2.2 字段初始为 None（模拟 serde 反序列化 v2.1 数据）
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: None,
            last_accessed_at: None,
            summary: None,
        };

        let mut store = MemoryStore {
            version: "2.1".to_string(),
            project_path: "/test".to_string(),
            entries: vec![entry],
            last_dedup_at: now,
            config: MemoryConfig::default(),
            domains: None,
        };

        let result = store.upgrade_to_current();
        assert!(result.is_ok());
        assert_eq!(store.version, "2.2");
        assert_eq!(store.entries[0].vitality_score, Some(1.5));
        assert!(store.entries[0].last_accessed_at.is_some());
    }

    // --- 追加测试 ---

    #[test]
    fn test_v20_to_v22_chain_upgrade() {
        // v2.0 -> v2.1 -> v2.2 链式升级
        use super::super::types::{MemoryStore, MemoryConfig, MemoryEntry, MemoryCategory};
        use chrono::Utc;

        let now = Utc::now();
        let entry = MemoryEntry {
            id: "chain-test".to_string(),
            content: "链式升级测试".to_string(),
            content_normalized: String::new(), // v2.0 可能没有归一化内容
            category: MemoryCategory::Pattern,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: None,
            last_accessed_at: None,
            summary: None,
        };

        let mut store = MemoryStore {
            version: "2.0".to_string(),
            project_path: "/test".to_string(),
            entries: vec![entry],
            last_dedup_at: now,
            config: MemoryConfig::default(),
            domains: None,
        };

        let result = store.upgrade_to_current();
        assert!(result.is_ok(), "v2.0 -> v2.2 链式升级应成功");
        assert_eq!(store.version, "2.2");
        // v2.0 -> v2.1 应填充 content_normalized
        assert!(!store.entries[0].content_normalized.is_empty(), "升级后 content_normalized 不应为空");
        // v2.1 -> v2.2 应填充 vitality_score 和 last_accessed_at
        assert_eq!(store.entries[0].vitality_score, Some(1.5));
        assert!(store.entries[0].last_accessed_at.is_some());
    }

    #[test]
    fn test_v10_to_v22_chain_upgrade() {
        // v1.0 -> v2.0 -> v2.1 -> v2.2 全链路升级
        use super::super::types::{MemoryStore, MemoryConfig, MemoryEntry, MemoryCategory};
        use chrono::Utc;

        let now = Utc::now();
        let entry = MemoryEntry {
            id: "full-chain".to_string(),
            content: "完整链路升级".to_string(),
            content_normalized: String::new(),
            category: MemoryCategory::Context,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: None,
            last_accessed_at: None,
            summary: None,
        };

        let mut store = MemoryStore {
            version: "1.0".to_string(),
            project_path: "/test".to_string(),
            entries: vec![entry],
            last_dedup_at: now,
            config: MemoryConfig::default(),
            domains: None,
        };

        let result = store.upgrade_to_current();
        assert!(result.is_ok(), "v1.0 -> v2.2 全链路升级应成功");
        assert_eq!(store.version, "2.2");
        assert!(!store.entries[0].content_normalized.is_empty());
        assert_eq!(store.entries[0].vitality_score, Some(1.5));
        assert!(store.entries[0].last_accessed_at.is_some());
    }

    #[test]
    fn test_unknown_version_incompatible() {
        // 未知版本应返回错误
        use super::super::types::{MemoryStore, MemoryConfig};
        use chrono::Utc;

        let mut store = MemoryStore {
            version: "99.0".to_string(),
            project_path: "/test".to_string(),
            entries: Vec::new(),
            last_dedup_at: Utc::now(),
            config: MemoryConfig::default(),
            domains: None,
        };

        let result = store.upgrade_to_current();
        assert!(result.is_err(), "未知版本升级应返回错误");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("不兼容"), "错误信息应包含'不兼容': {}", err_msg);
    }

    #[test]
    fn test_current_version_no_upgrade_needed() {
        // 当前版本不需要升级
        use super::super::types::{MemoryStore, MemoryConfig};
        use chrono::Utc;

        let mut store = MemoryStore {
            version: "2.2".to_string(),
            project_path: "/test".to_string(),
            entries: Vec::new(),
            last_dedup_at: Utc::now(),
            config: MemoryConfig::default(),
            domains: None,
        };

        let result = store.upgrade_to_current();
        assert!(result.is_ok());
        assert_eq!(store.version, "2.2", "版本号不应改变");
    }

    #[test]
    fn test_parse_md_empty_lines_skipped() {
        // 空行和非列表行应被跳过
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("test.md");

        let content = r#"# 标题

- 有效条目一

这不是列表

- 有效条目二
-
- 有效条目三
"#;
        fs::write(&md_path, content).unwrap();

        let entries = MemoryMigrator::parse_md_file(&md_path, MemoryCategory::Rule).unwrap();
        assert_eq!(entries.len(), 3, "应解析出 3 条有效条目（跳过空行和非列表行）");
        assert_eq!(entries[0].content, "有效条目一");
        assert_eq!(entries[1].content, "有效条目二");
        assert_eq!(entries[2].content, "有效条目三");
    }

    #[test]
    fn test_parse_md_empty_file() {
        // 空 MD 文件应返回空列表，不应崩溃
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("empty.md");
        fs::write(&md_path, "").unwrap();

        let entries = MemoryMigrator::parse_md_file(&md_path, MemoryCategory::Preference).unwrap();
        assert!(entries.is_empty(), "空文件应返回空列表");
    }

    #[test]
    fn test_parse_md_no_list_items() {
        // 没有列表项的 MD 文件应返回空列表
        let temp_dir = TempDir::new().unwrap();
        let md_path = temp_dir.path().join("no_items.md");
        fs::write(&md_path, "# 只有标题\n\n一些普通文本\n").unwrap();

        let entries = MemoryMigrator::parse_md_file(&md_path, MemoryCategory::Context).unwrap();
        assert!(entries.is_empty(), "没有列表项应返回空列表");
    }

    #[test]
    fn test_migrate_full_workflow() {
        // 完整迁移流程：创建 MD 文件 -> 迁移 -> 验证结果
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path();

        // 创建旧版 MD 文件
        fs::write(memory_dir.join("rules.md"), "# 规则\n- 规则A\n- 规则B\n").unwrap();
        fs::write(memory_dir.join("preferences.md"), "# 偏好\n- 偏好一\n").unwrap();

        assert!(MemoryMigrator::needs_migration(memory_dir));

        let result = MemoryMigrator::migrate(memory_dir, "/test/project").unwrap();
        assert!(result.migrated, "应执行迁移");
        assert_eq!(result.md_entries_count, 3, "应读取 3 条 MD 条目");
        assert!(result.backed_up_files.contains(&"rules.md".to_string()));
        assert!(result.backed_up_files.contains(&"preferences.md".to_string()));

        // 迁移后新版文件应存在
        assert!(memory_dir.join("memories.json").exists(), "应生成 memories.json");
        // 旧文件应被备份
        assert!(memory_dir.join("backup/rules.md").exists(), "旧文件应备份");
        assert!(memory_dir.join("backup/preferences.md").exists());
        // 原始位置不应存在
        assert!(!memory_dir.join("rules.md").exists(), "原始 MD 文件应被移除");
    }

    #[test]
    fn test_migrate_skips_when_json_exists() {
        // 已存在 memories.json 时应跳过迁移
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path();

        fs::write(memory_dir.join("rules.md"), "# 规则\n- 规则1\n").unwrap();
        fs::write(memory_dir.join("memories.json"), "{}").unwrap();

        let result = MemoryMigrator::migrate(memory_dir, "/test").unwrap();
        assert!(!result.migrated, "已存在 JSON 时应跳过迁移");
        assert_eq!(result.md_entries_count, 0);
    }

    #[test]
    fn test_migrate_empty_directory() {
        // 空目录不需要迁移
        let temp_dir = TempDir::new().unwrap();
        let result = MemoryMigrator::migrate(temp_dir.path(), "/test").unwrap();
        assert!(!result.migrated);
        assert_eq!(result.md_entries_count, 0);
    }

    #[test]
    fn test_backup_dir_path() {
        let temp_dir = TempDir::new().unwrap();
        let backup = MemoryMigrator::backup_dir(temp_dir.path());
        assert!(backup.ends_with("backup"));
    }

    #[test]
    fn test_migration_result_default() {
        // 默认 MigrationResult 各字段应为零值
        let result = MigrationResult::default();
        assert!(!result.migrated);
        assert_eq!(result.md_entries_count, 0);
        assert_eq!(result.deduped_entries_count, 0);
        assert_eq!(result.removed_duplicates, 0);
        assert!(result.backed_up_files.is_empty());
    }

    #[test]
    fn test_v21_to_v22_preserves_existing_vitality() {
        // 已有 vitality_score 的条目在升级时不应被覆盖
        use super::super::types::{MemoryStore, MemoryConfig, MemoryEntry, MemoryCategory};
        use chrono::Utc;

        let now = Utc::now();
        let entry = MemoryEntry {
            id: "preserve-test".to_string(),
            content: "保留已有活力值".to_string(),
            content_normalized: "保留已有活力值".to_string(),
            category: MemoryCategory::Rule,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: Some(2.5), // 已有非默认值
            last_accessed_at: Some(now),
            summary: None,
        };

        let mut store = MemoryStore {
            version: "2.1".to_string(),
            project_path: "/test".to_string(),
            entries: vec![entry],
            last_dedup_at: now,
            config: MemoryConfig::default(),
            domains: None,
        };

        store.upgrade_to_current().unwrap();
        assert_eq!(store.entries[0].vitality_score, Some(2.5), "已有的活力值不应被覆盖");
        assert_eq!(store.entries[0].last_accessed_at, Some(now), "已有的 last_accessed_at 不应被覆盖");
    }

    #[test]
    fn test_v21_to_v22_multiple_entries() {
        // 多条目升级，混合有/无 vitality_score
        use super::super::types::{MemoryStore, MemoryConfig, MemoryEntry, MemoryCategory};
        use chrono::Utc;

        let now = Utc::now();
        let entries = vec![
            MemoryEntry {
                id: "entry-1".to_string(),
                content: "条目一".to_string(),
                content_normalized: "条目一".to_string(),
                category: MemoryCategory::Rule,
                created_at: now, updated_at: now, version: 1,
                snapshots: Vec::new(), uri_path: None, domain: None,
                tags: None, vitality_score: None, last_accessed_at: None, summary: None,
            },
            MemoryEntry {
                id: "entry-2".to_string(),
                content: "条目二".to_string(),
                content_normalized: "条目二".to_string(),
                category: MemoryCategory::Pattern,
                created_at: now, updated_at: now, version: 2,
                snapshots: Vec::new(), uri_path: None, domain: None,
                tags: None, vitality_score: Some(2.0), last_accessed_at: Some(now), summary: None,
            },
        ];

        let mut store = MemoryStore {
            version: "2.1".to_string(),
            project_path: "/multi".to_string(),
            entries,
            last_dedup_at: now,
            config: MemoryConfig::default(),
            domains: None,
        };

        store.upgrade_to_current().unwrap();
        // 条目一：None -> Some(1.5)
        assert_eq!(store.entries[0].vitality_score, Some(1.5));
        assert!(store.entries[0].last_accessed_at.is_some());
        // 条目二：保持 Some(2.0)
        assert_eq!(store.entries[1].vitality_score, Some(2.0));
    }
}
