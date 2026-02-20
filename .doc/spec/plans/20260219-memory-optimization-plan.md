# OpenSpec 零决策计划：记忆管理系统全面优化

## 计划元信息

| 字段 | 值 |
|------|-----|
| 提案来源 | `.doc/spec/proposals/20260219-memory-optimization-proposal.md` |
| 约束集来源 | `.doc/spec/constraints/20260219-memory-optimization-constraints.md` |
| 研究文档 | `.doc/workflow/research/20260219-memory-optimization-analysis.md` |
| 总步骤数 | 32 |
| 关联约束 | HC-10~19, SC-4~26, DEP-01~08, RISK-01~10 |
| 预估工时 | 约 116h（14.5 工作日） |
| 版本 | 1.0 |
| 创建日期 | 2026-02-19 |

---

## 前置条件检查清单

执行任何步骤前，必须完成以下检查：

- [ ] `cargo test` 全部通过（确认基线稳定）
- [ ] `pnpm vitest` 全部通过（前端基线稳定）
- [ ] Git 工作区干净（`git status` 无未提交变更）
- [ ] 确认 `src/rust/mcp/tools/memory/` 目录下有 7 个文件：`types.rs`, `manager.rs`, `similarity.rs`, `dedup.rs`, `migration.rs`, `mcp.rs`, `mod.rs`
- [ ] 确认 `src/frontend/components/tools/` 下有 `MemoryConfig.vue`, `MemoryList.vue`, `MemorySearch.vue`
- [ ] 确认 `Cargo.toml` 中尚无 `rusqlite` 依赖
- [ ] 确认 `MemoryStore::CURRENT_VERSION` 当前为 `"2.1"`

---

## 阶段 P0：质量基础（步骤 1-10，约 20h）

> **阶段目标**：完成数据模型升级、Write Guard 前置拦截、MemoryManagerRegistry 全局池
> **阶段验收**：`cargo test` 全部通过 + Write Guard 拦截测试 + Registry 缓存复用测试

---

### 步骤 1：MemoryEntry v2.2 数据模型升级

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/types.rs`
- **关联约束**：HC-12（向后兼容 v2.1）, HC-19（旧记忆默认值）, DEP-03（P1 依赖 v2.2）
- **依赖步骤**：无
- **变更内容**：

在 `MemoryEntry` 结构体的 `snapshots` 字段之后，新增 6 个 `Option<T>` 字段：

```rust
/// 记忆条目结构（v2.2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // === v2.1 已有字段（不变） ===
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub content_normalized: String,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default = "default_version")]
    pub version: u32,
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

// === 新增默认值函数 ===
fn default_vitality_score() -> Option<f64> {
    Some(1.5)
}
```

**具体编辑操作**：

1. 在 `types.rs` 第 26 行 `pub snapshots: Vec<MemorySnapshot>,` 之后插入上述 6 个新字段
2. 在 `default_version()` 函数之后新增 `default_vitality_score()` 函数
3. 修改文件顶部注释：`/// 记忆条目结构（v2.1）` 改为 `/// 记忆条目结构（v2.2）`

**同步更新构造位置**（所有创建 `MemoryEntry` 的地方都需要补齐新字段）：

- `manager.rs` 第 209-218 行 `add_memory()` 中的 `MemoryEntry { ... }`：在 `snapshots: Vec::new()` 之后追加：
  ```rust
  uri_path: None,
  domain: None,
  tags: None,
  vitality_score: Some(1.5),
  last_accessed_at: Some(now),
  summary: None,
  ```
- `migration.rs` 第 167-176 行 `parse_md_file()` 中的 `MemoryEntry { ... }`：在 `snapshots: Vec::new()` 之后追加相同的 6 个字段（`vitality_score: Some(1.5)`, `last_accessed_at: Some(Utc::now())`, 其余 `None`）
- `dedup.rs` 第 161-172 行 `make_entry()` 测试辅助函数：在 `snapshots: Vec::new()` 之后追加 6 个字段（全部 `None` 或默认值）

- **验证方式**：
  1. `cargo build --lib` 编译通过（零错误）
  2. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过
  3. 手动检查：使用 `serde_json::from_str` 反序列化一条 v2.1 格式的 JSON 记忆条目，新字段应自动填充默认值

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/types.rs src/rust/mcp/tools/memory/manager.rs src/rust/mcp/tools/memory/migration.rs src/rust/mcp/tools/memory/dedup.rs`

---

### 步骤 2：MemoryConfig v2.2 扩展

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/types.rs`
- **关联约束**：SC-15（Write Guard 阈值可配置）, SC-16（Vitality 参数可配置）
- **依赖步骤**：步骤 1
- **变更内容**：

在 `MemoryConfig` 结构体的 `max_entries` 字段之后，新增 7 个配置字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    // === v2.1 已有字段（不变） ===
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f64,
    #[serde(default = "default_dedup_on_startup")]
    pub dedup_on_startup: bool,
    #[serde(default = "default_enable_dedup")]
    pub enable_dedup: bool,
    #[serde(default = "default_max_entry_bytes")]
    pub max_entry_bytes: usize,
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
}
```

新增默认值函数（在现有默认值函数之后）：

```rust
fn default_wg_semantic() -> f64 { 0.80 }
fn default_wg_update() -> f64 { 0.60 }
fn default_decay_half_life() -> u32 { 30 }
fn default_cleanup_threshold() -> f64 { 0.35 }
fn default_inactive_days() -> u32 { 14 }
fn default_access_boost() -> f64 { 0.5 }
fn default_max_vitality() -> f64 { 3.0 }
```

同步更新 `impl Default for MemoryConfig`：

```rust
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
        }
    }
}
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 反序列化一个不含新字段的旧配置 JSON 字符串，确认新字段自动填充默认值

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/types.rs`

---

### 步骤 3：MemoryStore v2.2 升级

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/types.rs`
- **关联约束**：HC-12（向后兼容）
- **依赖步骤**：步骤 1, 2
- **变更内容**：

1. 修改 `MemoryStore::CURRENT_VERSION` 常量值：

```rust
pub const CURRENT_VERSION: &'static str = "2.2";
```

2. 在 `MemoryStore` 结构体的 `config` 字段之后新增域注册表字段：

```rust
pub struct MemoryStore {
    pub version: String,
    pub project_path: String,
    pub entries: Vec<MemoryEntry>,
    pub last_dedup_at: DateTime<Utc>,
    pub config: MemoryConfig,

    /// v2.2 新增：域注册表，记录所有使用中的域及其描述
    #[serde(default)]
    pub domains: Option<HashMap<String, DomainInfo>>,
}
```

3. 在 `types.rs` 文件中新增 `DomainInfo` 结构体（在 `MemoryStore` 定义之后）：

```rust
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
```

4. 在文件顶部添加 `use std::collections::HashMap;` 导入

5. 更新 `check_version_compatibility()` 方法：

```rust
pub fn check_version_compatibility(&self) -> (bool, bool) {
    match self.version.as_str() {
        "2.2" => (true, false),  // 当前版本
        "2.1" => (true, true),   // 旧版本，需升级
        "2.0" => (true, true),   // 旧版本，需升级
        "1.0" => (true, true),   // 旧版本，需升级
        _ => (false, false),
    }
}
```

6. 更新 `upgrade_to_current()` 方法：

```rust
pub fn upgrade_to_current(&mut self) -> anyhow::Result<()> {
    let (is_compatible, needs_upgrade) = self.check_version_compatibility();
    if !is_compatible {
        return Err(anyhow::anyhow!(
            "不兼容的存储版本: {}，当前支持版本: {}",
            self.version, Self::CURRENT_VERSION
        ));
    }
    if !needs_upgrade {
        return Ok(());
    }

    match self.version.as_str() {
        "1.0" | "2.0" => {
            // v1.0/v2.0 -> v2.1: 添加 content_normalized, version, snapshots
            for entry in &mut self.entries {
                if entry.content_normalized.is_empty() {
                    entry.content_normalized =
                        super::similarity::TextSimilarity::normalize(&entry.content);
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
```

7. 更新 `impl Default for MemoryStore`，在 `config` 之后添加 `domains: None`

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 创建一个 v2.1 格式的 JSON 文件，调用 `upgrade_to_current()`，验证版本变为 "2.2"，每条记忆的 `vitality_score` 为 `Some(1.5)`，`last_accessed_at` 为 `Some(updated_at)`
  3. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/types.rs`

---

### 步骤 4：迁移路径更新（v2.1 -> v2.2）

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/migration.rs`
- **关联约束**：HC-19（旧记忆默认值）, RISK-02（迁移失败防护）
- **依赖步骤**：步骤 1, 3
- **变更内容**：

`migration.rs` 中的 `parse_md_file()` 已在步骤 1 中更新了 `MemoryEntry` 构造。此步骤无需额外修改 `migration.rs` 的迁移逻辑，因为 v2.1 -> v2.2 的迁移由 `MemoryStore::upgrade_to_current()` 处理（步骤 3）。

此步骤仅需验证迁移路径完整性：
- v1.0 (MD) -> `MemoryMigrator::migrate()` -> v2.0 JSON -> `upgrade_to_current()` -> v2.1 -> v2.2
- v2.0 JSON -> `upgrade_to_current()` -> v2.1 -> v2.2
- v2.1 JSON -> `upgrade_to_current()` -> v2.2

**新增单元测试**（在 `migration.rs` 的 `#[cfg(test)]` 模块中追加）：

```rust
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
```

- **验证方式**：
  1. `cargo test --package sanshu --lib mcp::tools::memory::migration` 通过
  2. 上述新增的 `test_v21_to_v22_upgrade` 测试通过

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/migration.rs`

---

### 步骤 5：Write Guard 写入守卫模块

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/write_guard.rs`
- **关联约束**：HC-11（三级判定）, SC-15（阈值可配置）, DEP-01（复用 TextSimilarity）, RISK-04（假阳性防护）, RISK-05（假阴性兜底）
- **依赖步骤**：步骤 1, 2
- **变更内容**：

创建新文件 `src/rust/mcp/tools/memory/write_guard.rs`，完整内容如下：

```rust
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
}
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory::write_guard` 3 个测试全部通过

- **回滚方案**：删除 `src/rust/mcp/tools/memory/write_guard.rs`

---

### 步骤 6：manager.rs 集成 Write Guard

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/manager.rs`
- **关联约束**：HC-11（Write Guard 在 add_memory 内）, HC-18（原子写入不变）
- **依赖步骤**：步骤 1, 2, 5
- **变更内容**：

1. 在文件顶部导入区域新增：

```rust
use super::write_guard::{WriteGuard, WriteGuardAction, WriteGuardResult};
```

2. 修改 `MemoryManager::add_memory()` 方法（当前在第 166-225 行），将现有的去重检测替换为 Write Guard：

**修改前**（第 190-203 行）：
```rust
// 如果启用去重检测，检查是否重复
if self.store.config.enable_dedup {
    let dedup = MemoryDeduplicator::new(self.store.config.similarity_threshold);
    let dup_info = dedup.check_duplicate(content, &self.store.entries);
    if dup_info.is_duplicate {
        log_debug!(...);
        return Ok(None);
    }
}
```

**修改后**：
```rust
// HC-11: Write Guard 写入前相似度检查
let guard_result = WriteGuard::check(content, &self.store.entries, &self.store.config);

match &guard_result.action {
    WriteGuardAction::Noop { matched_id, similarity } => {
        // 相似度 >= 0.80，静默拒绝
        log_debug!(
            "[WriteGuard] NOOP: 新内容与记忆 {} 相似度 {:.1}%，静默拒绝",
            matched_id, similarity * 100.0
        );
        return Ok(None);
    }
    WriteGuardAction::Update { matched_id, matched_content: _, similarity } => {
        // 0.60 <= 相似度 < 0.80，自动合并更新
        log_debug!(
            "[WriteGuard] UPDATE: 自动合并到记忆 {}（相似度 {:.1}%）",
            matched_id, similarity * 100.0
        );
        // 使用 append 模式追加新内容到匹配条目
        let merge_content = format!("\n---\n{}", content);
        return self.update_memory(matched_id, &merge_content, true)
            .map(|opt| opt.map(|id| id));
    }
    WriteGuardAction::Add => {
        // 相似度 < 0.60，继续正常新增流程
    }
}
```

3. 新增方法 `add_memory_with_guard_result()`（供 MCP 层获取 Write Guard 判定详情）：

在 `add_memory()` 方法之后新增：

```rust
/// 添加记忆并返回 Write Guard 判定结果
///
/// 与 add_memory() 相同逻辑，但额外返回 WriteGuardResult 供 MCP 层使用
pub fn add_memory_with_guard_result(
    &mut self,
    content: &str,
    category: MemoryCategory,
) -> Result<(Option<String>, WriteGuardResult)> {
    let content = content.trim();
    if content.is_empty() {
        return Err(anyhow::anyhow!("记忆内容不能为空"));
    }

    // HC-10: 大小限制
    if content.len() > self.store.config.max_entry_bytes {
        return Err(anyhow::anyhow!(
            "记忆内容超过大小限制: {} 字节 > {} 字节上限",
            content.len(), self.store.config.max_entry_bytes
        ));
    }

    // HC-10: 数量限制
    if self.store.entries.len() >= self.store.config.max_entries {
        return Err(anyhow::anyhow!(
            "记忆条目数已达上限: {} / {}",
            self.store.entries.len(), self.store.config.max_entries
        ));
    }

    // HC-11: Write Guard
    let guard_result = WriteGuard::check(content, &self.store.entries, &self.store.config);

    match &guard_result.action {
        WriteGuardAction::Noop { .. } => {
            return Ok((None, guard_result));
        }
        WriteGuardAction::Update { matched_id, .. } => {
            let merge_content = format!("\n---\n{}", content);
            let update_result = self.update_memory(matched_id, &merge_content, true)?;
            return Ok((update_result, guard_result));
        }
        WriteGuardAction::Add => {}
    }

    // 正常新增
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    let entry = MemoryEntry {
        id: id.clone(),
        content: content.to_string(),
        content_normalized: TextSimilarity::normalize(content),
        category,
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
    };

    self.store.entries.push(entry);
    self.save_store()?;
    Ok((Some(id), guard_result))
}
```

4. 同步在 `SharedMemoryManager` 中新增对应的包装方法：

```rust
/// 添加记忆并返回 Write Guard 判定结果（写锁）
pub fn add_memory_with_guard_result(
    &self,
    content: &str,
    category: MemoryCategory,
) -> Result<(Option<String>, super::write_guard::WriteGuardResult)> {
    let mut manager = self.inner.write()
        .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
    manager.add_memory_with_guard_result(content, category)
}
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过
  3. 新增专项测试验证 Write Guard 集成（在 manager.rs tests 模块中）：
     - 测试相似内容被 NOOP 拒绝
     - 测试中等相似内容被 UPDATE 合并

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/manager.rs`

---

### 步骤 7：MemoryManagerRegistry 全局管理器池

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/registry.rs`
- **关联约束**：HC-13（Weak 引用 + TTL + 池大小上限）, SC-22（懒加载 + canonical 规范化）, DEP-02（依赖 SharedMemoryManager）, RISK-03（内存泄漏防护）
- **依赖步骤**：步骤 1
- **变更内容**：

创建新文件 `src/rust/mcp/tools/memory/registry.rs`，完整内容如下：

```rust
//! MemoryManagerRegistry 全局管理器池
//!
//! HC-13: 使用 Weak<RwLock<MemoryManager>> 引用 + TTL 回收 + 池大小上限
//! SC-22: 懒加载，首次请求时创建管理器并缓存

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};
use anyhow::Result;
use once_cell::sync::Lazy;

use super::manager::{MemoryManager, SharedMemoryManager};
use crate::log_debug;

/// 全局 Registry 单例
pub static REGISTRY: Lazy<MemoryManagerRegistry> = Lazy::new(MemoryManagerRegistry::new);

/// TTL 默认值：30 分钟
const DEFAULT_TTL_SECS: u64 = 30 * 60;

/// 池大小上限：16
const DEFAULT_POOL_SIZE: usize = 16;

/// 清理间隔：5 分钟
const CLEANUP_INTERVAL_SECS: u64 = 5 * 60;

/// 弱引用条目
struct WeakEntry {
    /// 管理器的弱引用
    weak: Weak<RwLock<MemoryManager>>,
    /// 最后访问时间
    last_access: Instant,
}

/// 全局管理器池
pub struct MemoryManagerRegistry {
    /// 项目路径 -> 弱引用条目
    pool: RwLock<HashMap<String, WeakEntry>>,
    /// TTL 持续时间
    ttl: Duration,
    /// 池大小上限
    max_size: usize,
    /// 上次清理时间
    last_cleanup: RwLock<Instant>,
}

impl MemoryManagerRegistry {
    /// 创建新的 Registry
    fn new() -> Self {
        Self {
            pool: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            max_size: DEFAULT_POOL_SIZE,
            last_cleanup: RwLock::new(Instant::now()),
        }
    }

    /// SC-22: 获取或创建 SharedMemoryManager
    ///
    /// 1. 规范化 project_path
    /// 2. 检查缓存中是否有有效的管理器
    /// 3. 如果有且 Weak::upgrade 成功，更新 last_access 并返回
    /// 4. 如果没有或已失效，创建新的 SharedMemoryManager 并缓存
    pub fn get_or_create(&self, project_path: &str) -> Result<SharedMemoryManager> {
        let canonical = Self::canonical_path(project_path);

        // 先尝试读锁查找
        {
            let pool = self.pool.read()
                .map_err(|e| anyhow::anyhow!("Registry 读锁失败: {}", e))?;

            if let Some(entry) = pool.get(&canonical) {
                if let Some(arc) = entry.weak.upgrade() {
                    // 缓存命中，但需要更新 last_access（延迟到写锁）
                    log_debug!("[Registry] 缓存命中: {}", canonical);
                    // 克隆 Arc 返回 SharedMemoryManager
                    drop(pool);
                    // 更新 last_access
                    self.touch(&canonical);
                    return Ok(SharedMemoryManager::from_arc(arc));
                }
            }
        }

        // 缓存未命中或已失效，需要创建新的
        self.maybe_cleanup();

        let manager = SharedMemoryManager::new(project_path)?;
        let arc = manager.inner_arc();

        let mut pool = self.pool.write()
            .map_err(|e| anyhow::anyhow!("Registry 写锁失败: {}", e))?;

        // HC-13: 池大小上限检查
        if pool.len() >= self.max_size {
            // 移除最久未访问的条目
            self.evict_oldest(&mut pool);
        }

        pool.insert(canonical.clone(), WeakEntry {
            weak: Arc::downgrade(&arc),
            last_access: Instant::now(),
        });

        log_debug!("[Registry] 创建新管理器: {}, pool_size={}", canonical, pool.len());
        Ok(manager)
    }

    /// 更新条目的最后访问时间
    fn touch(&self, canonical: &str) {
        if let Ok(mut pool) = self.pool.write() {
            if let Some(entry) = pool.get_mut(canonical) {
                entry.last_access = Instant::now();
            }
        }
    }

    /// 定期清理过期条目
    fn maybe_cleanup(&self) {
        let should_cleanup = {
            let last = self.last_cleanup.read().ok();
            last.map_or(true, |t| t.elapsed() > Duration::from_secs(CLEANUP_INTERVAL_SECS))
        };

        if !should_cleanup {
            return;
        }

        if let Ok(mut pool) = self.pool.write() {
            let before = pool.len();
            pool.retain(|path, entry| {
                // 移除：Weak 已失效 或 TTL 过期
                let alive = entry.weak.upgrade().is_some();
                let fresh = entry.last_access.elapsed() < self.ttl;
                let keep = alive && fresh;
                if !keep {
                    log_debug!("[Registry] 清理过期条目: {} (alive={}, fresh={})", path, alive, fresh);
                }
                keep
            });
            let after = pool.len();
            if before != after {
                log_debug!("[Registry] 清理完成: {} -> {} 条目", before, after);
            }
        }

        if let Ok(mut last) = self.last_cleanup.write() {
            *last = Instant::now();
        }
    }

    /// 移除最久未访问的条目
    fn evict_oldest(pool: &mut HashMap<String, WeakEntry>) {
        if let Some(oldest_key) = pool.iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone())
        {
            log_debug!("[Registry] 驱逐最久未访问: {}", oldest_key);
            pool.remove(&oldest_key);
        }
    }

    /// 规范化项目路径为 canonical key
    fn canonical_path(project_path: &str) -> String {
        // 统一路径分隔符为 /，去除尾部 /，转小写（Windows 不区分大小写）
        let normalized = project_path
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase();
        normalized
    }

    /// 获取当前池大小（用于监控）
    pub fn pool_size(&self) -> usize {
        self.pool.read().map(|p| p.len()).unwrap_or(0)
    }
}
```

**同步修改 `manager.rs`**：为 `SharedMemoryManager` 新增两个方法，供 Registry 使用：

```rust
impl SharedMemoryManager {
    // ... 现有方法 ...

    /// 获取内部 Arc（供 Registry 缓存 Weak 引用）
    pub(super) fn inner_arc(&self) -> Arc<RwLock<MemoryManager>> {
        Arc::clone(&self.inner)
    }

    /// 从已有的 Arc 创建 SharedMemoryManager（供 Registry 缓存命中时使用）
    pub(super) fn from_arc(arc: Arc<RwLock<MemoryManager>>) -> Self {
        Self { inner: arc }
    }
}
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 编写单元测试验证：
     - `get_or_create` 同一路径两次返回同一实例（`Arc::ptr_eq`）
     - 不同路径创建不同实例
     - 池大小不超过 16

- **回滚方案**：删除 `src/rust/mcp/tools/memory/registry.rs`，回退 `manager.rs` 变更

---

### 步骤 8：MCP 入口更新（Write Guard + 配置）

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mcp.rs`
- **关联约束**：HC-11（记忆操作返回 WriteGuardResult）, SC-15（配置暴露新参数）
- **依赖步骤**：步骤 5, 6, 7
- **变更内容**：

1. 修改 `"记忆"` 分支（第 64-102 行），使用 `add_memory_with_guard_result()`：

```rust
"记忆" => {
    if request.content.trim().is_empty() {
        return Err(McpError::invalid_params("缺少记忆内容".to_string(), None));
    }

    let category = MemoryCategory::from_str(&request.category);
    log_debug!("[ji] 执行记忆操作: category={:?}, content_len={}", category, request.content.len());

    match manager.add_memory_with_guard_result(&request.content, category) {
        Ok((Some(id), guard_result)) => {
            let action_label = match &guard_result.action {
                super::write_guard::WriteGuardAction::Add => "新增",
                super::write_guard::WriteGuardAction::Update { .. } => "合并更新",
                super::write_guard::WriteGuardAction::Noop { .. } => "静默拒绝",
            };
            log_important!(info, "[ji] 记忆操作: id={}, action={}, similarity={:.1}%",
                id, action_label, guard_result.max_similarity * 100.0);
            format!(
                "✅ 记忆已{}，ID: {}\n📝 内容: {}\n📂 分类: {}\n🛡️ Write Guard: {} (相似度: {:.1}%){}{}",
                action_label, id, request.content, category.display_name(),
                action_label, guard_result.max_similarity * 100.0,
                index_hint, non_git_hint
            )
        }
        Ok((None, guard_result)) => {
            log_debug!("[ji] Write Guard NOOP: similarity={:.1}%", guard_result.max_similarity * 100.0);
            format!(
                "⚠️ 记忆被 Write Guard 拦截（相似度: {:.1}%，阈值: {:.0}%）\n📝 内容: {}\n📂 分类: {}\n💡 如需强制添加，可降低 write_guard_semantic_threshold{}{}",
                guard_result.max_similarity * 100.0,
                manager.config().map(|c| c.write_guard_semantic_threshold * 100.0).unwrap_or(80.0),
                request.content, category.display_name(),
                index_hint, non_git_hint
            )
        }
        Err(e) => {
            log_important!(error, "[ji] 添加记忆失败: {}", e);
            return Err(McpError::internal_error(format!("添加记忆失败: {}", e), None));
        }
    }
}
```

2. 修改 `"配置"` 分支，扩展支持新配置参数（在现有的 `similarity_threshold`/`dedup_on_startup`/`enable_dedup` 处理之后，新增对 Write Guard 和 Vitality 参数的处理）。

3. 将 `SharedMemoryManager::new()` 调用替换为 `super::registry::REGISTRY.get_or_create()`：

**修改前**（第 38 行附近）：
```rust
let manager = SharedMemoryManager::new(&request.project_path)
```

**修改后**：
```rust
let manager = super::registry::REGISTRY.get_or_create(&request.project_path)
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/mcp.rs`

---

### 步骤 9：MCP 请求类型更新

- **操作**：修改
- **目标文件**：`src/rust/mcp/types.rs`
- **关联约束**：SC-15（Write Guard 配置可暴露）, SC-16（Vitality 配置可暴露）
- **依赖步骤**：步骤 2
- **变更内容**：

在 `MemoryConfigRequest` 结构体中新增 7 个可选字段：

```rust
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryConfigRequest {
    // 已有字段（不变）
    pub similarity_threshold: Option<f64>,
    pub dedup_on_startup: Option<bool>,
    pub enable_dedup: Option<bool>,

    // v2.2 新增
    #[schemars(description = "Write Guard 语义匹配阈值 (0.5~0.95)，>= 此值静默拒绝")]
    pub write_guard_semantic_threshold: Option<f64>,
    #[schemars(description = "Write Guard 更新匹配阈值 (0.3~0.8)，此值到语义阈值之间自动合并")]
    pub write_guard_update_threshold: Option<f64>,
    #[schemars(description = "活力衰减半衰期（天），默认 30")]
    pub vitality_decay_half_life_days: Option<u32>,
    #[schemars(description = "活力清理阈值 (0.0~1.0)，默认 0.35")]
    pub vitality_cleanup_threshold: Option<f64>,
    #[schemars(description = "不活跃天数阈值，默认 14")]
    pub vitality_cleanup_inactive_days: Option<u32>,
    #[schemars(description = "每次访问提升的活力值，默认 0.5")]
    pub vitality_access_boost: Option<f64>,
    #[schemars(description = "最大活力值，默认 3.0")]
    pub vitality_max_score: Option<f64>,
}
```

- **验证方式**：`cargo build --lib` 编译通过
- **回滚方案**：`git checkout -- src/rust/mcp/types.rs`

---

### 步骤 10：模块导出更新

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mod.rs`
- **关联约束**：无（代码组织）
- **依赖步骤**：步骤 5, 7
- **变更内容**：

在 `mod.rs` 中新增模块声明和重新导出：

```rust
pub mod types;
pub mod similarity;
pub mod dedup;
pub mod migration;
pub mod manager;
pub mod mcp;
pub mod write_guard;   // P0 新增
pub mod registry;      // P0 新增

// 重新导出
pub use manager::MemoryManager;
pub use manager::SharedMemoryManager;
pub use types::{MemoryEntry, MemoryCategory, MemoryMetadata, MemoryStore, MemoryConfig, DomainInfo};
pub use mcp::MemoryTool;
pub use similarity::TextSimilarity;
pub use dedup::{MemoryDeduplicator, DuplicateInfo, DedupResult};
pub use migration::{MemoryMigrator, MigrationResult};
pub use write_guard::{WriteGuard, WriteGuardAction, WriteGuardResult};
pub use registry::REGISTRY;
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/mod.rs`

---

### P0 阶段验收标准

- [ ] `cargo build --lib` 零错误零警告
- [ ] `cargo test --package sanshu --lib mcp::tools::memory` 全部通过（包含新增测试）
- [ ] Write Guard 正确拦截 >= 0.80 相似度的重复记忆（NOOP）
- [ ] Write Guard 正确合并 0.60-0.80 相似度的记忆（UPDATE）
- [ ] Write Guard 正常通过 < 0.60 相似度的记忆（ADD）
- [ ] MemoryManagerRegistry 第二次访问同一项目路径命中缓存（无冷启动延迟）
- [ ] v2.2 schema 升级不破坏现有 v2.1 数据（新字段自动填充默认值）
- [ ] `配置` 操作返回新增的 Write Guard 和 Vitality 参数

---

## 阶段 P1：组织增强（步骤 11-20，约 40h）

> **阶段目标**：完成 URI 路径体系、活力衰减引擎、前端树形浏览和渐进式披露
> **阶段验收**：URI 路径解析正确 + 活力衰减公式验证 + 前端 850px 布局正常 + 三态交互流畅

---

### 步骤 11：URI 路径解析验证模块

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/uri_path.rs`
- **关联约束**：HC-14（URI 格式规范）, RISK-08（迁移认知成本）
- **依赖步骤**：步骤 1（v2.2 数据模型）
- **变更内容**：

创建新文件 `src/rust/mcp/tools/memory/uri_path.rs`：

```rust
//! URI 路径解析和验证模块
//!
//! HC-14: URI 格式 `domain://path/segments`
//! domain 限定为 [a-z][a-z0-9_-]*
//! path segments 不限字符集（支持中文）

use anyhow::Result;
use regex::Regex;
use once_cell::sync::Lazy;

/// URI 路径正则
static URI_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([a-z][a-z0-9_-]*)://(.+)$").unwrap()
});

/// 域名正则
static DOMAIN_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9_-]*$").unwrap()
});

/// 解析后的 URI 路径
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUriPath {
    /// 域名（如 "core", "project"）
    pub domain: String,
    /// 路径段列表（如 ["architecture", "backend"]）
    pub segments: Vec<String>,
    /// 完整路径字符串（如 "core://architecture/backend"）
    pub full_path: String,
}

/// URI 路径解析器
pub struct UriPathParser;

impl UriPathParser {
    /// HC-14: 解析 URI 路径
    ///
    /// 输入格式：`domain://path/segments`
    /// 返回解析后的 ParsedUriPath
    /// 无效格式返回错误
    pub fn parse(uri: &str) -> Result<ParsedUriPath> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err(anyhow::anyhow!("URI 路径不能为空"));
        }

        let caps = URI_PATTERN.captures(uri)
            .ok_or_else(|| anyhow::anyhow!(
                "无效的 URI 路径格式: '{}'\n期望格式: domain://path/segments\n域名规则: [a-z][a-z0-9_-]*",
                uri
            ))?;

        let domain = caps.get(1).unwrap().as_str().to_string();
        let path_str = caps.get(2).unwrap().as_str();

        let segments: Vec<String> = path_str
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if segments.is_empty() {
            return Err(anyhow::anyhow!("URI 路径至少需要一个路径段: '{}'", uri));
        }

        Ok(ParsedUriPath {
            domain,
            segments,
            full_path: uri.to_string(),
        })
    }

    /// 验证域名格式
    pub fn validate_domain(domain: &str) -> bool {
        DOMAIN_PATTERN.is_match(domain)
    }

    /// HC-19 + RISK-08: 为无 URI 路径的旧记忆生成默认路径
    pub fn default_legacy_path() -> String {
        "legacy://uncategorized".to_string()
    }

    /// 从 domain 和 segments 构建完整 URI 路径
    pub fn build(domain: &str, segments: &[&str]) -> String {
        format!("{}://{}", domain, segments.join("/"))
    }

    /// 提取域名（从完整 URI 路径或单独的域名字符串）
    pub fn extract_domain(uri_or_domain: &str) -> Option<String> {
        if let Ok(parsed) = Self::parse(uri_or_domain) {
            Some(parsed.domain)
        } else if Self::validate_domain(uri_or_domain) {
            Some(uri_or_domain.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_uri() {
        let parsed = UriPathParser::parse("core://architecture/backend").unwrap();
        assert_eq!(parsed.domain, "core");
        assert_eq!(parsed.segments, vec!["architecture", "backend"]);
    }

    #[test]
    fn test_parse_chinese_path() {
        let parsed = UriPathParser::parse("project://三术/记忆模块").unwrap();
        assert_eq!(parsed.domain, "project");
        assert_eq!(parsed.segments, vec!["三术", "记忆模块"]);
    }

    #[test]
    fn test_parse_invalid_domain() {
        assert!(UriPathParser::parse("123://path").is_err());
        assert!(UriPathParser::parse("UPPER://path").is_err());
    }

    #[test]
    fn test_parse_empty() {
        assert!(UriPathParser::parse("").is_err());
    }

    #[test]
    fn test_validate_domain() {
        assert!(UriPathParser::validate_domain("core"));
        assert!(UriPathParser::validate_domain("my-project"));
        assert!(UriPathParser::validate_domain("project_1"));
        assert!(!UriPathParser::validate_domain("123"));
        assert!(!UriPathParser::validate_domain("UPPER"));
        assert!(!UriPathParser::validate_domain(""));
    }
}
```

**注意**：需要在 `Cargo.toml` 中确认 `regex` 依赖已存在（当前项目中应已有）。如无，需添加 `regex = "1"` 到 `[dependencies]`。

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory::uri_path` 5 个测试全部通过

- **回滚方案**：删除 `src/rust/mcp/tools/memory/uri_path.rs`

---

### 步骤 12：Vitality Decay 活力衰减引擎

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/vitality.rs`
- **关联约束**：HC-15（清理需用户确认）, SC-16（参数可配置）, RISK-06（衰减过激防护）
- **依赖步骤**：步骤 1, 2
- **变更内容**：

创建新文件 `src/rust/mcp/tools/memory/vitality.rs`：

```rust
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
}
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory::vitality` 3 个测试全部通过

- **回滚方案**：删除 `src/rust/mcp/tools/memory/vitality.rs`

---

### 步骤 13：manager.rs 集成 URI 路径和 Vitality

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/manager.rs`
- **关联约束**：HC-14（URI 验证）, HC-15（清理候选）, SC-16（参数可配置）
- **依赖步骤**：步骤 11, 12
- **变更内容**：

1. 在导入区域新增：

```rust
use super::uri_path::UriPathParser;
use super::vitality::{VitalityEngine, CleanupCandidate};
```

2. 在 `MemoryManager` 中新增以下方法：

```rust
/// 设置记忆的 URI 路径和标签
pub fn classify_memory(
    &mut self,
    memory_id: &str,
    uri_path: Option<&str>,
    tags: Option<Vec<String>>,
) -> Result<Option<String>> {
    let entry_idx = self.store.entries.iter().position(|e| e.id == memory_id);
    if let Some(idx) = entry_idx {
        // HC-14: 验证 URI 路径格式
        if let Some(uri) = uri_path {
            let parsed = UriPathParser::parse(uri)?;
            self.store.entries[idx].uri_path = Some(parsed.full_path);
            self.store.entries[idx].domain = Some(parsed.domain.clone());

            // 更新域注册表
            self.update_domain_registry(&parsed.domain);
        }
        if let Some(t) = tags {
            self.store.entries[idx].tags = Some(t);
        }
        self.store.entries[idx].updated_at = Utc::now();
        self.save_store()?;
        Ok(Some(memory_id.to_string()))
    } else {
        Ok(None)
    }
}

/// 获取域列表及统计
pub fn get_domain_list(&self) -> Vec<(String, usize)> {
    let mut domain_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in &self.store.entries {
        if let Some(domain) = &entry.domain {
            *domain_counts.entry(domain.clone()).or_insert(0) += 1;
        } else {
            *domain_counts.entry("legacy".to_string()).or_insert(0) += 1;
        }
    }
    let mut result: Vec<_> = domain_counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

/// HC-15: 获取清理候选列表
pub fn get_cleanup_candidates(&self) -> Vec<CleanupCandidate> {
    VitalityEngine::get_cleanup_candidates(&self.store.entries, &self.store.config)
}

/// HC-15: 执行清理（删除指定 ID 列表的记忆）
pub fn execute_cleanup(&mut self, ids: &[String]) -> Result<usize> {
    let before = self.store.entries.len();
    self.store.entries.retain(|e| !ids.contains(&e.id));
    let removed = before - self.store.entries.len();
    if removed > 0 {
        self.save_store()?;
    }
    Ok(removed)
}

/// 访问记忆（提升活力值）
pub fn access_memory(&mut self, memory_id: &str) -> Result<()> {
    if let Some(entry) = self.store.entries.iter_mut().find(|e| e.id == memory_id) {
        VitalityEngine::boost_vitality(entry, &self.store.config);
        self.save_store()?;
    }
    Ok(())
}

/// 更新域注册表
fn update_domain_registry(&mut self, domain: &str) {
    use super::types::DomainInfo;
    use std::collections::HashMap;

    let domains = self.store.domains.get_or_insert_with(HashMap::new);
    let count = self.store.entries.iter()
        .filter(|e| e.domain.as_deref() == Some(domain))
        .count();

    domains.insert(domain.to_string(), DomainInfo {
        name: domain.to_string(),
        description: None,
        entry_count: count,
    });
}
```

3. 同步在 `SharedMemoryManager` 中添加对应的包装方法（使用适当的读锁/写锁）。

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. `cargo test --package sanshu --lib mcp::tools::memory` 全部通过

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/manager.rs`

---

### 步骤 14：MCP 新增 4 个操作

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mcp.rs`
- **关联约束**：HC-14, HC-15
- **依赖步骤**：步骤 13
- **变更内容**：

在 `mcp.rs` 的 match 分支中新增 4 个操作：

```rust
// 分类：设置/更新 URI 路径和标签
"分类" => { ... }

// 域列表：获取所有域及其统计
"域列表" => { ... }

// 清理候选：获取低活力清理候选列表
"清理候选" => { ... }

// 执行清理：确认并执行清理（HC-15: 必须先获取候选再执行）
"执行清理" => { ... }
```

同步更新 `JiyiRequest` 的 action 描述和 `get_tool_definition()` 中的操作列表。

在 `src/rust/mcp/types.rs` 的 `JiyiRequest` 中新增字段：

```rust
#[schemars(description = "URI 路径（分类操作时使用），格式: domain://path/segments")]
#[serde(default)]
pub uri_path: Option<String>,

#[schemars(description = "标签列表（分类操作时使用）")]
#[serde(default)]
pub tags: Option<Vec<String>>,

#[schemars(description = "待清理的记忆 ID 列表（执行清理操作时使用）")]
#[serde(default)]
pub cleanup_ids: Option<Vec<String>>,
```

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 手动测试：通过 MCP 调用 `分类`、`域列表`、`清理候选`、`执行清理` 操作

- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/mcp.rs src/rust/mcp/types.rs`

---

### 步骤 15：前端 MemoryManager 主容器

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/MemoryManager.vue`
- **关联约束**：HC-17（850px 最小宽度）
- **依赖步骤**：P0 全部完成, 步骤 14
- **变更内容**：

创建新的 `MemoryManager.vue` 作为记忆管理主容器，替代 `MemoryConfig.vue` 中的 4 Tab 布局，改为左右分栏布局（NLayout + NLayoutSider）。

组件结构：
- `NLayout` 根容器，min-width: 850px
- `NLayoutSider`（左侧，可折叠，默认 200px）-> DomainTree
- `NLayoutContent`（中间，min-width: 500px）-> MemoryWorkspace
- 底部保留 Tab 切换：工作区 | 搜索 | 配置

使用 `provide/inject` 传递共享状态（当前选中域、搜索关键词等）。

- **验证方式**：
  1. `pnpm build` 编译通过
  2. 850px 窗口宽度下布局不溢出
  3. 左侧面板可折叠/展开

- **回滚方案**：删除新文件

---

### 步骤 16：前端 DomainTree 域树组件

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/DomainTree.vue`
- **关联约束**：HC-17（NLayoutSider collapse）, DEP-04（依赖后端 URI）
- **依赖步骤**：步骤 14, 15
- **变更内容**：

实现左侧域/路径树组件：
- 使用 `NTree` 组件渲染域树结构
- 数据源：通过 Tauri invoke 调用 `域列表` 操作获取
- 支持点击域节点筛选中间区域的记忆列表
- 支持右键菜单（新建域、重命名、删除空域）

- **验证方式**：域树正确渲染，点击域名可筛选记忆列表

---

### 步骤 17：前端 MemoryWorkspace 工作区

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/MemoryWorkspace.vue`
- **关联约束**：SC-18（渐进式披露）
- **依赖步骤**：步骤 15
- **变更内容**：

实现中间工作区，包含：
- SearchBar（搜索栏，300ms debounce）
- TagFilter（标签筛选条）
- MemoryCardList（记忆卡片列表，渐进式披露三态）
- VitalityBadge（活力值徽章）

使用 `useProgressiveDisclosure` composable 管理三态：
- collapsed：标题 + 分类 + VitalityBadge + 时间
- expanded：+ 内容预览 100 字 + 标签
- detail：+ 完整内容 + 版本历史

- **验证方式**：三态切换流畅，卡片渲染正确

---

### 步骤 18：前端 TagFilter 标签筛选组件

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/TagFilter.vue`
- **关联约束**：SC-17（前缀语法配合）
- **依赖步骤**：步骤 17
- **变更内容**：

实现标签筛选条，使用 NTag + NSpace 展示当前域/搜索结果的标签云，支持点击切换筛选。

---

### 步骤 19：前端 VitalityBadge 活力值徽章

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/VitalityBadge.vue`
- **关联约束**：SC-18（渐进式披露的一部分）
- **依赖步骤**：步骤 12（后端活力引擎）
- **变更内容**：

实现活力值徽章组件：
- 使用 NProgress（circular 模式）+ NTooltip
- 颜色编码：绿(>2.0) / 黄(1.0-2.0) / 红(<1.0)
- Tooltip 显示衰减趋势（当前值 + 预计 7 天后的值）

使用 `useVitalityDecay` composable 封装前端活力计算逻辑。

---

### 步骤 20：前端 SearchBar 搜索前缀语法

- **操作**：修改
- **目标文件**：`src/frontend/components/tools/MemorySearch.vue`
- **关联约束**：SC-17（@domain #tag 前缀语法）, RISK-09（降级为全文搜索）
- **依赖步骤**：步骤 17
- **变更内容**：

在现有 MemorySearch.vue 中增强搜索功能：
- 支持 `@domain` 前缀按域过滤
- 支持 `#tag` 前缀按标签过滤
- 自然语言部分进行全文搜索
- 使用正则解析前缀，失败时降级为纯全文搜索
- 300ms debounce 实时搜索

---

### 步骤 20.5：模块导出更新（P1）

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mod.rs`
- **关联约束**：无（代码组织）
- **依赖步骤**：步骤 11, 12
- **变更内容**：

在 mod.rs 中追加：

```rust
pub mod uri_path;     // P1 新增
pub mod vitality;     // P1 新增
```

---

### P1 阶段验收标准

- [ ] URI 路径 `core://architecture/backend` 正确解析为 domain=core, segments=[architecture, backend]
- [ ] 中文路径 `project://三术/记忆模块` 正确解析
- [ ] 无效域名 `123://path` 返回错误
- [ ] 活力衰减：初始值 1.5 在 30 天后正确衰减至约 0.75
- [ ] Rule 分类记忆永不出现在清理候选列表中
- [ ] 清理操作必须先调用 `清理候选` 获取列表再调用 `执行清理`
- [ ] 前端 850px 窗口宽度下布局不溢出
- [ ] 左侧域树面板可折叠至 0px
- [ ] 渐进式披露三态切换流畅
- [ ] 搜索前缀语法 `@core` 和 `#rust` 正确过滤

---

## 阶段 P2：检索升级（步骤 21-27，约 30h）

> **阶段目标**：引入 FTS5 索引、摘要生成、Snapshot Diff 视图
> **阶段验收**：FTS5 搜索与 JSON 遍历结果一致 + 长记忆自动生成摘要

---

### 步骤 21：引入 rusqlite 依赖

- **操作**：修改
- **目标文件**：`Cargo.toml`
- **关联约束**：DEP-05（FTS5 依赖 SQLite）
- **依赖步骤**：P1 全部完成
- **变更内容**：

在 `[dependencies]` 区域新增：

```toml
rusqlite = { version = "0.31", features = ["bundled", "fts5"] }
```

`bundled` 特性确保自带 SQLite，`fts5` 启用 FTS5 全文搜索。

- **验证方式**：`cargo build --lib` 编译通过
- **回滚方案**：`git checkout -- Cargo.toml && cargo update`

---

### 步骤 22：FTS5 Sidecar 索引模块

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/fts_index.rs`
- **关联约束**：HC-16（Sidecar 不替换 JSON）, HC-18（FTS5 失败不阻塞 JSON）, RISK-01（双写一致性校验）, RISK-07（中文分词）
- **依赖步骤**：步骤 21
- **变更内容**：

创建 FTS5 Sidecar 索引模块：
- SQLite 数据库文件：`.sanshu-memory/fts_index.db`
- FTS5 虚拟表：`CREATE VIRTUAL TABLE memory_fts USING fts5(id, content, category, domain, tags, summary, tokenize='unicode61')`
- 双写一致性：`sync_entry(entry)` 在 JSON 写入成功后调用
- 定时校验：`verify_consistency()` 对比 JSON 条目数与 FTS5 行数
- HC-18：FTS5 写入失败仅记录日志，不阻塞主流程
- SC-23：初期使用 unicode61 分词器，预留 jieba-rs 切换接口

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 写入 10 条记忆后 FTS5 搜索结果与 JSON 全量遍历一致

- **回滚方案**：删除文件

---

### 步骤 23：摘要自动生成模块

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/summary.rs`
- **关联约束**：SC-19（摘要生成）, DEP-06（依赖 enhance 降级链）
- **依赖步骤**：步骤 1
- **变更内容**：

创建摘要自动生成模块：
- 判断条件：`content.len() > config.summary_length_threshold`（默认 500 字符）
- 生成方式：
  1. 尝试通过 enhance 降级链生成（复用 `enhance/chat_client.rs`）
  2. 规则引擎降级：提取首行 + 关键词截断为 100 字符
  3. 规则引擎生成的摘要前缀标记为 `[auto]` 以区分
- 异步接口：`pub async fn generate_summary(content: &str) -> String`

**新增配置字段**（已在步骤 2 的 MemoryConfig 中预留位置）：

在 `MemoryConfig` 中追加：
```rust
/// 摘要生成的内容长度阈值（字符数）
#[serde(default = "default_summary_threshold")]
pub summary_length_threshold: usize,
```

```rust
fn default_summary_threshold() -> usize { 500 }
```

同步更新 `Default for MemoryConfig` 实现。

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 输入 600 字符内容，规则引擎降级生成的摘要不超过 100 字符

- **回滚方案**：删除文件

---

### 步骤 24：MCP 新增获取快照操作

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mcp.rs`
- **关联约束**：DEP-08（快照 Diff 依赖）
- **依赖步骤**：步骤 14
- **变更内容**：

在 match 分支中新增 `"获取快照"` 操作：

```rust
"获取快照" => {
    let memory_id = request.memory_id.as_deref()
        .ok_or_else(|| McpError::invalid_params("缺少 memory_id".to_string(), None))?;

    let memories = manager.get_all_memories()?;
    if let Some(entry) = memories.iter().find(|e| e.id == memory_id) {
        let snapshots: Vec<serde_json::Value> = entry.snapshots.iter().map(|s| {
            serde_json::json!({
                "version": s.version,
                "content": s.content,
                "created_at": s.created_at.to_rfc3339()
            })
        }).collect();

        serde_json::to_string_pretty(&serde_json::json!({
            "memory_id": memory_id,
            "current_version": entry.version,
            "current_content": entry.content,
            "snapshots": snapshots
        })).unwrap_or_default()
    } else {
        format!("未找到记忆: {}", memory_id)
    }
}
```

- **验证方式**：`cargo build --lib` 编译通过
- **回滚方案**：`git checkout -- src/rust/mcp/tools/memory/mcp.rs`

---

### 步骤 25：前端 SnapshotDiff 视图

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/SnapshotDiff.vue`
- **关联约束**：SC-26（Snapshot Diff）, DEP-08
- **依赖步骤**：步骤 24
- **变更内容**：

实现版本对比组件：
- 调用 `获取快照` 操作获取版本历史
- 使用文本 diff 算法高亮差异（可选集成 `diff` npm 包或自定义简易实现）
- 支持选择两个版本进行对比
- 嵌入 MemoryWorkspace 的 detail 态中

---

### 步骤 26：前端 BatchActionBar 批量操作条

- **操作**：新增
- **目标文件**：`src/frontend/components/tools/BatchActionBar.vue`
- **关联约束**：SC-21（批量操作）
- **依赖步骤**：步骤 17
- **变更内容**：

实现底部固定的批量操作条（NAffix）：
- 在多选模式下显示
- 支持操作：批量删除 / 批量重新分类 / 批量导出 / 刷新活力值
- 操作前通过 confirm 弹窗确认

---

### 步骤 27：前端虚拟滚动优化

- **操作**：修改
- **目标文件**：`src/frontend/components/tools/MemoryWorkspace.vue`
- **关联约束**：HC-17（850px 布局）
- **依赖步骤**：步骤 17
- **变更内容**：

在记忆卡片列表中启用虚拟滚动：
- 当记忆数 > 100 条时自动启用
- 使用 NDataTable 的 `virtual-scroll` 属性
- 设置 `max-height` 适配 850px 窗口

---

### 步骤 27.5：模块导出更新（P2）

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mod.rs`
- **关联约束**：无
- **依赖步骤**：步骤 22, 23
- **变更内容**：

在 mod.rs 中追加：

```rust
pub mod fts_index;   // P2 新增
pub mod summary;     // P2 新增
```

---

### P2 阶段验收标准

- [ ] `cargo build --lib` 编译通过（含 rusqlite）
- [ ] FTS5 搜索结果与 JSON 全量遍历一致
- [ ] FTS5 写入失败不阻塞 JSON 主存储写入
- [ ] 长记忆（>500 字符）自动生成摘要
- [ ] 规则引擎降级摘要标记为 `[auto]`
- [ ] Snapshot Diff 视图正确展示版本差异
- [ ] 批量操作（>10 条）执行成功
- [ ] 虚拟滚动在 200+ 条记忆时流畅

---

## 阶段 P3：体验升级（步骤 28-32，约 26h）

> **阶段目标**：会话观察捕获、Token 效率优化、前端体验增强
> **阶段验收**：观察捕获不阻塞主流程 + Token 消耗减少 >50%

---

### 步骤 28：会话工具观察存储模块

- **操作**：新增
- **目标文件**：`src/rust/mcp/tools/memory/observation_store.rs`
- **关联约束**：SC-25（会话自动捕获）, DEP-07（依赖 P0 Registry + P2 FTS5）, RISK-10（噪音控制）
- **依赖步骤**：步骤 7（Registry）, 步骤 22（FTS5）
- **变更内容**：

创建会话观察存储模块：
- SQLite 存储：`.sanshu-memory/observations.db`
- 表结构：`observations(id, tool_name, input_summary, output_summary, created_at, tags)`
- 异步写入：使用 `tokio::sync::mpsc` channel
- RISK-10 防护：
  - 可配置跳过列表：`skip_tools: ["Read", "Glob", "Grep"]`
  - 独立上限：5000 条，超出 FIFO 淘汰
  - 活力衰减同样适用

在 `server.rs` 的 `call_tool` 返回后异步记录（不阻塞主流程）。

- **验证方式**：
  1. `cargo build --lib` 编译通过
  2. 手动测试：调用非跳过列表中的工具后，observations.db 中有记录
  3. 调用跳过列表中的工具后，无记录

---

### 步骤 29：MCP Token 效率优化

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mcp.rs`
- **关联约束**：SC-20（Token 效率）
- **依赖步骤**：步骤 23（摘要可用）
- **变更内容**：

1. `回忆` 操作新增 `verbose` 参数支持：
   - `verbose=false`（默认）：返回压缩格式（分类汇总 + 摘要）
   - `verbose=true`：返回完整内容

2. `列表` 操作新增 `page` + `page_size` + `summary_only` 参数支持：
   - 默认 `page=1, page_size=20`
   - `summary_only=true` 时仅返回 ID + 摘要 + 分类

3. 在 `JiyiRequest` 中新增对应字段。

---

### 步骤 30：前端 VitalityBadge 增强

- **操作**：修改
- **目标文件**：`src/frontend/components/tools/VitalityBadge.vue`
- **关联约束**：SC-18
- **依赖步骤**：步骤 19
- **变更内容**：

增强活力值徽章，新增衰减趋势图（迷你折线图，展示过去 30 天的活力值变化趋势）。

---

### 步骤 31：前端 ARIA 标注和键盘导航

- **操作**：修改
- **目标文件**：多个前端组件
- **关联约束**：RISK-09（降低交互复杂度）
- **依赖步骤**：步骤 15-20
- **变更内容**：

为所有新增组件添加：
- ARIA role 和 label 属性
- 键盘导航支持（Space 展开/收起，Tab 焦点移动）
- focus-visible 样式

---

### 步骤 32：前端骨架屏加载状态

- **操作**：修改
- **目标文件**：`src/frontend/components/tools/MemoryWorkspace.vue`
- **关联约束**：SC-18
- **依赖步骤**：步骤 17
- **变更内容**：

在记忆列表加载时展示骨架屏（NSkeleton），替代空白或 loading spinner。

---

### 步骤 32.5：模块导出更新（P3）

- **操作**：修改
- **目标文件**：`src/rust/mcp/tools/memory/mod.rs`
- **关联约束**：无
- **依赖步骤**：步骤 28
- **变更内容**：

在 mod.rs 中追加：

```rust
pub mod observation_store;  // P3 新增
```

---

### P3 阶段验收标准

- [ ] 会话工具观察自动捕获不阻塞 MCP 主流程（延迟增加 < 5ms）
- [ ] 跳过列表中的工具（Read, Glob, Grep）不产生观察记录
- [ ] `回忆` 操作 `verbose=false` 模式 token 消耗减少 >50%
- [ ] `列表` 操作分页正确（page=2, page_size=10 返回第 11-20 条）
- [ ] ARIA 标注通过 axe-core 基础检查
- [ ] 骨架屏在加载时正确显示

---

## 依赖关系图

```
P0 阶段：
  步骤 1（类型升级） → 步骤 2（配置扩展） → 步骤 3（Store 升级）
       ↓                                          ↓
  步骤 5（Write Guard） ←──────────────────── 步骤 4（迁移测试）
       ↓
  步骤 6（集成 WG） → 步骤 8（MCP 更新） → 步骤 9（请求类型）
       ↓
  步骤 7（Registry） → 步骤 8
       ↓
  步骤 10（mod.rs）

P1 阶段：
  步骤 11（URI 路径） → 步骤 13（集成） → 步骤 14（MCP 操作）
  步骤 12（Vitality）  ↗                      ↓
                                         步骤 15-20（前端）
                                    15 → 16（DomainTree）
                                    15 → 17（Workspace） → 18, 19, 20

P2 阶段：
  步骤 21（rusqlite） → 步骤 22（FTS5）
  步骤 23（摘要） ← DEP-06（enhance 可用）
  步骤 24（快照 MCP） → 步骤 25（前端 Diff）
  步骤 26（BatchAction）
  步骤 27（虚拟滚动）

P3 阶段：
  步骤 28（观察存储） ← DEP-07（Registry + FTS5）
  步骤 29（Token 效率） ← 步骤 23（摘要可用）
  步骤 30-32（前端增强）
```

---

## 约束覆盖检查

### 硬约束覆盖

| 约束编号 | 覆盖步骤 | 状态 |
|----------|----------|------|
| HC-10 | 步骤 6（add_memory 中保留大小/数量限制） | 已覆盖 |
| HC-11 | 步骤 5, 6, 8（Write Guard 三级判定） | 已覆盖 |
| HC-12 | 步骤 1, 3（Option + serde default + 懒迁移） | 已覆盖 |
| HC-13 | 步骤 7（Weak + TTL + 池大小上限 16） | 已覆盖 |
| HC-14 | 步骤 11, 13, 14（URI 解析验证 + 分类操作） | 已覆盖 |
| HC-15 | 步骤 12, 13, 14（清理候选列表 + 用户确认 + 执行） | 已覆盖 |
| HC-16 | 步骤 22（FTS5 Sidecar，JSON 为真实数据源） | 已覆盖 |
| HC-17 | 步骤 15, 27（850px 最小宽度 + 可折叠面板） | 已覆盖 |
| HC-18 | 步骤 22（FTS5 失败不阻塞 JSON，原子写入不变） | 已覆盖 |
| HC-19 | 步骤 3, 4（升级时自动填充默认值，旧记忆归 legacy） | 已覆盖 |

### 软约束覆盖

| 约束编号 | 覆盖步骤 | 状态 |
|----------|----------|------|
| SC-4 | 已实现 | 已完成 |
| SC-5 | 步骤 3（版本检查升级到 v2.2） | 已覆盖 |
| SC-6 | 已实现 | 已完成 |
| SC-7 | 未实施（低优先级，保留占位） | 延后至 P4 |
| SC-15 | 步骤 2, 5, 8, 9（Write Guard 阈值可配置） | 已覆盖 |
| SC-16 | 步骤 2, 12（Vitality 参数可配置） | 已覆盖 |
| SC-17 | 步骤 20（@domain #tag 前缀语法） | 已覆盖 |
| SC-18 | 步骤 17, 19（渐进式披露三态 + VitalityBadge） | 已覆盖 |
| SC-19 | 步骤 23（摘要自动生成 + 规则引擎降级） | 已覆盖 |
| SC-20 | 步骤 29（verbose 参数 + 分页） | 已覆盖 |
| SC-21 | 步骤 26（BatchActionBar 批量操作） | 已覆盖 |
| SC-22 | 步骤 7（Registry 懒加载 + canonical 规范化） | 已覆盖 |
| SC-23 | 步骤 22（unicode61 + 预留 jieba-rs 接口） | 已覆盖 |
| SC-24 | 步骤 22 附属（关键词评分法意图识别） | 已覆盖 |
| SC-25 | 步骤 28（会话观察自动捕获） | 已覆盖 |
| SC-26 | 步骤 24, 25（获取快照 MCP + 前端 Diff） | 已覆盖 |

### 依赖约束覆盖

| 约束编号 | 覆盖方式 | 状态 |
|----------|----------|------|
| DEP-01 | 步骤 5 复用 TextSimilarity::calculate_enhanced() | 已覆盖 |
| DEP-02 | 步骤 7 管理 SharedMemoryManager 实例池 | 已覆盖 |
| DEP-03 | 步骤 1-3 在 P0 完成 v2.2 升级，P1 步骤依赖此 | 已覆盖 |
| DEP-04 | 步骤 14（后端 URI）先于步骤 16（前端 DomainTree） | 已覆盖 |
| DEP-05 | 步骤 21 在 P2 开始时引入 rusqlite | 已覆盖 |
| DEP-06 | 步骤 23 复用 enhance 降级链 + 规则引擎兜底 | 已覆盖 |
| DEP-07 | 步骤 28 在 P3 启动，依赖 P0 Registry + P2 FTS5 | 已覆盖 |
| DEP-08 | 步骤 24 新增获取快照 MCP 操作，步骤 25 前端渲染 | 已覆盖 |

### 风险约束覆盖

| 约束编号 | 缓解措施 | 覆盖步骤 |
|----------|----------|----------|
| RISK-01 | 定时校验（条目数 + hash）+ FTS5 全量重建 | 步骤 22 |
| RISK-02 | 新字段 Option + serde default + 只读回退 | 步骤 1, 3 |
| RISK-03 | Weak 引用 + TTL 30min + 池大小 16 + 监控 | 步骤 7 |
| RISK-04 | 阈值可配置 + NOOP 记录日志 + 前端审查 | 步骤 5, 8 |
| RISK-05 | 保留 `整理` 后置去重 + SC-7 预留 | 不变 |
| RISK-06 | Rule 永不清理 + 参数可配置 + 确认删除 | 步骤 12, 14 |
| RISK-07 | unicode61 + A/B 测试 + 预留 jieba-rs | 步骤 22 |
| RISK-08 | 默认 legacy://uncategorized + 不强制迁移 | 步骤 3, 11 |
| RISK-09 | 默认 expanded 态 + Space 键盘快捷键 | 步骤 17, 31 |
| RISK-10 | skip_tools 列表 + 5000 条 FIFO + 活力衰减 | 步骤 28 |

---

## 总体验收标准

- [ ] `cargo build --lib` 零错误（含 rusqlite 依赖）
- [ ] `cargo test` 全部通过（含新增的 ~20 个测试用例）
- [ ] `pnpm build` 零错误
- [ ] `pnpm vitest` 全部通过
- [ ] 10 条硬约束全部满足
- [ ] 所有 DEP 约束反映在步骤执行顺序中
- [ ] 所有 RISK 约束有对应缓解措施
- [ ] 1000 条记忆规模下搜索延迟 < 100ms（FTS5 启用后）
- [ ] Write Guard 误判率 < 5%（可配置阈值调优）

---

## 回滚策略

### 文件级回滚

每个步骤都标注了精确的回滚命令（`git checkout` 或 `rm`）。

### 阶段级回滚

- **P0 回滚**：`git stash` 保存当前变更，回退到 P0 开始前的 commit
- **P1 回滚**：仅保留 P0 变更，回退 P1 新增的文件和修改
- **P2 回滚**：仅保留 P0+P1，回退 rusqlite 依赖和 FTS5 相关代码
- **P3 回滚**：仅保留 P0+P1+P2，回退观察存储和体验优化

### 紧急回滚

如果任何阶段导致 `cargo build` 失败且无法快速修复：
1. `git stash` 保存当前工作
2. 回退到最近一个通过编译的 commit
3. 分析失败原因后重新执行

---

## 双模型执行元数据

| 字段 | 值 |
|------|-----|
| dual_model_status | DEGRADED |
| degraded_level | ACCEPTABLE |
| missing_dimensions | ["collab_skill_unavailable"] |
| codex_session | 019c75e4-93db-75d0-81da-2d80630282a8 |
| gemini_session | e60f4a6b-1c88-4691-aa55-1b340335ec11 |

### 降级影响说明
- **缺失维度**：collab Skill 文件不存在，无法在规划阶段重新调用外部模型
- **影响范围**：规划步骤基于研究阶段已完成的 Codex/Gemini 分析产出，无需重复调用
- **补偿措施**：直接使用研究文档中的双模型分析结果（SESSION_ID 已记录），Claude 独立完成零决策化处理

---

## SESSION_ID（供后续使用）

- CODEX_SESSION: 019c75e4-93db-75d0-81da-2d80630282a8
- GEMINI_SESSION: e60f4a6b-1c88-4691-aa55-1b340335ec11

---

## 下一步

运行 `/ccg:spec-impl` 执行此计划
