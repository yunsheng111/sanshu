# 记忆工具 (memory)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **memory**

---

## 模块职责

全局记忆管理工具 (ji)，提供跨会话的知识存储与检索能力。支持 4 种记忆分类、智能去重、相似度检测、旧格式自动迁移、并发安全访问（SharedMemoryManager）和版本控制（快照/回滚）。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `ji`
- **标识符**: `mcp______ji`
- **状态**: 默认启用（可禁用）

### 核心结构
```rust
// 并发安全包装器（P0 新增）
pub struct SharedMemoryManager {
    inner: Arc<RwLock<MemoryManager>>,
}

// 底层记忆管理器
pub struct MemoryManager {
    memory_dir: PathBuf,
    project_path: String,
    store: MemoryStore,
    is_non_git_project: bool,
}
```

---

## 对外接口

### 支持的操作

| 操作 | 说明 | 必需参数 |
|------|------|----------|
| `记忆` | 添加新记忆 | `content`, `category` |
| `更新` | 更新已有记忆（自动创建快照） | `memory_id`, `content` |
| `回忆` | 查询记忆 | `content` (查询关键词) |
| `整理` | 去重检测 | 无 |
| `列表` | 列出所有记忆 | 无 |
| `预览相似` | 检测相似度 | `content` |
| `配置` | 获取/更新配置 | `config` (可选) |
| `删除` | 删除记忆 | `memory_id` |
| `分类` | 设置 URI 路径和标签 | `memory_id`, `uri_path`/`tags` (可选) |
| `域列表` | 获取所有域及统计 | 无 |
| `清理候选` | 获取低活力清理候选 | 无 |
| `执行清理` | 删除指定记忆列表 | `cleanup_ids` |
| `活力趋势` | 获取活力值历史趋势 | `memory_id` |
| `快照列表` | 获取记忆快照列表 | `memory_id` |
| `回滚快照` | 回滚到指定快照版本 | `memory_id`, `target_version` |

> **P0 新增**："更新"操作会自动创建旧内容的快照，支持后续回滚。

---

## 关键依赖与配置

### 存储结构（v2.2）
- **位置**: `.sanshu-memory/memories.json`
- **格式**: JSON

```rust
/// 记忆条目（v2.2）
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub content_normalized: String,    // 归一化内容（用于相似度计算）
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,                  // P0 新增：版本号（每次更新递增）
    pub snapshots: Vec<MemorySnapshot>,// P0 新增：历史快照（最多 5 个）

    // P0-P3 新增字段（全部 Option<T>）
    pub uri_path: Option<String>,      // HC-14: URI 路径（domain://path/segments）
    pub domain: Option<String>,        // 域名（从 uri_path 提取）
    pub tags: Option<Vec<String>>,     // 标签列表
    pub vitality_score: Option<f64>,   // HC-15: 活力值（0.0-3.0）
    pub last_accessed_at: Option<DateTime<Utc>>, // 最后访问时间
    pub summary: Option<String>,       // SC-19: 自动摘要（>500字符时生成）
}

/// 记忆快照（P0 新增）
pub struct MemorySnapshot {
    pub version: u32,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// 记忆存储
pub struct MemoryStore {
    pub version: String,               // 当前版本："2.2"
    pub project_path: String,
    pub entries: Vec<MemoryEntry>,
    pub last_dedup_at: DateTime<Utc>,
    pub config: MemoryConfig,
    pub domains: Option<HashMap<String, DomainInfo>>, // 域注册表
}

/// 域信息（P1 新增）
pub struct DomainInfo {
    pub name: String,
    pub description: Option<String>,
    pub entry_count: usize,
    pub created_at: DateTime<Utc>,
}
```

### 记忆分类
```rust
pub enum MemoryCategory {
    Rule,        // 规范规则（支持中文别名："规范"、"规则"）
    Preference,  // 用户偏好（"偏好"）
    Pattern,     // 最佳实践（"模式"、"最佳实践"）
    Context,     // 项目上下文（"背景"、"上下文"）
}
```

### 配置选项
```rust
pub struct MemoryConfig {
    pub enable_dedup: bool,          // 启用去重检测
    pub similarity_threshold: f64,   // 相似度阈值（默认 0.70）
    pub dedup_on_startup: bool,      // 启动时自动去重
}
```

---

## 核心功能

### 1. SharedMemoryManager（P0 新增）

提供并发安全的记忆管理访问。

```rust
impl SharedMemoryManager {
    pub fn new(project_path: &str) -> Result<Self>   // 创建（自动迁移 + 去重）
    pub fn is_non_git_project(&self) -> bool          // 是否为降级模式

    // 所有操作通过 RwLock 保护
    pub fn add_memory(...) -> Result<String>           // 读锁 -> 写锁
    pub fn update_memory(...) -> Result<()>            // 写锁（自动创建快照）
    pub fn search_memories(...) -> Vec<MemoryEntry>    // 读锁
    pub fn delete_memory(...) -> Result<()>            // 写锁
    pub fn auto_deduplicate(...) -> Result<DedupResult>// 写锁
}
```

**原子写入**：所有写操作先写入临时文件 `.memories.json.tmp`，成功后 rename 替换，避免写入中断导致数据损坏。

### 2. 版本控制（P0 新增）

每次"更新"操作自动创建旧内容的快照：

```
更新请求 → 读取当前内容 → 创建 MemorySnapshot → 递增版本号 → 写入新内容
```

- **最大快照数**: 5（超出时淘汰最旧的）
- **快照字段**: version、content、created_at

### 3. 相似度检测 (`similarity.rs`)

三算法加权组合：

| 算法 | 权重 | 说明 |
|------|------|------|
| Levenshtein | 40% | 编辑距离（字符级） |
| Phrase | 40% | 短语匹配（词级） |
| Jaccard | 20% | 集合相似度 |

### 4. 去重检测 (`dedup.rs`)

- **保留规则**: 保留 `updated_at` 最新的记忆
- **删除规则**: 删除相似度超过阈值的旧记忆
- **默认阈值**: 0.70

### 5. 数据迁移 (`migration.rs`)

支持从旧版 Markdown 格式迁移到新版 JSON 格式：

```
旧格式 (rules.md, preferences.md, patterns.md, context.md)
    → 解析 Markdown 列表
    → 转换为 MemoryEntry (v2.1)
    → 自动去重
    → 写入 memories.json
    → 备份旧文件到 backup/
```

**迁移路径**: v1.0 (MD 文件) -> v2.0 (JSON) -> v2.1 (带 version/snapshots)

---

## 数据流程

### 添加记忆
```
AI 请求 → SharedMemoryManager (写锁) → 生成 ID → 原子写入 JSON → 返回 ID
```

### 更新记忆（P0 新增）
```
AI 请求 → SharedMemoryManager (写锁) → 创建快照 → 递增版本 → 原子写入 → 返回确认
```

### 查询记忆
```
AI 请求 → SharedMemoryManager (读锁) → 模糊匹配 → 返回结果
```

### 启动流程
```
SharedMemoryManager::new()
    → 路径规范化（Git 根 / 非 Git 降级）
    → 检测旧格式 → 迁移（如需要）
    → 加载 JSON 存储
    → 启动时去重（如配置启用）
```

---

## 常见问题 (FAQ)

### Q: 如何调整相似度阈值？
A: 调用 `配置` 操作，设置 `similarity_threshold` (0.0-1.0)

### Q: 如何回滚记忆到旧版本？
A: 当前快照信息存储在 `MemoryEntry.snapshots` 中，可通过"更新"操作手动恢复

### Q: 并发安全如何保证？
A: `SharedMemoryManager` 使用 `Arc<RwLock>` 包装，读操作使用读锁，写操作使用写锁

### Q: 原子写入如何工作？
A: 写入 `.memories.json.tmp` 临时文件，成功后 rename 替换正式文件

### Q: 记忆存储在哪里？
A: `.sanshu-memory/memories.json`（Git 根目录或项目根目录）

---

## 相关文件清单

### 核心文件
- `manager.rs` - 记忆管理器 + SharedMemoryManager
- `similarity.rs` - 相似度算法（Levenshtein + Phrase + Jaccard）
- `dedup.rs` - 去重检测器
- `migration.rs` - 旧格式迁移（MD -> JSON v2.1）
- `mcp.rs` - MCP 工具实现（支持"更新"action）
- `types.rs` - 数据类型定义（v2.1，含 version/snapshots）
- `mod.rs` - 模块导出

### 数据文件
- `.sanshu-memory/memories.json` - 记忆存储（v2.1）
- `.sanshu-memory/backup/` - 旧格式备份

---

**最后更新**: 2026-02-19
