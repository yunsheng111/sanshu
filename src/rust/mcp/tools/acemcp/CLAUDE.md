# 搜索工具 (acemcp)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **acemcp**

---

## 模块职责

代码库索引与语义搜索工具 (sou)，提供增量索引、智能等待、文件监听和多编码支持。通过 Augment Ace API 实现语义搜索能力。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `sou`
- **标识符**: `mcp______sou`
- **状态**: 默认关闭（可选启用）

### 核心结构
```rust
pub struct AcemcpTool;

impl AcemcpTool {
    pub async fn search_context(request: AcemcpRequest) -> Result<CallToolResult, McpError>
    async fn get_acemcp_config() -> Result<AcemcpConfig>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "sou",
  "arguments": {
    "project_root_path": "/path/to/project",
    "query": "用户认证逻辑"
  }
}
```

### 请求参数
```rust
pub struct AcemcpRequest {
    /// 项目根路径
    pub project_root_path: String,

    /// 搜索查询
    pub query: String,
}
```

### 响应格式
```json
{
  "results": [
    {
      "file": "src/auth/login.rs",
      "line": 42,
      "content": "pub fn authenticate_user(username: &str, password: &str) -> Result<User>",
      "score": 0.95
    }
  ],
  "total": 10,
  "index_status": "ready"
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = ["stream", "json", "socks"] }
tokio = { version = "1.0", features = ["fs", "process", "sync", "time"] }
ignore = "0.4"
notify = "6.0"
notify-debouncer-full = "0.3"
encoding_rs = "0.8"
globset = "0.4"
ring = "0.17"
```

### 配置结构
```rust
pub struct AcemcpConfig {
    /// API 基础 URL
    pub base_url: Option<String>,

    /// API Token
    pub token: Option<String>,

    /// 智能等待范围（秒）
    pub smart_wait_range: Option<(u64, u64)>,

    /// 代理配置
    pub proxy: Option<ProxyConfig>,
}
```

### 默认配置
- **base_url**: `http://localhost:3000`
- **smart_wait_range**: `(1, 5)` 秒
- **token**: 从环境变量 `ACEMCP_TOKEN` 读取

---

## 核心功能

### 1. 智能等待机制

#### 索引状态检测
```rust
enum InitialIndexState {
    Missing,    // 未索引
    Idle,       // 空闲（可搜索）
    Indexing,   // 索引中
    Failed,     // 索引失败
}

fn get_initial_index_state(project_root: &str) -> InitialIndexState {
    // 调用 API 检查索引状态
}
```

#### 等待策略
```rust
match initial_state {
    InitialIndexState::Missing | InitialIndexState::Idle | InitialIndexState::Failed => {
        // 启动后台索引
        ensure_initial_index_background(&config, &project_root).await?;
        hint_message = "索引已启动，正在后台运行...";
    }
    InitialIndexState::Indexing => {
        // 智能等待（1-5 秒）
        let wait_secs = rand::thread_rng().gen_range(1..=5);
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
    }
}
```

### 2. 文件监听 (`watcher.rs`)

#### 监听管理器
```rust
pub struct WatcherManager {
    watchers: Arc<Mutex<HashMap<String, WatcherHandle>>>,
}

impl WatcherManager {
    /// 启动文件监听
    pub async fn start_watching(
        &self,
        project_root: String,
        config: AcemcpConfig,
        debounce_delay: Option<Duration>
    ) -> Result<()>

    /// 停止文件监听
    pub fn stop_watching(&self, project_root: &str) -> Result<()>

    /// 检查是否正在监听
    pub fn is_watching(&self, project_root: &str) -> bool
}
```

#### 防抖机制
- **默认延迟**: 2 秒
- **触发条件**: 文件创建、修改、删除
- **忽略规则**: 遵循 `.gitignore`

#### 增量上传
```rust
async fn handle_file_changes(
    events: Vec<DebouncedEvent>,
    project_root: &str,
    config: &AcemcpConfig
) {
    // 1. 收集变更文件
    // 2. 计算文件哈希
    // 3. 批量上传到 API
    // 4. 更新索引状态
}
```

### 3. 多编码支持

#### 编码检测
```rust
fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    // 1. 检测 BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return UTF_8;
    }

    // 2. 尝试 UTF-8 解码
    if std::str::from_utf8(bytes).is_ok() {
        return UTF_8;
    }

    // 3. 降级到 GBK（中文）
    GBK
}
```

#### 支持的编码
- UTF-8 (优先)
- GBK (中文)
- Windows-1252 (西欧)

### 4. 忽略规则

#### 规则来源
1. `.gitignore` 文件
2. 默认忽略模式
```rust
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "node_modules/**",
    ".git/**",
    "target/**",
    "dist/**",
    "build/**",
    "*.lock",
    "*.log",
];
```

#### 规则合并
```rust
fn build_gitignore(project_root: &Path) -> Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(project_root);

    // 1. 加载 .gitignore
    if let Ok(gitignore_path) = project_root.join(".gitignore").canonicalize() {
        builder.add(gitignore_path);
    }

    // 2. 添加默认规则
    for pattern in DEFAULT_IGNORE_PATTERNS {
        builder.add_line(None, pattern)?;
    }

    builder.build()
}
```

---

## 数据流程

### 搜索流程
```
AI 请求 → 检查索引状态 → 智能等待 → 调用 API → 返回结果
```

### 索引流程
```
文件变更 → 防抖延迟 → 收集变更 → 计算哈希 → 批量上传 → 更新状态
```

### 首次索引
```
启动监听 → 扫描所有文件 → 批量上传 → 等待索引完成 → 可搜索
```

---

## API 集成

### 端点列表
| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/search` | POST | 语义搜索 |
| `/api/index/status` | GET | 索引状态 |
| `/api/index/upload` | POST | 上传文件 |
| `/api/index/delete` | POST | 删除文件 |

### 请求示例
```rust
// 搜索请求
let response = client
    .post(format!("{}/api/search", base_url))
    .header(AUTHORIZATION, format!("Bearer {}", token))
    .json(&json!({
        "project_root": project_root,
        "query": query,
        "limit": 10
    }))
    .send()
    .await?;
```

---

## 常见问题 (FAQ)

### Q: 如何配置 API 地址？
A: 在配置文件中设置 `acemcp_config.base_url`

### Q: 如何启用代理？
A: 在配置文件中设置 `acemcp_config.proxy`

### Q: 智能等待时间如何调整？
A: 在配置文件中设置 `acemcp_config.smart_wait_range`

### Q: 如何查看索引状态？
A: 调用 Tauri 命令 `get_acemcp_index_status`

### Q: 如何手动触发索引？
A: 调用 Tauri 命令 `trigger_acemcp_index`

### Q: 支持哪些文件类型？
A: 所有文本文件（根据 `.gitignore` 过滤）

---

## 相关文件清单

### 核心文件
- `mcp.rs` - MCP 工具实现
- `watcher.rs` - 文件监听管理器
- `commands.rs` - Tauri 命令
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 配置文件
- `config.json` - 全局配置
- `.gitignore` - 忽略规则

---

## 使用示例

### 基础搜索
```rust
let request = AcemcpRequest {
    project_root_path: "/path/to/project".to_string(),
    query: "用户认证逻辑".to_string(),
};

let result = AcemcpTool::search_context(request).await?;
```

### 启动文件监听
```rust
let watcher_manager = get_watcher_manager();
watcher_manager.start_watching(
    project_root.clone(),
    config.clone(),
    Some(Duration::from_secs(2))
).await?;
```

### 停止文件监听
```rust
let watcher_manager = get_watcher_manager();
watcher_manager.stop_watching(&project_root)?;
```

---

**最后更新**: 2026-02-18
