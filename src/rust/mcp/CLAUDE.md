# MCP 模块 (src/rust/mcp)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **mcp**

---

## 模块职责

MCP (Model Context Protocol) 服务器实现，提供 8 个核心工具（zhi, ji, sou, enhance, context7, icon, uiux, skills），通过 stdio 传输与 AI 助手通信。新增配置热更新、可观测性指标收集和统一错误分类基础设施。

---

## 入口与启动

### MCP 服务器入口
- **文件**: `bin/mcp_server.rs`
- **二进制名**: `三术`
- **传输方式**: stdio (标准输入/输出)
- **协议**: JSON-RPC 2.0

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    auto_init_logger()?;
    log_important!(info, "启动 MCP 服务器");
    run_server().await
}
```

### 服务器实现
- **文件**: `server.rs`
- **核心结构**: `ZhiServer`
- **协议版本**: MCP 2024-11-05

---

## 对外接口 (MCP 工具)

### 核心工具列表

| 工具名 | 标识符 | 职责 | 默认状态 | 文档 |
|--------|--------|------|----------|------|
| 智 | `zhi` | 交互式决策、多模态输入 | 启用（不可禁用） | [查看](./tools/interaction/CLAUDE.md) |
| 记 | `ji` | 全局记忆管理（并发安全 + 版本控制） | 启用 | [查看](./tools/memory/CLAUDE.md) |
| 搜 | `sou` | 代码库混合检索（BM25 + 向量语义） | 可选 | [查看](./tools/acemcp/CLAUDE.md) |
| 增强 | `enhance` | 提示词增强（三级降级链 + 缓存） | 可选 | [查看](./tools/enhance/CLAUDE.md) |
| 文档 | `context7` | 框架文档查询 | 启用 | [查看](./tools/context7/CLAUDE.md) |
| 图标 | `tu` | 图标搜索与管理 | 启用 | [查看](./tools/icon/CLAUDE.md) |
| 设计 | `uiux` | UI/UX 设计检索 | 启用 | [查看](./tools/uiux/CLAUDE.md) |
| 技能 | `skill_run` | 技能运行时 | 启用 | [查看](./tools/skills/CLAUDE.md) |

### 工具调用流程
```
AI 助手 → MCP Client → stdio → ZhiServer
    |-- hot_reload: 缓存工具启用状态（5s 刷新）
    |-- metrics: 记录调用延迟/缓存/错误
    +-- Tool Handler → 返回结果
```

### 工具注册机制（带热更新）
```rust
async fn list_tools(&self, ...) -> Result<ListToolsResult, McpError> {
    let mut tools = Vec::new();

    // 必需工具（始终可用）
    tools.push(InteractionTool::get_tool_definition());

    // 可选工具（通过 is_tool_enabled_cached() 热更新判断）
    if is_tool_enabled_cached("ji") {
        tools.push(MemoryTool::get_tool_definition());
    }

    // 动态工具（技能运行时）
    tools.extend(SkillsTool::list_dynamic_tools(project_root));

    Ok(ListToolsResult { tools, next_cursor: None })
}
```

---

## 基础设施模块（P2 新增）

### 1. 配置热更新 (`hot_reload.rs`)

SC-13 实现。提供工具启用状态的缓存和自动刷新机制，避免每次工具调用都读取配置文件。

```rust
/// 配置缓存刷新间隔
const CACHE_REFRESH_INTERVAL_SECS: u64 = 5;

/// 热更新配置缓存
struct HotReloadCache {
    tools: HashMap<String, bool>,  // 工具启用状态
    last_updated: Instant,          // 最后更新时间
    config_path: Option<PathBuf>,   // 配置文件路径
}

/// 检查工具是否启用（带缓存和热更新）
pub fn is_tool_enabled_cached(tool_name: &str) -> bool
```

**工作原理**：
- 全局缓存 `CONFIG_CACHE`（`Lazy<Arc<RwLock<HotReloadCache>>>`）
- 每次查询检查缓存是否过期（> 5 秒）
- 过期则从配置文件重新加载
- 加载失败时使用默认值

### 2. 可观测性指标 (`metrics.rs`)

SC-14 实现。收集 MCP 工具调用的性能指标，支持 P50/P95/P99 延迟百分位数计算。

```rust
pub struct McpMetrics {
    pub tool_calls: AtomicU64,           // 工具调用总数
    pub cache_hits: AtomicU64,           // 缓存命中数
    pub cache_misses: AtomicU64,         // 缓存未命中数
    pub api_errors: AtomicU64,           // API 错误数
    pub latency_samples: RwLock<Vec<u64>>,   // 延迟样本（毫秒）
    pub tool_call_counts: RwLock<HashMap<String, u64>>,  // 每工具调用计数
    pub tool_error_counts: RwLock<HashMap<String, u64>>, // 每工具错误计数
}
```

**核心方法**：
- `record_call(tool, latency_ms)` - 记录工具调用和延迟
- `record_cache_hit()` / `record_cache_miss()` - 缓存指标
- `record_error(tool)` - 错误计数
- `get_latency_percentile(p)` - 获取 P50/P95/P99 延迟
- `get_summary()` - 获取完整指标摘要

### 3. 统一错误分类 (`utils/errors.rs`)

HC-6 实现。提供统一的 MCP 错误类型枚举，支持可重试判断。

```rust
#[derive(Debug, thiserror::Error)]
pub enum McpToolError {
    #[error("项目路径错误: {0}")]       ProjectPath(String),
    #[error("弹窗创建失败: {0}")]       PopupCreation(String),
    #[error("响应解析失败: {0}")]       ResponseParsing(String),
    #[error("记忆管理错误: {0}")]       Memory(String),
    #[error("IO 错误: {0}")]           Io(#[from] std::io::Error),
    #[error("JSON 序列化错误: {0}")]    Json(#[from] serde_json::Error),
    #[error("通用错误: {0}")]           Generic(#[from] anyhow::Error),
    // HC-6 新增网络错误分类
    #[error("网络超时: {0}")]           NetworkTimeout(String),
    #[error("网络连接失败: {0}")]       NetworkConnection(String),
    #[error("认证失败: {0}")]           AuthenticationFailed(String),
    #[error("API 限流: {0}")]           RateLimited(String),
    #[error("外部服务不可用: {0}")]     ServiceUnavailable(String),
    #[error("参数验证失败: {0}")]       ValidationError(String),
}
```

**可重试判断**：`NetworkTimeout`、`NetworkConnection`、`RateLimited`、`ServiceUnavailable` 为可重试错误。

---

## 关键依赖与配置

### 核心依赖
```toml
[dependencies]
rmcp = { version = "0.12.0", features = [
  "server",
  "transport-io"
] }
tokio = { version = "1.0", features = [
  "rt-multi-thread",
  "macros",
  "fs",
  "process",
  "sync",
  "time"
] }
anyhow = "1.0"
thiserror = "1.0"
serde = { version = "1.0", features = [ "derive" ] }
serde_json = "1.0"
reqwest = { version = "0.11", features = [
  "stream",
  "json",
  "socks"
] }
once_cell = "1.0"
```

### 配置文件
- **位置**: `~/.config/sanshu/config.json` (Linux/macOS) 或 `%APPDATA%\sanshu\config.json` (Windows)
- **热更新**: 通过 `hot_reload.rs` 每 5 秒自动刷新工具启用状态

---

## 工具模块详解

### 1. 交互工具 (interaction)
- **路径**: `tools/interaction/`
- **核心文件**: `mcp.rs`, `zhi_history.rs`
- **职责**: 弹出 GUI 窗口，支持预定义选项、自由文本、图片上传

### 2. 记忆工具 (memory) - **P0 重大升级**
- **路径**: `tools/memory/`
- **核心文件**: `manager.rs`, `similarity.rs`, `dedup.rs`, `migration.rs`, `mcp.rs`, `types.rs`
- **新增能力**:
  - `SharedMemoryManager` - Arc<RwLock> 并发安全访问
  - 版本控制 - 每条记忆支持版本号和快照（最多 5 个）
  - 原子写入 - 写临时文件后 rename，避免损坏
  - "更新"action - 支持更新已有记忆（自动创建快照）
  - 数据迁移 v2.1 - `MemoryEntry` 新增 `version`/`snapshots` 字段

### 3. 搜索工具 (acemcp) - **P1 重大升级**
- **路径**: `tools/acemcp/`
- **核心文件**: `mcp.rs`, `watcher.rs`, `cache.rs`, `local_index.rs`, `hybrid_search.rs`, `embedding_client.rs`
- **新增能力**:
  - 混合检索 - BM25 关键词 + 向量语义 + RRF 融合排序
  - 本地向量索引 - 增量更新、并发安全、500MB 空间限制
  - 多提供者嵌入 - Jina/SiliconFlow/Cloudflare/Nomic/Cohere/Ollama
  - 三层缓存 - 内存 LRU（100 条/5min TTL）+ 磁盘持久化（SHA256 key）+ API 回源

### 4. 增强工具 (enhance) - **P1 重大升级**
- **路径**: `tools/enhance/`
- **核心文件**: `core.rs`, `history.rs`, `cache.rs`, `chat_client.rs`, `provider_factory.rs`, `rule_engine.rs`
- **新增能力**:
  - 统一 Chat 客户端 - 支持 Ollama/OpenAI/Gemini/Anthropic/规则引擎
  - 三级降级链 - L1 Ollama 本地 -> L2 云端 API -> L3 规则引擎
  - 结果缓存 - LRU 缓存（50 条/10min TTL）
  - 规则引擎 - 无 API 时的 10 条内置规则降级增强

### 5. 文档工具 (context7) - 无变更
### 6. 图标工具 (icon) - 无变更
### 7. UI/UX 工具 (uiux) - 无变更
### 8. 技能工具 (skills) - 无变更

---

## 数据模型

### MCP 请求类型 (`types.rs`)
```rust
pub struct ZhiRequest { ... }    // 交互请求
pub struct JiyiRequest { ... }   // 记忆请求（新增"更新"action）
pub struct AcemcpRequest { ... } // 搜索请求
pub struct EnhanceMcpRequest { ... } // 增强请求
```

---

## 测试策略

### 单元测试
- `tools/memory/similarity.rs` - 文本相似度算法
- `tools/memory/dedup.rs` - 去重检测器
- `tools/memory/migration.rs` - 旧格式迁移
- `tools/uiux/engine.rs` - 设计系统搜索

### 集成测试
```bash
cargo test --package sanshu --lib mcp
cargo test --package sanshu --lib mcp::tools::memory
```

---

## 相关文件清单

### 核心文件
- `bin/mcp_server.rs` - MCP 服务器入口
- `server.rs` - 服务器实现
- `types.rs` - 数据类型定义
- `commands.rs` - Tauri 命令（GUI 集成）
- `mod.rs` - 模块导出
- `hot_reload.rs` - SC-13 配置热更新
- `metrics.rs` - SC-14 可观测性指标

### 工具目录
- `tools/interaction/` - 交互工具 (4 文件)
- `tools/memory/` - 记忆工具 (7 文件)
- `tools/acemcp/` - 搜索工具 (9 文件)
- `tools/enhance/` - 增强工具 (11 文件)
- `tools/context7/` - 文档工具 (4 文件)
- `tools/icon/` - 图标工具 (5 文件)
- `tools/uiux/` - UI/UX 工具 (8 文件)
- `tools/skills/` - 技能工具 (1 文件)

### 辅助模块
- `handlers/` - 请求处理器（popup, response, icon_popup）
- `utils/` - 工具函数（`errors.rs` 统一错误分类、`common.rs` ID 生成）

---

**最后更新**: 2026-02-19
