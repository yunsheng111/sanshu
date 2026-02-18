# Telegram 集成模块 (telegram)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **telegram**

---

## 模块职责

Telegram Bot 集成模块，提供远程交互能力。支持将 MCP 弹窗推送到 Telegram、接收用户响应、Markdown 渲染和自定义 API 端点。

---

## 入口与启动

### 核心结构
```rust
pub struct TelegramCore {
    pub bot: Bot,
    pub chat_id: ChatId,
}

impl TelegramCore {
    pub fn new(bot_token: String, chat_id: String) -> Result<Self>
    pub fn new_with_api_url(bot_token: String, chat_id: String, api_url: Option<String>) -> Result<Self>
}
```

---

## 对外接口

### Tauri 命令
```rust
#[tauri::command]
async fn test_telegram_connection(config: TelegramConfig) -> Result<String, String>

#[tauri::command]
async fn send_telegram_message(message: String, config: TelegramConfig) -> Result<(), String>
```

### 配置结构
```rust
pub struct TelegramConfig {
    /// 是否启用 Telegram
    pub enabled: bool,

    /// Bot Token
    pub bot_token: String,

    /// Chat ID
    pub chat_id: String,

    /// 自定义 API URL（可选）
    pub api_url: Option<String>,
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
teloxide = { version = "0.15.0", features = ["macros"] }
tokio = { version = "1.0", features = ["sync"] }
regex = "1.0"
```

### 配置文件
- **位置**: `config.json`
- **字段**:
```json
{
  "telegram_config": {
    "enabled": true,
    "bot_token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
    "chat_id": "123456789",
    "api_url": null
  }
}
```

---

## 核心功能

### 1. 消息发送 (`core.rs`)

#### 普通消息
```rust
pub async fn send_message(&self, message: &str) -> Result<()> {
    self.bot
        .send_message(self.chat_id, message)
        .await?;
    Ok(())
}
```

#### Markdown 消息
```rust
pub async fn send_message_with_markdown(&self, message: &str, is_markdown: bool) -> Result<()> {
    let processed = if is_markdown {
        process_telegram_markdown(message)
    } else {
        message.to_string()
    };

    self.bot
        .send_message(self.chat_id, processed)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}
```

#### 带选项的消息
```rust
pub async fn send_message_with_options(
    &self,
    message: &str,
    options: &[String],
    is_markdown: bool
) -> Result<MessageId> {
    let processed = if is_markdown {
        process_telegram_markdown(message)
    } else {
        message.to_string()
    };

    // 构建内联键盘
    let keyboard = InlineKeyboardMarkup::new(
        options.iter().map(|opt| {
            vec![InlineKeyboardButton::callback(opt.clone(), opt.clone())]
        }).collect::<Vec<_>>()
    );

    let msg = self.bot
        .send_message(self.chat_id, processed)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(keyboard)
        .await?;

    Ok(msg.id)
}
```

### 2. Markdown 处理 (`markdown.rs`)

#### 转义规则
```rust
pub fn process_telegram_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // 1. 保护代码块
    let code_blocks = extract_code_blocks(&result);
    result = replace_code_blocks_with_placeholders(&result, &code_blocks);

    // 2. 转义特殊字符
    let special_chars = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!'];
    for ch in special_chars {
        result = result.replace(ch, &format!("\\{}", ch));
    }

    // 3. 恢复代码块
    result = restore_code_blocks(&result, &code_blocks);

    result
}
```

#### 代码块处理
```rust
fn extract_code_blocks(text: &str) -> Vec<String> {
    let re = Regex::new(r"```[\s\S]*?```|`[^`]+`").unwrap();
    re.find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}

fn replace_code_blocks_with_placeholders(text: &str, blocks: &[String]) -> String {
    let mut result = text.to_string();
    for (i, block) in blocks.iter().enumerate() {
        result = result.replace(block, &format!("__CODE_BLOCK_{}__", i));
    }
    result
}

fn restore_code_blocks(text: &str, blocks: &[String]) -> String {
    let mut result = text.to_string();
    for (i, block) in blocks.iter().enumerate() {
        result = result.replace(&format!("__CODE_BLOCK_{}__", i), block);
    }
    result
}
```

#### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_special_chars() {
        let input = "Hello *world* _test_";
        let output = process_telegram_markdown(input);
        assert_eq!(output, "Hello \\*world\\* \\_test\\_");
    }

    #[test]
    fn test_preserve_code_blocks() {
        let input = "Text `code` more text";
        let output = process_telegram_markdown(input);
        assert!(output.contains("`code`"));
    }
}
```

### 3. 事件处理 (`core.rs`)

#### 文本消息处理
```rust
pub async fn handle_text_message(
    bot: Bot,
    msg: Message,
    app_handle: AppHandle
) -> Result<()> {
    if let Some(text) = msg.text() {
        // 发送事件到前端
        app_handle.emit("telegram_event", TelegramEvent::TextUpdated {
            text: text.to_string()
        })?;
    }
    Ok(())
}
```

#### 回调查询处理
```rust
pub async fn handle_callback_query(
    bot: Bot,
    query: CallbackQuery,
    app_handle: AppHandle
) -> Result<()> {
    if let Some(data) = query.data {
        // 发送事件到前端
        app_handle.emit("telegram_event", TelegramEvent::OptionToggled {
            option: data.clone(),
            selected: true
        })?;

        // 回复确认
        bot.answer_callback_query(query.id)
            .text("已选择")
            .await?;
    }
    Ok(())
}
```

### 4. MCP 集成 (`mcp_handler.rs`)

#### MCP 请求处理
```rust
pub async fn handle_telegram_only_mcp_request(
    request: PopupRequest,
    config: TelegramConfig
) -> Result<String> {
    // 1. 创建 Telegram 核心
    let core = TelegramCore::new_with_api_url(
        config.bot_token,
        config.chat_id,
        config.api_url
    )?;

    // 2. 发送消息
    if request.predefined_options.is_some() {
        core.send_message_with_options(
            &request.message,
            &request.predefined_options.unwrap(),
            request.is_markdown
        ).await?;
    } else {
        core.send_message_with_markdown(
            &request.message,
            request.is_markdown
        ).await?;
    }

    // 3. 等待响应（通过轮询或 webhook）
    let response = wait_for_telegram_response(&core).await?;

    Ok(response)
}
```

### 5. 连接测试 (`core.rs`)

#### 测试流程
```rust
pub async fn test_telegram_connection(
    bot_token: String,
    chat_id: String,
    api_url: Option<String>
) -> Result<String> {
    // 1. 创建核心
    let core = TelegramCore::new_with_api_url(bot_token, chat_id, api_url)?;

    // 2. 发送测试消息
    core.send_message("🎉 三术 Telegram 连接测试成功！").await?;

    Ok("连接成功".to_string())
}
```

---

## 事件类型

### TelegramEvent
```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelegramEvent {
    /// 选项状态变化
    OptionToggled { option: String, selected: bool },

    /// 文本输入更新
    TextUpdated { text: String },

    /// 继续按钮点击
    ContinuePressed,

    /// 发送按钮点击
    SendPressed,
}
```

---

## 数据流程

### 消息发送流程
```
MCP 请求 → 创建 TelegramCore → 处理 Markdown → 构建键盘 → 发送消息 → 返回 MessageId
```

### 响应接收流程
```
用户点击按钮 → Telegram 回调 → handle_callback_query → 发送事件到前端 → 更新 UI
```

---

## 常见问题 (FAQ)

### Q: 如何获取 Bot Token？
A: 在 Telegram 中与 @BotFather 对话，创建新 Bot 获取 Token

### Q: 如何获取 Chat ID？
A: 与 Bot 对话后，访问 `https://api.telegram.org/bot<token>/getUpdates` 查看

### Q: 如何使用自定义 API 端点？
A: 在配置中设置 `api_url`（用于代理或自建 Bot API）

### Q: Markdown 渲染失败怎么办？
A: 检查特殊字符是否正确转义，或设置 `is_markdown: false`

### Q: 如何调试 Telegram 集成？
A: 设置 `RUST_LOG=debug` 查看详细日志

---

## 相关文件清单

### 核心文件
- `core.rs` - Telegram 核心功能
- `markdown.rs` - Markdown 处理
- `mcp_handler.rs` - MCP 集成
- `integration.rs` - 集成逻辑
- `commands.rs` - Tauri 命令
- `mod.rs` - 模块导出

---

## 使用示例

### 发送普通消息
```rust
let core = TelegramCore::new(bot_token, chat_id)?;
core.send_message("Hello, Telegram!").await?;
```

### 发送 Markdown 消息
```rust
let message = r#"
## 代码审查结果

- ✅ 语法正确
- ⚠️ 性能问题

**建议**: 优化循环
"#;

core.send_message_with_markdown(message, true).await?;
```

### 发送带选项的消息
```rust
let options = vec!["修复".to_string(), "忽略".to_string()];
let msg_id = core.send_message_with_options(
    "发现性能问题，如何处理？",
    &options,
    false
).await?;
```

### 测试连接
```rust
let result = test_telegram_connection(
    "123456:ABC-DEF".to_string(),
    "123456789".to_string(),
    None
).await?;
```

---

**最后更新**: 2026-02-18
