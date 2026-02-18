# MCP 模块 (src/rust/mcp)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **mcp**

---

## 模块职责

MCP (Model Context Protocol) 服务器实现，提供 8 个核心工具（zhi, ji, sou, enhance, context7, icon, uiux, skills），通过 stdio 传输与 AI 助手通信。

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

```rust
impl ServerHandler for ZhiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "Zhi-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                // ...
            },
            instructions: Some("Zhi 智能代码审查工具...".to_string()),
        }
    }
}
```

---

## 对外接口 (MCP 工具)

### 核心工具列表

| 工具名 | 标识符 | 职责 | 默认状态 | 文档 |
|--------|--------|------|----------|------|
| 智 | `zhi` | 交互式决策、多模态输入 | ✅ 启用（不可禁用） | [查看](./tools/interaction/CLAUDE.md) |
| 记 | `ji` | 全局记忆管理 | ✅ 启用 | [查看](./tools/memory/CLAUDE.md) |
| 搜 | `sou` | 代码库语义搜索 | ⚙️ 可选 | [查看](./tools/acemcp/CLAUDE.md) |
| 增强 | `enhance` | 提示词增强 | ⚙️ 可选 | [查看](./tools/enhance/CLAUDE.md) |
| 文档 | `context7` | 框架文档查询 | ✅ 启用 | [查看](./tools/context7/CLAUDE.md) |
| 图标 | `tu` | 图标搜索与管理 | ✅ 启用 | [查看](./tools/icon/CLAUDE.md) |
| 设计 | `uiux` | UI/UX 设计检索 | ✅ 启用 | [查看](./tools/uiux/CLAUDE.md) |
| 技能 | `skill_run` | 技能运行时 | ✅ 启用 | [查看](./tools/skills/CLAUDE.md) |

### 工具调用流程
```
AI 助手 → MCP Client → stdio → ZhiServer → Tool Handler → 返回结果
```

### 工具注册机制
```rust
async fn list_tools(&self, ...) -> Result<ListToolsResult, McpError> {
    let mut tools = Vec::new();

    // 必需工具（始终可用）
    tools.push(InteractionTool::get_tool_definition());

    // 可选工具（根据配置启用）
    if self.is_tool_enabled("ji") {
        tools.push(MemoryTool::get_tool_definition());
    }

    // 动态工具（技能运行时）
    tools.extend(SkillsTool::list_dynamic_tools(project_root));

    Ok(ListToolsResult { tools, next_cursor: None })
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
[dependencies]
rmcp = { version = "0.12.0", features = ["server", "transport-io"] }
tokio = { version = "1.0", features = ["rt-multi-thread", "macros", "fs", "process", "sync", "time"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["stream", "json", "socks"] }
```

### 配置文件
- **位置**: `~/.config/sanshu/config.json` (Linux/macOS) 或 `%APPDATA%\sanshu\config.json` (Windows)
- **结构**: 见 `src/rust/config/settings.rs`

```json
{
  "mcp_config": {
    "tools": {
      "zhi": true,
      "ji": true,
      "sou": false,
      "enhance": false,
      "context7": true,
      "uiux": true
    }
  }
}
```

---

## 工具模块详解

### 1. 交互工具 (interaction)
- **路径**: `tools/interaction/`
- **核心文件**: `mcp.rs`, `zhi_history.rs`
- **职责**: 弹出 GUI 窗口，支持预定义选项、自由文本、图片上传
- **特性**:
  - 历史记录管理
  - UI/UX 上下文控制信号
  - Telegram 集成

### 2. 记忆工具 (memory)
- **路径**: `tools/memory/`
- **核心文件**: `manager.rs`, `similarity.rs`, `dedup.rs`, `migration.rs`
- **职责**: 全局记忆存储、相似度检测、自动去重
- **存储**: `.sanshu-memory/memories.json`
- **特性**:
  - 4 种记忆分类（rule, preference, pattern, context）
  - 文本相似度算法（Levenshtein + Phrase + Jaccard）
  - 启动时自动迁移旧格式

### 3. 搜索工具 (acemcp)
- **路径**: `tools/acemcp/`
- **核心文件**: `mcp.rs`, `watcher.rs`
- **职责**: 代码库语义搜索、增量索引、文件监听
- **特性**:
  - 智能等待机制（1-5 秒）
  - 文件变更监听（notify-debouncer）
  - 多编码支持（UTF-8, GBK, Windows-1252）

### 4. 增强工具 (enhance)
- **路径**: `tools/enhance/`
- **核心文件**: `core.rs`, `history.rs`
- **职责**: 提示词优化、上下文增强
- **API**: Augment chat-stream API
- **特性**:
  - 流式响应
  - 历史对话整合
  - 项目上下文注入

### 5. 文档工具 (context7)
- **路径**: `tools/context7/`
- **核心文件**: `mcp.rs`, `commands.rs`
- **职责**: 框架文档查询（React, Vue, Tailwind 等）
- **API**: Context7 API
- **特性**:
  - 智能降级搜索
  - 分页浏览
  - 缓存机制

### 6. 图标工具 (icon)
- **路径**: `tools/icon/`
- **核心文件**: `api.rs`, `mcp.rs`
- **职责**: Iconfont 图标搜索、预览、下载
- **API**: iconfont.cn API
- **特性**:
  - 搜索结果缓存（30 分钟）
  - 批量下载
  - SVG 转 PNG

### 7. UI/UX 工具 (uiux)
- **路径**: `tools/uiux/`
- **核心文件**: `engine.rs`, `lexicon.rs`
- **职责**: 设计系统生成、样式搜索
- **数据**: 嵌入式 CSV 数据库（50+ 样式、97 色板、57 字体对）
- **特性**:
  - 优先级推荐
  - 多技术栈支持（React, Vue, SwiftUI 等）
  - 本地化输出

### 8. 技能工具 (skills)
- **路径**: `tools/skills/`
- **核心文件**: `mod.rs`
- **职责**: 动态加载和执行 Python 技能脚本
- **扫描路径**: `.codex/skills`, `.claude/skills`, `skills/` 等
- **特性**:
  - 动态工具注册（`skill_<name>`）
  - 配置驱动（`skill.config.json`）
  - 安全沙箱（路径穿透检测）

---

## 数据模型

### MCP 请求类型 (`types.rs`)
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ZhiRequest {
    pub message: String,
    #[serde(default)]
    pub predefined_options: Vec<String>,
    #[serde(default = "default_true")]
    pub is_markdown: bool,
    pub project_root_path: Option<String>,
    pub uiux_intent: Option<String>,
    pub uiux_context_policy: Option<String>,
    pub uiux_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JiyiRequest {
    pub action: String,
    pub project_path: String,
    pub category: Option<String>,
    pub content: Option<String>,
    pub memory_id: Option<String>,
    pub config: Option<serde_json::Value>,
}
```

---

## 测试策略

### 单元测试
- ✅ `tools/memory/similarity.rs` - 文本相似度算法
- ✅ `tools/memory/dedup.rs` - 去重检测器
- ✅ `tools/memory/migration.rs` - 旧格式迁移
- ✅ `tools/uiux/engine.rs` - 设计系统搜索

### 集成测试
```bash
# 运行所有测试
cargo test --package sanshu --lib mcp

# 运行特定模块测试
cargo test --package sanshu --lib mcp::tools::memory
```

---

## 常见问题 (FAQ)

### Q: 如何添加新的 MCP 工具？
A:
1. 在 `tools/` 创建新模块目录
2. 实现 `get_tool_definition()` 和 `call_tool()` 方法
3. 在 `server.rs` 的 `list_tools()` 中注册
4. 在 `server.rs` 的 `call_tool()` 中路由
5. 添加单元测试和文档

### Q: 如何调试 MCP 服务器？
A: 设置环境变量 `RUST_LOG=debug` 并运行 `cargo run --bin 三术`

### Q: 工具启用状态如何持久化？
A: 通过 `config.json` 中的 `mcp_config.tools` 字段

---

## 相关文件清单

### 核心文件
- `bin/mcp_server.rs` - MCP 服务器入口
- `server.rs` - 服务器实现
- `types.rs` - 数据类型定义
- `commands.rs` - Tauri 命令（GUI 集成）
- `mod.rs` - 模块导出

### 工具目录
- `tools/interaction/` - 交互工具
- `tools/memory/` - 记忆工具
- `tools/acemcp/` - 搜索工具
- `tools/enhance/` - 增强工具
- `tools/context7/` - 文档工具
- `tools/icon/` - 图标工具
- `tools/uiux/` - UI/UX 工具
- `tools/skills/` - 技能工具

### 辅助模块
- `handlers/` - 请求处理器（popup, response）
- `utils/` - 工具函数（错误处理、ID 生成）

---

**最后更新**: 2026-02-18
