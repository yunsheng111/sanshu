# 交互工具 (interaction)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **interaction**

---

## 模块职责

智能代码审查交互工具 (zhi)，提供交互式决策能力，支持预定义选项、自由文本输入和图片上传。通过弹出 GUI 窗口与用户交互，是三术系统的核心交互入口。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `zhi`
- **标识符**: `mcp______zhi`
- **状态**: 必需工具（不可禁用）

### 核心结构
```rust
pub struct InteractionTool;

impl InteractionTool {
    pub async fn zhi(request: ZhiRequest) -> Result<CallToolResult, McpError>
    pub async fn zhi_with_request_id(request: ZhiRequest, request_id: String) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "zhi",
  "arguments": {
    "message": "请选择操作",
    "predefined_options": ["选项1", "选项2", "选项3"],
    "is_markdown": true,
    "project_root_path": "/path/to/project",
    "uiux_intent": "design_review",
    "uiux_context_policy": "minimal",
    "uiux_reason": "快速决策"
  }
}
```

### 请求参数
```rust
pub struct ZhiRequest {
    /// 显示给用户的消息（支持 Markdown）
    pub message: String,

    /// 预定义选项列表（可选）
    #[serde(default)]
    pub predefined_options: Vec<String>,

    /// 是否启用 Markdown 渲染（默认 true）
    #[serde(default = "default_true")]
    pub is_markdown: bool,

    /// 项目根路径（用于索引状态显示）
    pub project_root_path: Option<String>,

    /// UI/UX 上下文控制信号
    pub uiux_intent: Option<String>,
    pub uiux_context_policy: Option<String>,
    pub uiux_reason: Option<String>,
}
```

### 响应格式
- **文本响应**: 用户输入的文本或选择的选项
- **图片响应**: Base64 编码的图片数据（格式：`IMAGE:<base64>`）
- **取消响应**: `CANCELLED`

---

## 关键依赖与配置

### 核心依赖
```toml
rmcp = { version = "0.12.0", features = ["server"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 历史记录管理
- **存储位置**: `.sanshu-memory/zhi_history.json`
- **最大条数**: 100
- **字段**: `timestamp`, `message`, `response`, `options`, `project_root`

```rust
pub struct ZhiHistoryEntry {
    pub timestamp: String,
    pub message: String,
    pub response: String,
    pub options: Option<Vec<String>>,
    pub project_root: Option<String>,
}
```

---

## 数据流程

### 1. 请求接收
```
AI 助手 → MCP Client → stdio → ZhiServer → InteractionTool::zhi()
```

### 2. 弹窗创建
```rust
let popup_request = PopupRequest {
    id: request_id.clone(),
    message: request.message,
    predefined_options: Some(request.predefined_options),
    is_markdown: request.is_markdown,
    project_root_path: request.project_root_path,
    uiux_intent: request.uiux_intent,
    uiux_context_policy: request.uiux_context_policy,
    uiux_reason: request.uiux_reason,
};

create_tauri_popup(&popup_request)?
```

### 3. 用户交互
```
GUI 窗口 → 用户输入/选择 → Tauri 事件 → 响应返回
```

### 4. 历史记录
```rust
ZhiHistoryManager::add_entry(
    &project_root,
    &request.message,
    &response,
    request.predefined_options.clone()
)?;
```

---

## UI/UX 上下文控制

### 控制信号
| 信号 | 类型 | 说明 |
|------|------|------|
| `uiux_intent` | String | 交互意图（如 `design_review`, `code_approval`） |
| `uiux_context_policy` | String | 上下文策略（`minimal`, `standard`, `full`） |
| `uiux_reason` | String | 控制原因（用于审计） |

### 使用场景
```rust
// 快速决策（最小上下文）
ZhiRequest {
    message: "是否继续？",
    predefined_options: vec!["是".to_string(), "否".to_string()],
    uiux_intent: Some("quick_decision".to_string()),
    uiux_context_policy: Some("minimal".to_string()),
    uiux_reason: Some("快速确认".to_string()),
    ..Default::default()
}
```

---

## Telegram 集成

### 远程交互
当配置了 Telegram Bot 时，弹窗会同时推送到 Telegram：

```rust
// 发送消息到 Telegram
telegram_core.send_message_with_options(
    &request.message,
    &request.predefined_options,
    request.is_markdown
).await?;

// 等待用户响应
let response = telegram_core.wait_for_response().await?;
```

### 配置要求
- `telegram_config.enabled = true`
- `telegram_config.bot_token` - Bot Token
- `telegram_config.chat_id` - Chat ID

---

## 常见问题 (FAQ)

### Q: 如何支持图片上传？
A: 用户在弹窗中点击"上传图片"按钮，选择图片后自动转换为 Base64 编码返回。

### Q: 历史记录如何清理？
A: 自动保留最近 100 条记录，超出部分自动删除。

### Q: 如何禁用 Markdown 渲染？
A: 设置 `is_markdown: false`

### Q: 如何获取历史记录？
A: 调用 `ZhiHistoryManager::load_history(project_root)`

---

## 相关文件清单

### 核心文件
- `mcp.rs` - MCP 工具实现
- `zhi_history.rs` - 历史记录管理
- `commands.rs` - Tauri 命令（GUI 集成）
- `mod.rs` - 模块导出

### 数据文件
- `.sanshu-memory/zhi_history.json` - 历史记录存储

---

## 使用示例

### 基础交互
```rust
// 简单确认
let request = ZhiRequest {
    message: "是否继续执行？".to_string(),
    predefined_options: vec!["是".to_string(), "否".to_string()],
    is_markdown: false,
    ..Default::default()
};

let result = InteractionTool::zhi(request).await?;
```

### Markdown 消息
```rust
// 带格式的消息
let request = ZhiRequest {
    message: r#"
## 代码审查结果

- ✅ 语法正确
- ⚠️ 性能问题：循环嵌套过深
- ❌ 安全问题：SQL 注入风险

**建议**: 重构数据库查询逻辑
    "#.to_string(),
    predefined_options: vec!["修复".to_string(), "忽略".to_string(), "稍后处理".to_string()],
    is_markdown: true,
    ..Default::default()
};
```

### 自由文本输入
```rust
// 无预定义选项，允许自由输入
let request = ZhiRequest {
    message: "请输入提交信息：".to_string(),
    predefined_options: vec![],
    is_markdown: false,
    ..Default::default()
};
```

---

**最后更新**: 2026-02-18
