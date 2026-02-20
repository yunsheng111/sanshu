# 提示词增强工具 (enhance)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **enhance**

---

## 模块职责

提示词增强工具 (enhance)，通过统一 Chat 客户端将口语化提示词转换为结构化专业提示词。支持三级降级链（Ollama 本地 -> 云端 API -> 规则引擎）、流式响应、历史对话整合、项目上下文注入和结果缓存。

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
    pub prompt: String,                    // 原始提示词
    pub project_root_path: Option<String>, // 项目根路径（用于上下文注入）
}
```

---

## 核心功能

### 1. 统一 Chat 客户端 (`chat_client.rs`) - P1 新增

支持多提供者的统一 Chat 接口，三段超时 + 懒构建 reqwest::Client。

```rust
pub enum ChatProvider {
    Ollama,        // 本地 Ollama
    OpenAICompat,  // OpenAI / Grok / DeepSeek / SiliconFlow / Groq / Cloudflare
    Gemini,        // Google Gemini 原生格式
    Anthropic,     // Anthropic Claude 原生格式
    RuleEngine,    // 纯规则降级（无 API）
}

pub struct ChatClient {
    provider: ChatProvider,
    base_url: String,
    api_key: Option<String>,
    model: String,
}
```

### 2. 三级降级链 (`provider_factory.rs`) - P1 新增

```
L1: Ollama 本地 → 检测可用性 → 有则使用
    |
    v (不可用)
L2: 云端 API → 读取配置中的 enhance_provider 和 enhance_api_key
    |           支持：openai, grok, deepseek, siliconflow, groq, cloudflare, gemini, anthropic
    v (不可用)
L3: 规则引擎 → 10 条内置规则 → 正则匹配 → 模板增强
```

```rust
/// 从配置构建 ChatClient
pub fn build_enhance_client(config: &McpConfig) -> ChatClient
```

### 3. 规则引擎 (`rule_engine.rs`) - P1 新增

无 API 时的纯规则降级增强，10 条内置规则覆盖常见场景：

```rust
pub struct RuleEnhancer {
    rules: Vec<EnhanceRule>,     // 10 条内置规则
    strategy: RuleMatchStrategy, // FirstMatch（默认）或 AllMatch
}

pub struct EnhanceRule {
    pub trigger: Regex,    // 触发正则
    pub template: String,  // 追加模板
    pub priority: u32,     // 优先级
}
```

**内置规则示例**：
- `fix|bug|error` -> 追加诊断步骤模板
- `test|测试` -> 追加测试策略模板
- `性能|optimize` -> 追加性能分析模板

### 4. 结果缓存 (`cache.rs`) - P1 新增

LRU 缓存，减少重复增强请求的 API 调用。

```rust
pub struct EnhanceCache {
    entries: HashMap<String, EnhanceCacheEntry>,
    max_entries: usize,  // 默认 50
    ttl: Duration,       // 默认 10 分钟
}
```

**缓存策略**：
- **缓存键**: 原始提示词的 hash
- **淘汰策略**: LRU（最久未访问优先淘汰）
- **过期策略**: TTL 10 分钟

### 5. 提示词增强核心 (`core.rs`)

```
增强流程：
1. 检查缓存 → 命中则直接返回
2. 构建请求负载（加载历史 + 注入上下文 + zhi 历史摘要）
3. 调用 ChatClient（三级降级链）
4. 解析响应（提取 <augment-enhanced-prompt> 标签内容）
5. 保存到缓存
6. 保存历史对话
7. 返回增强结果
```

### 6. 历史对话整合 (`history.rs`)

- **存储位置**: `.sanshu-memory/enhance_history.json`
- **最大条数**: 10
- **功能**: 加载/保存/添加/清理历史对话

### 7. 上下文注入

增强请求自动注入：
- 历史对话（最近 10 条）
- zhi 交互历史摘要（最近 5 条）
- 项目上下文（如提供 project_root_path）

---

## 数据流程

### 增强流程（含缓存和降级）
```
AI 请求
    → 检查 EnhanceCache
    → [命中] 返回缓存结果
    → [未命中] build_enhance_client(config)
        → [L1] Ollama 本地 → 成功返回
        → [L2] 云端 API → 成功返回
        → [L3] RuleEnhancer → 规则增强返回
    → 存入缓存
    → 保存历史
    → 返回增强结果
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = [
  "stream",
  "json"
] }
tokio = { version = "1.0", features = [
  "fs",
  "sync",
  "time"
] }
futures-util = "0.3"
regex = "1.0"
anyhow = "1.0"
```

### 配置字段（McpConfig 扩展）
```rust
// Ollama 本地（L1）
pub enhance_ollama_url: Option<String>,
pub enhance_ollama_model: Option<String>,  // 默认 "qwen2.5-coder:7b"

// 云端 API（L2）
pub enhance_provider: Option<String>,      // "openai", "gemini", "anthropic" 等
pub enhance_api_key: Option<String>,
pub enhance_model: Option<String>,
pub enhance_base_url: Option<String>,
```

---

## 常见问题 (FAQ)

### Q: 如何配置 Ollama 本地增强？
A: 设置 `enhance_ollama_url`（如 `http://localhost:11434`）和 `enhance_ollama_model`

### Q: 没有 API 怎么使用增强？
A: 系统会自动降级到规则引擎（L3），使用 10 条内置规则进行结构化增强

### Q: 缓存如何清理？
A: 缓存 10 分钟自动过期，或重启 MCP 服务器清空

### Q: 支持哪些云端 API？
A: OpenAI、Grok(xAI)、DeepSeek、SiliconFlow、Groq、Cloudflare、Gemini、Anthropic

---

## 相关文件清单

### 核心文件
- `core.rs` - 提示词增强核心逻辑
- `chat_client.rs` - P1 统一 Chat 客户端（多提供者）
- `provider_factory.rs` - P1 三级降级链工厂
- `rule_engine.rs` - P1 规则引擎（10 条内置规则）
- `cache.rs` - P1 结果缓存（LRU + TTL）
- `history.rs` - 历史对话管理
- `mcp.rs` - MCP 工具实现
- `types.rs` - 数据类型定义
- `commands.rs` - Tauri 命令
- `utils.rs` - 工具函数
- `mod.rs` - 模块导出

### 数据文件
- `.sanshu-memory/enhance_history.json` - 历史对话存储

---

**最后更新**: 2026-02-19
