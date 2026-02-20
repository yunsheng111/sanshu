use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 记忆条目结构（v2.2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 唯一标识符
    pub id: String,
    /// 记忆内容（原始）
    pub content: String,
    /// 归一化内容（用于相似度计算，去除空格和标点）
    #[serde(default)]
    pub content_normalized: String,
    /// 记忆分类
    pub category: MemoryCategory,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// SC-6: 版本号（每次更新递增）
    #[serde(default = "default_version")]
    pub version: u32,
    /// SC-6: 历史快照（最多保留 5 个）
    #[serde(default)]
    pub snapshots: Vec<MemorySnapshot>,

    // === v2.2 新增字段 ===

    /// URI 路径，如 "core://architecture/backend"
    /// 用于树形浏览和层级组织
    #[serde(default)]
    pub uri_path: Option<String>,

    /// 域名，从 uri_path 中提取的顶级域
    /// 如 "core", "project", "session", "legacy"
    #[serde(default)]
    pub domain: Option<String>,

    /// 自由标签，支持横向筛选
    /// 如 ["rust", "性能", "P0", "架构"]
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// 活力值 (0.0-3.0)，用于生命周期管理
    /// 默认 1.5，每次访问 +0.5，指数衰减（半衰期 30 天）
    #[serde(default = "default_vitality_score")]
    pub vitality_score: Option<f64>,

    /// 最后访问时间，用于活力衰减计算
    #[serde(default)]
    pub last_accessed_at: Option<DateTime<Utc>>,

    /// 自动生成的摘要（长记忆 > 500 字符时生成）
    #[serde(default)]
    pub summary: Option<String>,
}

fn default_version() -> u32 {
    1
}

fn default_vitality_score() -> Option<f64> {
    Some(1.5)
}

/// SC-6: 记忆快照（用于版本回滚）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// 快照版本号
    pub version: u32,
    /// 快照内容
    pub content: String,
    /// 快照时间
    pub created_at: DateTime<Utc>,
}

/// 记忆分类
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryCategory {
    /// 开发规范和规则
    Rule,
    /// 用户偏好设置
    Preference,
    /// 常用模式和最佳实践
    Pattern,
    /// 项目上下文信息
    Context,
}

impl MemoryCategory {
    /// 从字符串解析分类
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rule" | "规范" | "规则" => Self::Rule,
            "preference" | "偏好" => Self::Preference,
            "pattern" | "模式" | "最佳实践" => Self::Pattern,
            "context" | "背景" | "上下文" => Self::Context,
            _ => Self::Rule, // 默认为规则
        }
    }

    /// 获取分类的中文名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rule => "规范",
            Self::Preference => "偏好",
            Self::Pattern => "模式",
            Self::Context => "背景",
        }
    }
}

/// 新版记忆存储结构（v2.0）
///
/// 使用单一 JSON 文件存储所有记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStore {
    /// 存储格式版本
    pub version: String,
    /// 项目路径
    pub project_path: String,
    /// 所有记忆条目
    pub entries: Vec<MemoryEntry>,
    /// 最后去重时间
    pub last_dedup_at: DateTime<Utc>,
    /// 去重配置
    pub config: MemoryConfig,

    /// v2.2 新增：域注册表，记录所有使用中的域及其描述
    #[serde(default)]
    pub domains: Option<HashMap<String, DomainInfo>>,
}

/// 域信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    /// 域名
    pub name: String,
    /// 域描述
    pub description: Option<String>,
    /// 该域下的记忆条目数
    #[serde(default)]
    pub entry_count: usize,
}

/// Task 2 新增：活力值趋势
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalityTrend {
    /// 记忆 ID
    pub memory_id: String,
    /// 当前活力值（考虑衰减）
    pub current_vitality: f64,
    /// 基础活力值（上次记录值）
    pub base_vitality: f64,
    /// 最后访问时间
    pub last_accessed_at: DateTime<Utc>,
    /// 趋势点列表
    pub trend_points: Vec<VitalityTrendPoint>,
}

/// Task 2 新增：活力值趋势点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalityTrendPoint {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 该时间点的活力值
    pub vitality_score: f64,
    /// 事件描述（如"创建"、"更新"、"当前"）
    pub event: String,
}

impl MemoryStore {
    /// SC-5: 当前支持的存储版本
    pub const CURRENT_VERSION: &'static str = "2.2";

    /// SC-5: 检查版本兼容性
    ///
    /// 返回 (is_compatible, needs_upgrade)
    pub fn check_version_compatibility(&self) -> (bool, bool) {
        match self.version.as_str() {
            "2.2" => (true, false),  // 当前版本
            "2.1" => (true, true),   // 旧版本，需升级
            "2.0" => (true, true),   // 旧版本，需升级
            "1.0" => (true, true),   // 旧版本，需升级
            _ => (false, false),     // 未知版本，不兼容
        }
    }

    /// SC-5: 升级存储格式到当前版本
    pub fn upgrade_to_current(&mut self) -> anyhow::Result<()> {
        let (is_compatible, needs_upgrade) = self.check_version_compatibility();

        if !is_compatible {
            return Err(anyhow::anyhow!(
                "不兼容的存储版本: {}，当前支持版本: {}",
                self.version,
                Self::CURRENT_VERSION
            ));
        }

        if !needs_upgrade {
            return Ok(()); // 已是最新版本
        }

        // 执行版本升级
        match self.version.as_str() {
            "1.0" | "2.0" => {
                // v1.0/v2.0 -> v2.1: 添加 content_normalized, version, snapshots 字段
                for entry in &mut self.entries {
                    if entry.content_normalized.is_empty() {
                        entry.content_normalized = super::similarity::TextSimilarity::normalize(&entry.content);
                    }
                }
                // 继续升级到 v2.2
                self.version = "2.1".to_string();
                self.upgrade_to_current()
            }
            "2.1" => {
                // HC-19: v2.1 -> v2.2: 新字段通过 serde(default) 自动填充
                // 仅需显式设置 vitality_score 和 last_accessed_at 的默认值
                for entry in &mut self.entries {
                    if entry.vitality_score.is_none() {
                        entry.vitality_score = Some(1.5);
                    }
                    if entry.last_accessed_at.is_none() {
                        entry.last_accessed_at = Some(entry.updated_at);
                    }
                }
                self.version = Self::CURRENT_VERSION.to_string();
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION.to_string(),
            project_path: String::new(),
            entries: Vec::new(),
            last_dedup_at: Utc::now(),
            config: MemoryConfig::default(),
            domains: None,
        }
    }
}

/// 记忆去重配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 相似度阈值（0.0 ~ 1.0），默认 0.70
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f64,
    /// 启动时是否自动去重，默认 true
    #[serde(default = "default_dedup_on_startup")]
    pub dedup_on_startup: bool,
    /// 是否启用去重检测，默认 true
    #[serde(default = "default_enable_dedup")]
    pub enable_dedup: bool,
    /// 单条记忆最大字节数（默认 10240 = 10KB）
    #[serde(default = "default_max_entry_bytes")]
    pub max_entry_bytes: usize,
    /// 最大记忆条目数（默认 1000）
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    // === v2.2 新增配置 ===

    /// Write Guard 语义匹配阈值（>= 此值自动 NOOP）
    #[serde(default = "default_wg_semantic")]
    pub write_guard_semantic_threshold: f64,

    /// Write Guard 更新匹配阈值（此值到 semantic 之间自动 UPDATE）
    #[serde(default = "default_wg_update")]
    pub write_guard_update_threshold: f64,

    /// 活力衰减半衰期（天）
    #[serde(default = "default_decay_half_life")]
    pub vitality_decay_half_life_days: u32,

    /// 活力清理阈值
    #[serde(default = "default_cleanup_threshold")]
    pub vitality_cleanup_threshold: f64,

    /// 不活跃天数阈值
    #[serde(default = "default_inactive_days")]
    pub vitality_cleanup_inactive_days: u32,

    /// 每次访问提升的活力值
    #[serde(default = "default_access_boost")]
    pub vitality_access_boost: f64,

    /// 最大活力值
    #[serde(default = "default_max_vitality")]
    pub vitality_max_score: f64,

    /// 摘要生成的内容长度阈值（字符数）
    #[serde(default = "default_summary_threshold")]
    pub summary_length_threshold: usize,
}

fn default_similarity_threshold() -> f64 {
    0.70
}

fn default_dedup_on_startup() -> bool {
    true
}

fn default_enable_dedup() -> bool {
    true
}

fn default_max_entry_bytes() -> usize {
    10240 // 10KB
}

fn default_max_entries() -> usize {
    1000
}

fn default_wg_semantic() -> f64 { 0.80 }
fn default_wg_update() -> f64 { 0.60 }
fn default_decay_half_life() -> u32 { 30 }
fn default_cleanup_threshold() -> f64 { 0.35 }
fn default_inactive_days() -> u32 { 14 }
fn default_access_boost() -> f64 { 0.5 }
fn default_max_vitality() -> f64 { 3.0 }
fn default_summary_threshold() -> usize { 500 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: default_similarity_threshold(),
            dedup_on_startup: default_dedup_on_startup(),
            enable_dedup: default_enable_dedup(),
            max_entry_bytes: default_max_entry_bytes(),
            max_entries: default_max_entries(),
            // v2.2 新增
            write_guard_semantic_threshold: default_wg_semantic(),
            write_guard_update_threshold: default_wg_update(),
            vitality_decay_half_life_days: default_decay_half_life(),
            vitality_cleanup_threshold: default_cleanup_threshold(),
            vitality_cleanup_inactive_days: default_inactive_days(),
            vitality_access_boost: default_access_boost(),
            vitality_max_score: default_max_vitality(),
            summary_length_threshold: default_summary_threshold(),
        }
    }
}

/// 记忆元数据（兼容旧版）
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub project_path: String,
    pub last_organized: DateTime<Utc>,
    pub total_entries: usize,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- MemoryConfig 默认值验证 ---

    #[test]
    fn test_memory_config_default_values() {
        let config = MemoryConfig::default();
        assert_eq!(config.similarity_threshold, 0.70);
        assert!(config.dedup_on_startup);
        assert!(config.enable_dedup);
        assert_eq!(config.max_entry_bytes, 10240);
        assert_eq!(config.max_entries, 1000);
        // v2.2 新增配置
        assert_eq!(config.write_guard_semantic_threshold, 0.80);
        assert_eq!(config.write_guard_update_threshold, 0.60);
        assert_eq!(config.vitality_decay_half_life_days, 30);
        assert_eq!(config.vitality_cleanup_threshold, 0.35);
        assert_eq!(config.vitality_cleanup_inactive_days, 14);
        assert_eq!(config.vitality_access_boost, 0.5);
        assert_eq!(config.vitality_max_score, 3.0);
        assert_eq!(config.summary_length_threshold, 500);
    }

    // --- DomainInfo 序列化/反序列化 ---

    #[test]
    fn test_domain_info_serialize_roundtrip() {
        let info = DomainInfo {
            name: "core".to_string(),
            description: Some("核心域".to_string()),
            entry_count: 42,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: DomainInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "core");
        assert_eq!(deserialized.description, Some("核心域".to_string()));
        assert_eq!(deserialized.entry_count, 42);
    }

    #[test]
    fn test_domain_info_deserialize_missing_optional() {
        // description 为 null 时应反序列化为 None
        let json = r#"{"name":"test","description":null,"entry_count":0}"#;
        let info: DomainInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "test");
        assert_eq!(info.description, None);
        assert_eq!(info.entry_count, 0);
    }

    #[test]
    fn test_domain_info_entry_count_default() {
        // entry_count 缺失时应使用默认值 0
        let json = r#"{"name":"project","description":"项目"}"#;
        let info: DomainInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.entry_count, 0);
    }

    // --- MemoryCategory 中文别名 ---

    #[test]
    fn test_memory_category_chinese_aliases() {
        assert_eq!(MemoryCategory::from_str("规范"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str("规则"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str("偏好"), MemoryCategory::Preference);
        assert_eq!(MemoryCategory::from_str("模式"), MemoryCategory::Pattern);
        assert_eq!(MemoryCategory::from_str("最佳实践"), MemoryCategory::Pattern);
        assert_eq!(MemoryCategory::from_str("背景"), MemoryCategory::Context);
        assert_eq!(MemoryCategory::from_str("上下文"), MemoryCategory::Context);
    }

    #[test]
    fn test_memory_category_english() {
        assert_eq!(MemoryCategory::from_str("rule"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str("preference"), MemoryCategory::Preference);
        assert_eq!(MemoryCategory::from_str("pattern"), MemoryCategory::Pattern);
        assert_eq!(MemoryCategory::from_str("context"), MemoryCategory::Context);
    }

    #[test]
    fn test_memory_category_case_insensitive() {
        assert_eq!(MemoryCategory::from_str("Rule"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str("RULE"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str("Preference"), MemoryCategory::Preference);
    }

    #[test]
    fn test_memory_category_unknown_defaults_to_rule() {
        // 未知字符串默认为 Rule
        assert_eq!(MemoryCategory::from_str("unknown"), MemoryCategory::Rule);
        assert_eq!(MemoryCategory::from_str(""), MemoryCategory::Rule);
    }

    #[test]
    fn test_memory_category_display_name() {
        assert_eq!(MemoryCategory::Rule.display_name(), "规范");
        assert_eq!(MemoryCategory::Preference.display_name(), "偏好");
        assert_eq!(MemoryCategory::Pattern.display_name(), "模式");
        assert_eq!(MemoryCategory::Context.display_name(), "背景");
    }

    // --- v2.2 serde 默认填充 ---

    #[test]
    fn test_v22_serde_default_fill() {
        // 模拟 v2.1 数据（无 v2.2 新字段）反序列化到 v2.2 结构体
        let json = r#"{
            "id": "test-1",
            "content": "测试内容",
            "content_normalized": "测试内容",
            "category": "Rule",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "version": 1,
            "snapshots": []
        }"#;

        let entry: MemoryEntry = serde_json::from_str(json).unwrap();
        // v2.2 新字段应使用 serde(default) 自动填充
        assert_eq!(entry.uri_path, None);
        assert_eq!(entry.domain, None);
        assert_eq!(entry.tags, None);
        assert_eq!(entry.vitality_score, Some(1.5)); // default_vitality_score
        assert_eq!(entry.last_accessed_at, None);
        assert_eq!(entry.summary, None);
    }

    #[test]
    fn test_v22_entry_full_roundtrip() {
        // 完整 v2.2 条目序列化/反序列化
        let now = Utc::now();
        let entry = MemoryEntry {
            id: "full-test".to_string(),
            content: "完整测试".to_string(),
            content_normalized: "完整测试".to_string(),
            category: MemoryCategory::Pattern,
            created_at: now,
            updated_at: now,
            version: 3,
            snapshots: vec![MemorySnapshot {
                version: 2,
                content: "旧内容".to_string(),
                created_at: now,
            }],
            uri_path: Some("core://test/path".to_string()),
            domain: Some("core".to_string()),
            tags: Some(vec!["rust".to_string(), "test".to_string()]),
            vitality_score: Some(2.5),
            last_accessed_at: Some(now),
            summary: Some("[auto] 完整测试".to_string()),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let restored: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "full-test");
        assert_eq!(restored.version, 3);
        assert_eq!(restored.snapshots.len(), 1);
        assert_eq!(restored.uri_path, Some("core://test/path".to_string()));
        assert_eq!(restored.domain, Some("core".to_string()));
        assert_eq!(restored.tags, Some(vec!["rust".to_string(), "test".to_string()]));
        assert_eq!(restored.vitality_score, Some(2.5));
        assert_eq!(restored.summary, Some("[auto] 完整测试".to_string()));
    }

    // --- 版本兼容性矩阵 ---

    #[test]
    fn test_version_compatibility_matrix() {
        let store_22 = MemoryStore { version: "2.2".to_string(), ..Default::default() };
        let (compat, upgrade) = store_22.check_version_compatibility();
        assert!(compat);
        assert!(!upgrade);

        let store_21 = MemoryStore { version: "2.1".to_string(), ..Default::default() };
        let (compat, upgrade) = store_21.check_version_compatibility();
        assert!(compat);
        assert!(upgrade);

        let store_20 = MemoryStore { version: "2.0".to_string(), ..Default::default() };
        let (compat, upgrade) = store_20.check_version_compatibility();
        assert!(compat);
        assert!(upgrade);

        let store_10 = MemoryStore { version: "1.0".to_string(), ..Default::default() };
        let (compat, upgrade) = store_10.check_version_compatibility();
        assert!(compat);
        assert!(upgrade);

        let store_99 = MemoryStore { version: "99.0".to_string(), ..Default::default() };
        let (compat, upgrade) = store_99.check_version_compatibility();
        assert!(!compat);
        assert!(!upgrade);
    }

    #[test]
    fn test_memory_store_default() {
        let store = MemoryStore::default();
        assert_eq!(store.version, MemoryStore::CURRENT_VERSION);
        assert_eq!(store.project_path, "");
        assert!(store.entries.is_empty());
        assert!(store.domains.is_none());
    }

    #[test]
    fn test_memory_store_current_version() {
        assert_eq!(MemoryStore::CURRENT_VERSION, "2.2");
    }
}
