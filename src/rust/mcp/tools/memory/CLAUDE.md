# 记忆工具 (memory)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **memory**

---

## 模块职责

全局记忆管理工具 (ji)，提供跨会话的知识存储与检索能力。支持 4 种记忆分类、智能去重、相似度检测和旧格式自动迁移。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `ji`
- **标识符**: `mcp______ji`
- **状态**: 默认启用（可禁用）

### 核心结构
```rust
pub struct MemoryTool;

impl MemoryTool {
    pub async fn jiyi(request: JiyiRequest) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "ji",
  "arguments": {
    "action": "记忆",
    "project_path": "/path/to/project",
    "category": "rule",
    "content": "项目使用 Rust 2021 Edition"
  }
}
```

### 支持的操作
| 操作 | 说明 | 必需参数 |
|------|------|----------|
| `记忆` | 添加新记忆 | `content`, `category` |
| `回忆` | 查询记忆 | `content` (查询关键词) |
| `整理` | 去重检测 | 无 |
| `列表` | 列出所有记忆 | 无 |
| `预览相似` | 检测相似度 | `content` |
| `配置` | 获取/更新配置 | `config` (可选) |
| `删除` | 删除记忆 | `memory_id` |

### 请求参数
```rust
pub struct JiyiRequest {
    /// 操作类型
    pub action: String,

    /// 项目路径（必需）
    pub project_path: String,

    /// 记忆分类（记忆操作时必需）
    pub category: Option<String>,

    /// 记忆内容
    pub content: Option<String>,

    /// 记忆 ID（删除操作时必需）
    pub memory_id: Option<String>,

    /// 配置参数（配置操作时使用）
    pub config: Option<serde_json::Value>,
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 存储结构
- **位置**: `.sanshu-memory/memories.json`
- **格式**: JSON
- **结构**:
```rust
pub struct MemoryStore {
    pub entries: Vec<MemoryEntry>,
    pub config: MemoryConfig,
}

pub struct MemoryEntry {
    pub id: String,
    pub category: MemoryCategory,
    pub content: String,
    pub metadata: MemoryMetadata,
}

pub struct MemoryMetadata {
    pub created_at: String,
    pub updated_at: String,
    pub access_count: u32,
    pub last_accessed_at: Option<String>,
}
```

### 记忆分类
```rust
pub enum MemoryCategory {
    Rule,        // 规范规则
    Preference,  // 用户偏好
    Pattern,     // 最佳实践
    Context,     // 项目上下文
}
```

### 配置选项
```rust
pub struct MemoryConfig {
    /// 启用去重检测
    pub enable_dedup: bool,

    /// 相似度阈值（0.0-1.0）
    pub similarity_threshold: f64,

    /// 启动时自动去重
    pub dedup_on_startup: bool,
}
```

---

## 核心功能

### 1. 相似度检测 (`similarity.rs`)

#### 算法组合
```rust
pub struct TextSimilarity;

impl TextSimilarity {
    /// 计算综合相似度（0.0-1.0）
    pub fn calculate(text1: &str, text2: &str) -> f64 {
        let levenshtein = Self::levenshtein_similarity(text1, text2);
        let phrase = Self::phrase_similarity(text1, text2);
        let jaccard = Self::jaccard_similarity(text1, text2);

        // 加权平均
        levenshtein * 0.4 + phrase * 0.4 + jaccard * 0.2
    }
}
```

#### 算法说明
| 算法 | 权重 | 说明 |
|------|------|------|
| Levenshtein | 40% | 编辑距离（字符级） |
| Phrase | 40% | 短语匹配（词级） |
| Jaccard | 20% | 集合相似度 |

#### 单元测试
```rust
#[test]
fn test_identical_texts() {
    let text = "项目使用 Rust 2021 Edition";
    assert_eq!(TextSimilarity::calculate(text, text), 1.0);
}

#[test]
fn test_similar_texts() {
    let text1 = "项目使用 Rust 2021 Edition";
    let text2 = "项目采用 Rust 2021 版本";
    let similarity = TextSimilarity::calculate(text1, text2);
    assert!(similarity > 0.7);
}
```

### 2. 去重检测 (`dedup.rs`)

#### 去重流程
```rust
pub struct MemoryDeduplicator;

impl MemoryDeduplicator {
    /// 检测重复记忆
    pub fn detect_duplicates(
        entries: &[MemoryEntry],
        threshold: f64
    ) -> Vec<DuplicateInfo> {
        // 1. 两两比较
        // 2. 计算相似度
        // 3. 超过阈值标记为重复
        // 4. 返回重复信息
    }

    /// 自动去重（保留最新）
    pub fn auto_deduplicate(
        entries: Vec<MemoryEntry>,
        threshold: f64
    ) -> DedupResult {
        // 1. 检测重复
        // 2. 保留最新记忆
        // 3. 删除旧记忆
        // 4. 返回去重结果
    }
}
```

#### 去重策略
- **保留规则**: 保留 `updated_at` 最新的记忆
- **删除规则**: 删除相似度超过阈值的旧记忆
- **默认阈值**: 0.70

#### 单元测试
```rust
#[test]
fn test_detect_duplicates() {
    let entries = vec![
        create_entry("项目使用 Rust 2021 Edition"),
        create_entry("项目采用 Rust 2021 版本"),
    ];

    let duplicates = MemoryDeduplicator::detect_duplicates(&entries, 0.7);
    assert_eq!(duplicates.len(), 1);
}
```

### 3. 旧格式迁移 (`migration.rs`)

#### 迁移流程
```rust
pub struct MemoryMigrator;

impl MemoryMigrator {
    /// 检查是否需要迁移
    pub fn needs_migration(memory_dir: &Path) -> bool {
        // 检查是否存在旧格式文件（*.md）
    }

    /// 执行迁移
    pub fn migrate(
        memory_dir: &Path,
        project_path: &str
    ) -> Result<MigrationResult> {
        // 1. 读取旧格式文件
        // 2. 解析 Markdown 内容
        // 3. 转换为新格式
        // 4. 自动去重
        // 5. 写入 JSON 文件
        // 6. 备份旧文件
    }
}
```

#### 旧格式支持
- **文件**: `rule.md`, `preference.md`, `pattern.md`, `context.md`
- **格式**: Markdown 列表
```markdown
- 项目使用 Rust 2021 Edition
- 遵循 Rust 命名规范
```

#### 单元测试
```rust
#[test]
fn test_migration() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 创建旧格式文件
    std::fs::write(
        temp_dir.path().join("rule.md"),
        "- 规则1\n- 规则2"
    ).unwrap();

    // 执行迁移
    let result = MemoryMigrator::migrate(
        temp_dir.path(),
        "/test/project"
    ).unwrap();

    assert_eq!(result.md_entries_count, 2);
}
```

### 4. 记忆管理 (`manager.rs`)

#### 核心功能
```rust
pub struct MemoryManager {
    memory_dir: PathBuf,
    project_path: String,
    store: MemoryStore,
    is_non_git_project: bool,
}

impl MemoryManager {
    /// 创建管理器（自动迁移 + 去重）
    pub fn new(project_path: &str) -> Result<Self>

    /// 添加记忆
    pub fn add_memory(&mut self, category: MemoryCategory, content: &str) -> Result<String>

    /// 查询记忆
    pub fn search_memories(&self, query: &str) -> Vec<&MemoryEntry>

    /// 列出所有记忆
    pub fn list_memories(&self) -> &[MemoryEntry]

    /// 删除记忆
    pub fn delete_memory(&mut self, memory_id: &str) -> Result<()>

    /// 去重检测
    pub fn detect_duplicates(&self) -> Vec<DuplicateInfo>

    /// 自动去重
    pub fn auto_deduplicate(&mut self) -> Result<DedupResult>
}
```

#### 路径规范化
- **Git 项目**: 使用 Git 根目录
- **非 Git 项目**: 使用项目目录本身（降级模式）

---

## 数据流程

### 添加记忆
```
AI 请求 → MemoryManager::add_memory() → 生成 ID → 保存到 JSON → 返回 ID
```

### 查询记忆
```
AI 请求 → MemoryManager::search_memories() → 模糊匹配 → 返回匹配结果
```

### 启动时去重
```
MemoryManager::new() → 检测旧格式 → 迁移 → 自动去重 → 加载存储
```

---

## 常见问题 (FAQ)

### Q: 如何调整相似度阈值？
A: 调用 `配置` 操作，设置 `similarity_threshold` (0.0-1.0)

### Q: 如何禁用启动时去重？
A: 调用 `配置` 操作，设置 `dedup_on_startup: false`

### Q: 记忆存储在哪里？
A: `.sanshu-memory/memories.json`（项目根目录或 Git 根目录）

### Q: 如何手动触发去重？
A: 调用 `整理` 操作

### Q: 支持哪些查询方式？
A: 模糊匹配（包含关键词即可）

---

## 相关文件清单

### 核心文件
- `manager.rs` - 记忆管理器
- `similarity.rs` - 相似度算法
- `dedup.rs` - 去重检测器
- `migration.rs` - 旧格式迁移
- `mcp.rs` - MCP 工具实现
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 数据文件
- `.sanshu-memory/memories.json` - 记忆存储
- `.sanshu-memory/*.md.bak` - 旧格式备份

---

## 使用示例

### 添加记忆
```rust
let request = JiyiRequest {
    action: "记忆".to_string(),
    project_path: "/path/to/project".to_string(),
    category: Some("rule".to_string()),
    content: Some("项目使用 Rust 2021 Edition".to_string()),
    ..Default::default()
};

let result = MemoryTool::jiyi(request).await?;
```

### 查询记忆
```rust
let request = JiyiRequest {
    action: "回忆".to_string(),
    project_path: "/path/to/project".to_string(),
    content: Some("Rust".to_string()),
    ..Default::default()
};

let result = MemoryTool::jiyi(request).await?;
```

### 去重检测
```rust
let request = JiyiRequest {
    action: "整理".to_string(),
    project_path: "/path/to/project".to_string(),
    ..Default::default()
};

let result = MemoryTool::jiyi(request).await?;
```

---

**最后更新**: 2026-02-18
