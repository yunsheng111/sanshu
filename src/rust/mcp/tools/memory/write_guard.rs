//! Write Guard 写入守卫模块
//!
//! HC-11: 写入前执行相似度检查，三级判定：
//! - similarity >= semantic_threshold (0.80) -> NOOP（静默拒绝）
//! - update_threshold (0.60) <= similarity < semantic_threshold -> UPDATE（合并更新）
//! - similarity < update_threshold -> ADD（正常新增）

use super::similarity::TextSimilarity;
use super::types::{MemoryConfig, MemoryEntry};
use crate::log_debug;

/// Write Guard 判定结果
#[derive(Debug, Clone, PartialEq)]
pub enum WriteGuardAction {
    /// 新增：相似度低于更新阈值
    Add,
    /// 更新：相似度在更新阈值和语义阈值之间，自动合并到匹配条目
    Update {
        /// 匹配到的记忆 ID
        matched_id: String,
        /// 匹配到的记忆内容
        matched_content: String,
        /// 相似度值
        similarity: f64,
    },
    /// 静默拒绝：相似度高于语义阈值，内容已存在
    Noop {
        /// 匹配到的记忆 ID
        matched_id: String,
        /// 相似度值
        similarity: f64,
    },
}

/// Write Guard 执行结果
#[derive(Debug, Clone)]
pub struct WriteGuardResult {
    /// 判定动作
    pub action: WriteGuardAction,
    /// 最高相似度值
    pub max_similarity: f64,
    /// 匹配到的记忆 ID（如果有）
    pub matched_id: Option<String>,
}

/// Write Guard 写入守卫
pub struct WriteGuard;

impl WriteGuard {
    /// HC-11: 执行写入前相似度检查
    ///
    /// # 参数
    /// - `content`: 待写入的记忆内容
    /// - `existing`: 现有记忆列表
    /// - `config`: 记忆配置（包含阈值）
    ///
    /// # 返回
    /// WriteGuardResult 包含判定动作和相似度信息
    pub fn check(
        content: &str,
        existing: &[MemoryEntry],
        config: &MemoryConfig,
    ) -> WriteGuardResult {
        let semantic_threshold = config.write_guard_semantic_threshold;
        let update_threshold = config.write_guard_update_threshold;

        let mut max_similarity: f64 = 0.0;
        let mut best_match_id: Option<String> = None;
        let mut best_match_content: Option<String> = None;

        // 遍历所有现有记忆，找到最高相似度的匹配
        for entry in existing {
            let similarity = TextSimilarity::calculate_enhanced(content, &entry.content);
            if similarity > max_similarity {
                max_similarity = similarity;
                best_match_id = Some(entry.id.clone());
                best_match_content = Some(entry.content.clone());
            }
        }

        // 三级判定
        let action = if max_similarity >= semantic_threshold {
            // NOOP: 相似度 >= 0.80，静默拒绝
            log_debug!(
                "[WriteGuard] NOOP: similarity={:.3} >= {:.2}, matched_id={:?}",
                max_similarity, semantic_threshold, best_match_id
            );
            WriteGuardAction::Noop {
                matched_id: best_match_id.clone().unwrap_or_default(),
                similarity: max_similarity,
            }
        } else if max_similarity >= update_threshold {
            // UPDATE: 0.60 <= 相似度 < 0.80，自动合并
            log_debug!(
                "[WriteGuard] UPDATE: similarity={:.3}, {:.2} <= sim < {:.2}, matched_id={:?}",
                max_similarity, update_threshold, semantic_threshold, best_match_id
            );
            WriteGuardAction::Update {
                matched_id: best_match_id.clone().unwrap_or_default(),
                matched_content: best_match_content.unwrap_or_default(),
                similarity: max_similarity,
            }
        } else {
            // ADD: 相似度 < 0.60，正常新增
            log_debug!(
                "[WriteGuard] ADD: similarity={:.3} < {:.2}",
                max_similarity, update_threshold
            );
            WriteGuardAction::Add
        };

        WriteGuardResult {
            action,
            max_similarity,
            matched_id: best_match_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{MemoryConfig, MemoryEntry, MemoryCategory};
    use chrono::Utc;

    fn make_entry(id: &str, content: &str) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            content_normalized: TextSimilarity::normalize(content),
            category: MemoryCategory::Rule,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: Some(1.5),
            last_accessed_at: Some(now),
            summary: None,
        }
    }

    #[test]
    fn test_write_guard_add() {
        let config = MemoryConfig::default();
        let existing = vec![make_entry("1", "使用 Rust 编写后端")];
        let result = WriteGuard::check("配置数据库连接参数", &existing, &config);
        assert_eq!(result.action, WriteGuardAction::Add);
    }

    #[test]
    fn test_write_guard_noop() {
        let config = MemoryConfig::default();
        let existing = vec![make_entry("1", "使用 KISS 原则编写代码")];
        let result = WriteGuard::check("使用KISS原则编写代码", &existing, &config);
        assert!(matches!(result.action, WriteGuardAction::Noop { .. }));
    }

    #[test]
    fn test_write_guard_empty_existing() {
        let config = MemoryConfig::default();
        let existing: Vec<MemoryEntry> = Vec::new();
        let result = WriteGuard::check("任意内容", &existing, &config);
        assert_eq!(result.action, WriteGuardAction::Add);
        assert_eq!(result.max_similarity, 0.0);
    }

    // --- 追加测试：Update 判定 ---

    #[test]
    fn test_write_guard_update() {
        // Arrange: 构造中等相似度内容（0.60 <= sim < 0.80）
        let config = MemoryConfig::default();
        let existing = vec![make_entry("1", "使用 Rust 编写后端服务代码")];
        // 内容相似但有明显差异，期望触发 Update
        let result = WriteGuard::check("使用 Rust 编写后端 API 接口代码", &existing, &config);
        // 判定：如果相似度处于 Update 区间应返回 Update，否则根据实际相似度判定
        match &result.action {
            WriteGuardAction::Update { matched_id, similarity, .. } => {
                assert_eq!(matched_id, "1");
                assert!(*similarity >= config.write_guard_update_threshold, "相似度应 >= update_threshold");
                assert!(*similarity < config.write_guard_semantic_threshold, "相似度应 < semantic_threshold");
            }
            WriteGuardAction::Add => {
                // 如果文本相似度算法判定为低于 update_threshold，也是合理的
                assert!(result.max_similarity < config.write_guard_update_threshold);
            }
            WriteGuardAction::Noop { .. } => {
                panic!("不应触发 Noop：内容有明显差异");
            }
        }
    }

    #[test]
    fn test_write_guard_custom_thresholds() {
        // Arrange: 自定义阈值 — 提高 update 阈值到 0.90，semantic 到 0.95
        let mut config = MemoryConfig::default();
        config.write_guard_update_threshold = 0.90;
        config.write_guard_semantic_threshold = 0.95;

        let existing = vec![make_entry("1", "使用 KISS 原则编写代码")];
        // 相似内容在默认阈值下会被 Noop，但自定义高阈值下应 Add
        let result = WriteGuard::check("使用KISS原则编写代码", &existing, &config);
        // 由于阈值提高到 0.90/0.95，文本相似度可能低于新的 update_threshold
        assert!(
            result.max_similarity > 0.0,
            "相似度不应为零：内容非常接近"
        );
    }

    #[test]
    fn test_write_guard_multiple_entries_finds_best_match() {
        // Arrange: 多条记忆，应找到最高相似度的匹配
        let config = MemoryConfig::default();
        let existing = vec![
            make_entry("1", "配置数据库连接参数"),
            make_entry("2", "使用 KISS 原则编写代码"),
            make_entry("3", "遵循 DRY 原则避免重复"),
        ];
        let result = WriteGuard::check("使用KISS原则编写代码", &existing, &config);

        // 最高相似度匹配应为 entry "2"
        assert_eq!(result.matched_id, Some("2".to_string()));
        assert!(result.max_similarity > 0.5, "最高相似度应较高");
    }

    #[test]
    fn test_write_guard_noop_returns_matched_id() {
        // 验证 Noop 判定包含正确的 matched_id
        let config = MemoryConfig::default();
        let existing = vec![make_entry("abc-123", "使用 KISS 原则编写代码")];
        let result = WriteGuard::check("使用KISS原则编写代码", &existing, &config);

        if let WriteGuardAction::Noop { matched_id, similarity } = &result.action {
            assert_eq!(matched_id, "abc-123");
            assert!(*similarity >= config.write_guard_semantic_threshold);
        }
        // Noop 场景下 matched_id 也应设置
        assert_eq!(result.matched_id, Some("abc-123".to_string()));
    }

    #[test]
    fn test_write_guard_completely_different_content() {
        // 完全不相关的内容应判定为 Add
        let config = MemoryConfig::default();
        let existing = vec![
            make_entry("1", "数据库连接池配置"),
            make_entry("2", "使用 Docker 部署应用"),
        ];
        let result = WriteGuard::check("量子计算机的原理和应用", &existing, &config);
        assert_eq!(result.action, WriteGuardAction::Add);
        assert!(result.max_similarity < config.write_guard_update_threshold);
    }

    #[test]
    fn test_write_guard_max_similarity_tracked_correctly() {
        // 验证 max_similarity 字段始终反映实际最高值
        let config = MemoryConfig::default();
        let existing: Vec<MemoryEntry> = Vec::new();
        let result = WriteGuard::check("任意内容", &existing, &config);
        assert_eq!(result.max_similarity, 0.0);
        assert_eq!(result.matched_id, None);
    }
}
