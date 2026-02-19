use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 记忆条目结构（v2.1）
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
}

fn default_version() -> u32 {
    1
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
}

impl MemoryStore {
    /// SC-5: 当前支持的存储版本
    pub const CURRENT_VERSION: &'static str = "2.1";

    /// SC-5: 检查版本兼容性
    ///
    /// 返回 (is_compatible, needs_upgrade)
    pub fn check_version_compatibility(&self) -> (bool, bool) {
        match self.version.as_str() {
            "2.1" => (true, false),  // 当前版本，完全兼容
            "2.0" => (true, true),   // 旧版本，兼容但需升级
            "1.0" => (true, true),   // 旧版本，兼容但需升级
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
                    // version 和 snapshots 通过 serde default 自动填充
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

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: default_similarity_threshold(),
            dedup_on_startup: default_dedup_on_startup(),
            enable_dedup: default_enable_dedup(),
            max_entry_bytes: default_max_entry_bytes(),
            max_entries: default_max_entries(),
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
