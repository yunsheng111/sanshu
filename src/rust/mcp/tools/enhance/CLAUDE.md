# 提示词增强工具 (enhance)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **enhance**

---

## 模块职责

提示词增强工具 (enhance)，通过调用 Augment chat-stream API 将口语化提示词转换为结构化专业提示词，支持流式响应、历史对话整合和项目上下文注入。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `enhance`
- **标识符**: `mcp______enhance`
- **状态**: 默认关闭（可选启用）

### 核心结构
```rust
pub struct EnhanceTool;

impl EnhanceTool {
    pub async fn enhance(request: EnhanceMcpRequest) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "enhance",
  "arguments": {
    "prompt": "帮我优化一下登录页面",
    "project_root_path": "/path/to/project"
  }
}
```

### 请求参数
```rust
pub struct EnhanceMcpRequest {
    /// 原始提示词
    pub prompt: String,

    /// 项目根路径（用于上下文注入）
    pub project_root_path: Option<String>,
}
```

### 响应格式
```markdown
### 增强后的提示词

请优化登录页面的用户体验，具体要求：

1. **UI 改进**：
   - 使用现代化的表单设计
   - 添加输入验证提示
   - 优化移动端适配

2. **功能增强**：
   - 添加"记住我"选项
   - 实现密码可见性切换
   - 添加第三方登录（Google, GitHub）

3. **安全性**：
   - 实现 CSRF 保护
   - 添加登录失败限制
   - 使用 HTTPS

4. **技术栈**：
   - 前端：Vue 3 + Naive UI
   - 后端：Rust + Tauri
   - 认证：JWT Token

请提供完整的实现方案和代码示例。
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = ["stream", "json"] }
tokio = { version = "1.0", features = ["fs", "sync", "time"] }
futures-util = "0.3"
regex = "1.0"
anyhow = "1.0"
```

### 配置结构
```rust
pub struct EnhanceConfig {
    /// Augment API 基础 URL
    pub base_url: String,

    /// API Token
    pub token: String,

    /// 项目根路径
    pub project_root: Option<String>,
}
```

### 默认配置
- **base_url**: `https://api.augmentcode.com`
- **token**: 从环境变量 `AUGMENT_TOKEN` 读取

---

## 核心功能

### 1. 提示词增强 (`core.rs`)

#### 增强流程
```rust
pub struct PromptEnhancer {
    base_url: String,
    token: String,
    client: Client,
    project_root: Option<String>,
}

impl PromptEnhancer {
    /// 增强提示词（流式响应）
    pub async fn enhance_prompt(&self, prompt: &str) -> Result<String> {
        // 1. 构建请求负载
        let payload = self.build_payload(prompt).await?;

        // 2. 调用 chat-stream API
        let mut stream = self.call_chat_stream_api(payload).await?;

        // 3. 解析流式响应
        let enhanced = self.parse_stream_response(&mut stream).await?;

        // 4. 提取增强内容
        self.extract_enhanced_prompt(&enhanced)
    }
}
```

#### 系统提示词
```rust
const ENHANCE_SYSTEM_PROMPT: &str = r#"⚠️ NO TOOLS ALLOWED ⚠️

Here is an instruction that I'd like to give you, but it needs to be improved.
Rewrite and enhance this instruction to make it clearer, more specific, less ambiguous,
and correct any mistakes. Do not use any tools: reply immediately with your answer.

Reply with the following format:

### BEGIN RESPONSE ###
Here is an enhanced version of the original instruction:
<augment-enhanced-prompt>enhanced prompt goes here</augment-enhanced-prompt>
### END RESPONSE ###

Here is my original instruction:
"#;
```

### 2. 历史对话整合 (`history.rs`)

#### 历史管理器
```rust
pub struct ChatHistoryManager {
    history_dir: PathBuf,
}

impl ChatHistoryManager {
    /// 加载历史对话
    pub fn load_history(&self, project_root: &str) -> Result<Vec<ChatMessage>>

    /// 保存历史对话
    pub fn save_history(&self, project_root: &str, messages: &[ChatMessage]) -> Result<()>

    /// 添加消息
    pub fn add_message(&self, project_root: &str, role: &str, content: &str) -> Result<()>

    /// 清理旧历史（保留最近 N 条）
    pub fn cleanup_old_history(&self, project_root: &str, keep_count: usize) -> Result<()>
}
```

#### 历史格式
```rust
pub struct ChatMessage {
    pub role: String,      // "user" | "assistant"
    pub content: String,
    pub timestamp: String,
}
```

#### 存储位置
- **路径**: `.sanshu-memory/enhance_history.json`
- **最大条数**: 10

### 3. 上下文注入

#### 项目上下文
```rust
async fn build_payload(&self, prompt: &str) -> Result<BuildPayloadResult> {
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: ENHANCE_SYSTEM_PROMPT.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    ];

    // 1. 加载历史对话
    if let Some(project_root) = &self.project_root {
        let history = ChatHistoryManager::load_history(project_root)?;
        messages.extend(history);
    }

    // 2. 添加当前提示词
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
        timestamp: Utc::now().to_rfc3339(),
    });

    // 3. 注入 zhi 历史摘要
    if let Some(project_root) = &self.project_root {
        let zhi_summary = self.build_zhi_history_summary(project_root)?;
        if !zhi_summary.is_empty() {
            messages.insert(1, ChatMessage {
                role: "system".to_string(),
                content: format!("最近的交互历史：\n{}", zhi_summary),
                timestamp: Utc::now().to_rfc3339(),
            });
        }
    }

    Ok(BuildPayloadResult {
        payload: json!({ "messages": messages }),
        history_diag: HistoryBuildDiagnostics::default(),
    })
}
```

#### zhi 历史摘要
```rust
fn build_zhi_history_summary(&self, project_root: &str) -> Result<String> {
    let zhi_history = ZhiHistoryManager::load_history(project_root)?;

    // 保留最近 5 条
    let recent = zhi_history.iter().rev().take(5);

    let mut summary = String::new();
    for entry in recent {
        summary.push_str(&format!(
            "- [{}] {}\n  响应: {}\n",
            entry.timestamp,
            truncate(&entry.message, 200),
            truncate(&entry.response, 200)
        ));
    }

    Ok(summary)
}
```

### 4. 流式响应解析

#### SSE 解析
```rust
async fn parse_stream_response(&self, stream: &mut impl Stream<Item = Result<Bytes>>) -> Result<String> {
    let mut full_response = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        // 解析 SSE 格式
        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }

                // 解析 JSON
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                        full_response.push_str(content);
                    }
                }
            }
        }
    }

    Ok(full_response)
}
```

#### 内容提取
```rust
fn extract_enhanced_prompt(&self, response: &str) -> Result<String> {
    // 提取 <augment-enhanced-prompt> 标签内容
    let re = Regex::new(r"<augment-enhanced-prompt>(.*?)</augment-enhanced-prompt>")?;

    if let Some(captures) = re.captures(response) {
        Ok(captures[1].trim().to_string())
    } else {
        // 降级：返回整个响应
        Ok(response.trim().to_string())
    }
}
```

---

## 数据流程

### 增强流程
```
AI 请求 → 加载历史 → 注入上下文 → 调用 API → 解析流式响应 → 提取增强内容 → 保存历史 → 返回结果
```

### 历史管理
```
增强请求 → 加载历史 → 添加到上下文 → 增强完成 → 保存新历史 → 清理旧历史
```

---

## 常见问题 (FAQ)

### Q: 如何配置 API Token？
A: 设置环境变量 `AUGMENT_TOKEN` 或在配置文件中设置 `enhance_config.token`

### Q: 历史对话保留多少条？
A: 默认保留最近 10 条

### Q: 如何禁用历史对话？
A: 不提供 `project_root_path` 参数

### Q: 支持哪些语言？
A: 支持所有语言，但增强效果以英文最佳

### Q: 如何查看增强历史？
A: 查看 `.sanshu-memory/enhance_history.json`

---

## 相关文件清单

### 核心文件
- `core.rs` - 提示词增强核心逻辑
- `history.rs` - 历史对话管理
- `mcp.rs` - MCP 工具实现
- `types.rs` - 数据类型定义
- `commands.rs` - Tauri 命令
- `mod.rs` - 模块导出

### 数据文件
- `.sanshu-memory/enhance_history.json` - 历史对话存储

---

## 使用示例

### 基础增强
```rust
let request = EnhanceMcpRequest {
    prompt: "帮我优化一下登录页面".to_string(),
    project_root_path: Some("/path/to/project".to_string()),
};

let result = EnhanceTool::enhance(request).await?;
```

### 无上下文增强
```rust
let request = EnhanceMcpRequest {
    prompt: "写一个快速排序算法".to_string(),
    project_root_path: None,
};

let result = EnhanceTool::enhance(request).await?;
```

---

**最后更新**: 2026-02-18
