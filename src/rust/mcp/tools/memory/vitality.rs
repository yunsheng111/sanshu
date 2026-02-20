//! Vitality Decay 活力衰减引擎
//!
//! 衰减公式：V(t) = V0 * 2^(-t/half_life)
//! 访问提升：V_new = min(V_current + boost, max_vitality)
//! 清理候选：vitality < threshold AND last_accessed > inactive_days AND category != Rule

use chrono::{DateTime, Utc};
use super::types::{MemoryConfig, MemoryEntry, MemoryCategory};

/// 活力衰减引擎
pub struct VitalityEngine;

/// 清理候选条目
#[derive(Debug, Clone)]
pub struct CleanupCandidate {
    /// 记忆 ID
    pub id: String,
    /// 记忆内容（截取前 100 字符）
    pub content_preview: String,
    /// 当前活力值
    pub vitality_score: f64,
    /// 最后访问距今天数
    pub days_since_access: i64,
    /// 分类
    pub category: String,
}

impl VitalityEngine {
    /// 计算当前活力值（指数衰减）
    ///
    /// V(t) = V0 * 2^(-t/half_life)
    ///
    /// # 参数
    /// - `base_vitality`: 上次记录的活力值 V0
    /// - `last_accessed`: 上次访问时间
    /// - `half_life_days`: 半衰期（天）
    pub fn calculate_current_vitality(
        base_vitality: f64,
        last_accessed: DateTime<Utc>,
        half_life_days: u32,
    ) -> f64 {
        let now = Utc::now();
        let elapsed_days = (now - last_accessed).num_seconds() as f64 / 86400.0;

        if elapsed_days <= 0.0 || half_life_days == 0 {
            return base_vitality;
        }

        base_vitality * (2.0_f64).powf(-elapsed_days / half_life_days as f64)
    }

    /// 执行访问提升
    ///
    /// V_new = min(V_current + boost, max_vitality)
    pub fn boost_vitality(
        entry: &mut MemoryEntry,
        config: &MemoryConfig,
    ) {
        let current = entry.vitality_score.unwrap_or(1.5);
        let boosted = (current + config.vitality_access_boost)
            .min(config.vitality_max_score);
        entry.vitality_score = Some(boosted);
        entry.last_accessed_at = Some(Utc::now());
    }

    /// HC-15: 获取清理候选列表
    ///
    /// 条件：
    /// - vitality_score < cleanup_threshold (0.35)
    /// - last_accessed_at < now - inactive_days (14 天)
    /// - RISK-06: category != Rule（Rule 分类永不自动清理）
    pub fn get_cleanup_candidates(
        entries: &[MemoryEntry],
        config: &MemoryConfig,
    ) -> Vec<CleanupCandidate> {
        let now = Utc::now();
        let inactive_threshold = chrono::Duration::days(
            config.vitality_cleanup_inactive_days as i64
        );

        entries.iter().filter_map(|entry| {
            // RISK-06: Rule 分类永不自动清理
            if entry.category == MemoryCategory::Rule {
                return None;
            }

            let last_accessed = entry.last_accessed_at.unwrap_or(entry.updated_at);
            let current_vitality = Self::calculate_current_vitality(
                entry.vitality_score.unwrap_or(1.5),
                last_accessed,
                config.vitality_decay_half_life_days,
            );

            let days_since = (now - last_accessed).num_days();

            // 检查清理条件
            if current_vitality < config.vitality_cleanup_threshold
                && (now - last_accessed) > inactive_threshold
            {
                let preview = if entry.content.len() > 100 {
                    format!("{}...", &entry.content[..entry.content.char_indices()
                        .nth(100).map(|(i, _)| i).unwrap_or(entry.content.len())])
                } else {
                    entry.content.clone()
                };

                Some(CleanupCandidate {
                    id: entry.id.clone(),
                    content_preview: preview,
                    vitality_score: current_vitality,
                    days_since_access: days_since,
                    category: entry.category.display_name().to_string(),
                })
            } else {
                None
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vitality_decay_formula() {
        // 30 天后活力值应减半
        let last_accessed = Utc::now() - chrono::Duration::days(30);
        let vitality = VitalityEngine::calculate_current_vitality(1.5, last_accessed, 30);
        assert!((vitality - 0.75).abs() < 0.05, "30天后应约为0.75, 实际: {}", vitality);
    }

    #[test]
    fn test_vitality_no_decay() {
        // 刚刚访问，活力值不变
        let vitality = VitalityEngine::calculate_current_vitality(2.0, Utc::now(), 30);
        assert!((vitality - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_rule_never_cleanup() {
        let config = MemoryConfig::default();
        let now = Utc::now();
        let old_time = now - chrono::Duration::days(365);

        let entries = vec![MemoryEntry {
            id: "1".to_string(),
            content: "重要规范".to_string(),
            content_normalized: "重要规范".to_string(),
            category: MemoryCategory::Rule,
            created_at: old_time,
            updated_at: old_time,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: Some(0.1), // 极低活力
            last_accessed_at: Some(old_time), // 很久没访问
            summary: None,
        }];

        let candidates = VitalityEngine::get_cleanup_candidates(&entries, &config);
        assert!(candidates.is_empty(), "Rule 分类不应出现在清理候选中");
    }

    // --- 追加测试 ---

    fn make_test_entry(id: &str, category: MemoryCategory, vitality: f64, days_ago: i64) -> MemoryEntry {
        let now = Utc::now();
        let last_accessed = now - chrono::Duration::days(days_ago);
        MemoryEntry {
            id: id.to_string(),
            content: format!("测试内容 {}", id),
            content_normalized: format!("测试内容 {}", id),
            category,
            created_at: last_accessed,
            updated_at: last_accessed,
            version: 1,
            snapshots: Vec::new(),
            uri_path: None,
            domain: None,
            tags: None,
            vitality_score: Some(vitality),
            last_accessed_at: Some(last_accessed),
            summary: None,
        }
    }

    #[test]
    fn test_vitality_decay_60_days() {
        // 60 天 = 2 个半衰期（半衰期 30 天），活力应衰减到约 1/4
        // V(60) = 1.5 * 2^(-60/30) = 1.5 * 0.25 = 0.375
        let last_accessed = Utc::now() - chrono::Duration::days(60);
        let vitality = VitalityEngine::calculate_current_vitality(1.5, last_accessed, 30);
        assert!(
            (vitality - 0.375).abs() < 0.05,
            "60天后应约为 0.375，实际: {}",
            vitality
        );
    }

    #[test]
    fn test_vitality_decay_90_days() {
        // 90 天 = 3 个半衰期，V = 1.5 * 2^(-3) = 1.5 * 0.125 = 0.1875
        let last_accessed = Utc::now() - chrono::Duration::days(90);
        let vitality = VitalityEngine::calculate_current_vitality(1.5, last_accessed, 30);
        assert!(
            (vitality - 0.1875).abs() < 0.05,
            "90天后应约为 0.1875，实际: {}",
            vitality
        );
    }

    #[test]
    fn test_boost_vitality_not_exceed_max() {
        // boost 后不应超过 max_vitality (3.0)
        let config = MemoryConfig::default();
        let mut entry = make_test_entry("1", MemoryCategory::Pattern, 2.8, 0);
        // boost = 0.5, 2.8 + 0.5 = 3.3 > max(3.0)，应被截断为 3.0
        VitalityEngine::boost_vitality(&mut entry, &config);
        assert_eq!(entry.vitality_score, Some(3.0));
    }

    #[test]
    fn test_boost_vitality_normal() {
        // 正常 boost
        let config = MemoryConfig::default();
        let mut entry = make_test_entry("1", MemoryCategory::Pattern, 1.0, 0);
        VitalityEngine::boost_vitality(&mut entry, &config);
        assert_eq!(entry.vitality_score, Some(1.5)); // 1.0 + 0.5
        // last_accessed_at 应被更新为当前时间
        assert!(entry.last_accessed_at.is_some());
    }

    #[test]
    fn test_boost_vitality_none_defaults_to_1_5() {
        // vitality_score 为 None 时默认使用 1.5
        let config = MemoryConfig::default();
        let mut entry = make_test_entry("1", MemoryCategory::Context, 0.0, 5);
        entry.vitality_score = None;
        VitalityEngine::boost_vitality(&mut entry, &config);
        assert_eq!(entry.vitality_score, Some(2.0)); // 1.5 (default) + 0.5
    }

    #[test]
    fn test_zero_half_life_no_crash() {
        // 零半衰期不应崩溃，应返回原始活力值
        let last_accessed = Utc::now() - chrono::Duration::days(30);
        let vitality = VitalityEngine::calculate_current_vitality(1.5, last_accessed, 0);
        assert_eq!(vitality, 1.5, "零半衰期应返回原始活力值");
    }

    #[test]
    fn test_negative_elapsed_no_crash() {
        // 未来时间（负 elapsed）应返回原始活力值
        let future = Utc::now() + chrono::Duration::days(10);
        let vitality = VitalityEngine::calculate_current_vitality(2.0, future, 30);
        assert_eq!(vitality, 2.0, "未来时间应返回原始活力值");
    }

    #[test]
    fn test_cleanup_non_rule_low_vitality() {
        // 非 Rule 分类、低活力、长时间不活跃应被清理
        let config = MemoryConfig::default();
        let entries = vec![
            make_test_entry("1", MemoryCategory::Preference, 0.1, 30), // 低活力 + 不活跃
        ];
        let candidates = VitalityEngine::get_cleanup_candidates(&entries, &config);
        // 该条目活力极低且不活跃 30 天 > 14 天阈值
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "1");
    }

    #[test]
    fn test_cleanup_mixed_entries() {
        // 混合场景：Rule 低活力被豁免，Pattern 低活力被清理，高活力不被清理
        let config = MemoryConfig::default();
        let entries = vec![
            make_test_entry("rule-old", MemoryCategory::Rule, 0.01, 365),      // Rule：豁免
            make_test_entry("pattern-old", MemoryCategory::Pattern, 0.01, 30), // Pattern：清理候选
            make_test_entry("context-fresh", MemoryCategory::Context, 2.5, 1), // 高活力：不清理
            make_test_entry("pref-inactive", MemoryCategory::Preference, 0.05, 60), // 低活力+不活跃：清理候选
        ];
        let candidates = VitalityEngine::get_cleanup_candidates(&entries, &config);

        let candidate_ids: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
        // Rule 不应在清理列表中
        assert!(!candidate_ids.contains(&"rule-old"), "Rule 应被豁免");
        // 高活力不应在清理列表中
        assert!(!candidate_ids.contains(&"context-fresh"), "高活力不应被清理");
        // 低活力 + 不活跃的非 Rule 条目应在清理列表中
        assert!(candidate_ids.contains(&"pattern-old"), "低活力 Pattern 应被清理");
        assert!(candidate_ids.contains(&"pref-inactive"), "低活力 Preference 应被清理");
    }

    #[test]
    fn test_cleanup_empty_entries() {
        let config = MemoryConfig::default();
        let entries: Vec<MemoryEntry> = Vec::new();
        let candidates = VitalityEngine::get_cleanup_candidates(&entries, &config);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_cleanup_candidate_content_preview() {
        // 验证 content_preview 截断逻辑
        let config = MemoryConfig::default();
        let mut entry = make_test_entry("long", MemoryCategory::Pattern, 0.01, 30);
        entry.content = "A".repeat(200); // 超过 100 字符
        let entries = vec![entry];
        let candidates = VitalityEngine::get_cleanup_candidates(&entries, &config);
        if !candidates.is_empty() {
            assert!(candidates[0].content_preview.ends_with("..."), "长内容应被截断并添加省略号");
        }
    }
}
