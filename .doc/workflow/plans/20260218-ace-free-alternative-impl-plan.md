# 实施计划：Augment Code API 免费平替方案

> **基于**：`.doc/agent-teams/research/20260217-ace-free-alternative-research.md` v5
> **审查报告**：`.doc/workflow/reviews/20260218-ace-free-alternative-review.md`
> **日期**：2026-02-18
> **目标**：在不依赖 Augment Code 付费 API 的前提下，实现 `enhance` 工具替换 + `sou` 本地索引双模式

---

## 总体策略

三个阶段串行推进，每阶段独立可验收：

| 阶段 | 目标 | 预估工作量 | 前置条件 |
|------|------|-----------|---------|
| 阶段 1 | 统一 API 接口层 + enhance 替换 | 3-4 天 | 无 |
| 阶段 2 | 本地向量索引（sou 替换） | 4-5 天 | 阶段 1 完成 |
| 阶段 3 | 混合检索 + 前端设置页 | 2-3 天 | 阶段 2 完成 |

---

## 阶段 1：统一 API 接口层 + enhance 替换

### 目标
用 `ChatClient` 统一接口替换 `enhance/core.rs` 中硬编码的 Augment API 调用，支持 Ollama / OpenAI 兼容 / 规则引擎三级降级链。

### Step 1.1：新建 `chat_client` 模块

**文件**：`src/rust/mcp/tools/enhance/chat_client.rs`（新建）

```rust
use crate::constants::network::{CONNECTION_TIMEOUT_MS, DEFAULT_TIMEOUT_MS};

/// 支持的 Chat 提供者
#[derive(Debug, Clone, PartialEq)]
pub enum ChatProvider {
    Ollama,
    OpenAICompat,  // SiliconFlow / Groq / Gemini / Cloudflare 等
    RuleEngine,    // 无 API 时的纯规则降级
}

/// 统一 Chat 客户端（三段超时 + 懒构建 reqwest::Client）
pub struct ChatClient {
    pub provider: ChatProvider,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub connect_timeout_ms: u64,   // 默认 CONNECTION_TIMEOUT_MS (10_000)
    pub request_timeout_ms: u64,   // 默认 DEFAULT_TIMEOUT_MS (30_000)
    pub stream_timeout_ms: u64,    // 流式专用，默认 120_000
}

impl ChatClient {
    pub fn new(provider: ChatProvider, base_url: String,
               api_key: Option<String>, model: String) -> Self { ... }

    /// 按 provider 类型自动选择 Ollama 或 OpenAI 兼容格式
    pub async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> { ... }

    /// 流式调用（on_chunk 回调逐块处理）
    pub async fn chat_stream<F>(&self, messages: &[Message], on_chunk: F)
        -> anyhow::Result<String>
    where F: FnMut(&str) + Send { ... }

    /// 带重试的 chat（复用 acemcp/mcp.rs 的 retry_request()）
    pub async fn chat_with_retry(&self, messages: &[Message]) -> anyhow::Result<String> { ... }

    fn build_client(&self, is_stream: bool) -> anyhow::Result<reqwest::Client> { ... }
}
```

**关键约束**：
- `build_client()` 必须同时设置 `connect_timeout` 和 `timeout`，不能只设一个
- Ollama 本地模式：`connect_timeout=3_000`，`stream_timeout=300_000`
- `chat_with_retry()` 直接调用 `acemcp::mcp::retry_request()`，不重新实现

---

### Step 1.2：新建 `rule_engine` 模块

**文件**：`src/rust/mcp/tools/enhance/rule_engine.rs`（新建）

```rust
/// 规则匹配策略
#[derive(Debug, Clone)]
pub enum RuleMatchStrategy {
    FirstMatch,   // 首次匹配后停止（默认，避免规则冲突）
    AllMatch,     // 全部匹配（叠加应用）
}

pub struct EnhanceRule {
    pub trigger: regex::Regex,
    pub template: String,
    pub priority: u32,
}

pub struct RuleEnhancer {
    rules: Vec<EnhanceRule>,
    strategy: RuleMatchStrategy,
}

impl RuleEnhancer {
    pub fn new_default() -> Self { ... }  // 内置 10 条基础规则
    pub fn enhance(&self, prompt: &str, context: &EnhanceContext) -> String { ... }
}
```

**内置规则示例**（10 条，覆盖常见场景）：
- `fix|bug|error` → 补充错误上下文模板
- `refactor|重构` → 补充重构约束模板
- `test|测试` → 补充测试覆盖要求模板
- `doc|文档` → 补充文档格式模板

---

### Step 1.3：新建 `provider_factory` 模块

**文件**：`src/rust/mcp/tools/enhance/provider_factory.rs`（新建）

```rust
/// 从 McpConfig 构建 ChatClient，实现三级降级链
pub fn build_enhance_client(config: &McpConfig) -> ChatClient {
    // L1：Ollama 本地
    if let Some(ollama_url) = &config.enhance_ollama_url {
        if is_ollama_available(ollama_url) {
            return ChatClient::new(ChatProvider::Ollama, ...);
        }
    }
    // L2：OpenAI 兼容（SiliconFlow / Groq 等）
    if let (Some(base_url), Some(api_key)) = (&config.enhance_base_url, &config.enhance_api_key) {
        return ChatClient::new(ChatProvider::OpenAICompat, ...);
    }
    // L3：规则引擎（无需 API）
    ChatClient::new(ChatProvider::RuleEngine, String::new(), None, String::new())
}

/// 异步检测 Ollama 是否可用（超时 3s）
async fn is_ollama_available(url: &str) -> bool { ... }
```

---

### Step 1.4：修改 `enhance/core.rs`

**文件**：`src/rust/mcp/tools/enhance/core.rs`（修改）

修改要点：
1. 删除 `PromptEnhancer` 中硬编码的 Augment API URL 和 120s 超时
2. 替换为调用 `provider_factory::build_enhance_client(config)`
3. 在所有 `log::info!` 中的 API Key 输出替换为 `mask_api_key(key)`
4. `from_acemcp_config()` 改为 `from_mcp_config()`，读取新的 `enhance_*` 字段

```rust
// 修改前（硬编码）
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(120))
    .build()?;
let url = format!("{}/chat-stream", self.base_url);

// 修改后（统一接口）
let chat_client = provider_factory::build_enhance_client(&self.config);
let result = chat_client.chat_with_retry(&messages).await?;
```

---

### Step 1.5：向 `McpConfig` 添加新字段

**文件**：`src/rust/config/settings.rs`（修改）

在 `McpConfig` 结构体末尾追加（保持向后兼容，全部 `Option`）：

```rust
// enhance 工具配置（v5 新增，统一 API 接口层）
pub enhance_provider: Option<String>,      // "ollama" | "openai_compat" | "rule_engine"
pub enhance_base_url: Option<String>,      // OpenAI 兼容 API 端点
pub enhance_api_key: Option<String>,       // API Key（日志中脱敏显示）
pub enhance_model: Option<String>,         // 模型名称
pub enhance_ollama_url: Option<String>,    // Ollama 端点（默认 http://localhost:11434）
pub enhance_ollama_model: Option<String>,  // Ollama 模型（默认 qwen2.5-coder:7b）
```

同步更新 `default_mcp_config()` 函数，为新字段补充 `None` 默认值。

**命名规范**：全部使用 `enhance_` 前缀，与现有 `acemcp_*` 风格一致。

---

### Step 1.6：新建 `mask_api_key` 工具函数

**文件**：`src/rust/mcp/tools/enhance/mod.rs` 或新建 `src/rust/mcp/tools/enhance/utils.rs`

```rust
/// API Key 脱敏（用于日志，防止泄露）
pub fn mask_api_key(key: &str) -> String {
    match key.len() {
        0 => "(空)".to_string(),
        1..=8 => "****".to_string(),
        _ => format!("{}****{}", &key[..4], &key[key.len()-4..]),
    }
}
```

扫描 `enhance/` 目录下所有 `log::info!`、`log::debug!` 中含 `key`、`token`、`api` 的输出，替换为 `mask_api_key()` 调用。

---

### 阶段 1 验收标准

| 验收项 | 验证方式 |
|--------|---------|
| Ollama 本地模式可用 | 启动 Ollama，调用 `enhance` 工具，返回增强后 prompt |
| OpenAI 兼容模式可用 | 配置 SiliconFlow API Key，调用 `enhance` 工具 |
| 规则引擎降级 | 不配置任何 API，调用 `enhance` 工具，返回规则增强结果 |
| 降级链自动切换 | Ollama 不可用时自动切换到 L2/L3 |
| API Key 不出现在日志 | `RUST_LOG=debug` 运行，grep 日志确认无明文 Key |
| 超时配置生效 | 模拟网络延迟，确认 connect_timeout 先触发 |
| `cargo test` 通过 | `cargo test --package sanshu` 无新增失败 |

---

## 阶段 2：本地向量索引（sou 替换）

### 目标
用本地 BM25 + 向量索引替换 `sou` 工具对 Augment Code 的依赖，支持 Jina / SiliconFlow / Ollama 嵌入模型。

### Step 2.1：新建 `embedding_client` 模块

**文件**：`src/rust/mcp/tools/acemcp/embedding_client.rs`（新建）

```rust
pub enum EmbeddingProvider {
    Jina,
    SiliconFlow,
    Cloudflare,
    Nomic,
    Cohere,
    Ollama,
}

pub struct EmbeddingClient {
    provider: EmbeddingProvider,
    base_url: String,
    api_key: String,
    model: String,
    dimension: usize,
    batch_size: usize,           // 按提供者设置（Jina=100, Cohere=50, Ollama=20）
    rate_limit_rpm: Option<u32>, // 限速（Jina=60, Cohere Trial=10）
}

impl EmbeddingClient {
    pub async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> { ... }
    pub async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> { ... }
    // 内部：分批 + 限速
    async fn embed_batch_internal(&self, chunk: &[String]) -> anyhow::Result<Vec<Vec<f32>>> { ... }
}
```

**批量限速实现**：使用 `tokio::time::sleep` 实现简单令牌桶，不引入额外依赖。

---

### Step 2.2：新建 `local_index` 模块

**文件**：`src/rust/mcp/tools/acemcp/local_index.rs`（新建）

```rust
pub struct LocalIndexManager {
    index_lock: tokio::sync::RwLock<()>,  // 并发安全
    index_path: PathBuf,
    embedding_client: EmbeddingClient,
}

impl LocalIndexManager {
    /// 增量更新索引（写锁保护 + 原子写入）
    pub async fn update_index(&self, files: &[PathBuf]) -> anyhow::Result<()> {
        let _guard = self.index_lock.write().await;
        // 1. 读取现有索引
        // 2. 计算文件 hash，跳过未变更文件
        // 3. 生成新文件的嵌入向量
        // 4. 原子写入：先写 tmp 文件，再 rename 替换
    }

    /// 向量搜索（读锁保护）
    pub async fn search(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<SearchResult>> {
        let _guard = self.index_lock.read().await;
        // 余弦相似度搜索
    }
}
```

**原子写入**：
```rust
// 写入临时文件后 rename，防止写入中途崩溃导致索引损坏
let tmp_path = index_path.with_extension("tmp");
std::fs::write(&tmp_path, &serialized)?;
std::fs::rename(&tmp_path, &index_path)?;
```

---

### Step 2.3：向 `McpConfig` 添加 sou 嵌入配置字段

**文件**：`src/rust/config/settings.rs`（修改）

```rust
// sou 本地索引配置（v5 新增）
pub sou_embedding_provider: Option<String>,  // "jina" | "siliconflow" | "ollama" 等
pub sou_embedding_base_url: Option<String>,
pub sou_embedding_api_key: Option<String>,
pub sou_embedding_model: Option<String>,
pub sou_mode: Option<String>,                // "local" | "acemcp"（默认 "acemcp"）
pub sou_index_path: Option<String>,          // 索引存储路径（默认 .sanshu-index/）
```

---

### 阶段 2 验收标准

| 验收项 | 验证方式 |
|--------|---------|
| Jina 嵌入可用 | 配置 Jina API Key，对 10 个文件建立索引 |
| Ollama 嵌入可用 | 启动 Ollama nomic-embed-text，建立索引 |
| 并发安全 | 同时触发 2 次索引更新，无数据竞争 |
| 原子写入 | 写入中途 kill 进程，索引文件不损坏 |
| 批量限速 | Cohere Trial 模式下，RPM 不超过 10 |
| 搜索结果相关性 | 搜索 "ChatClient 超时配置"，返回 enhance/chat_client.rs |

---

## 阶段 3：混合检索 + 前端设置页

### 目标
实现 BM25 + 向量 RRF 混合检索，并在前端设置页面添加 enhance/sou 提供者配置 UI。

### Step 3.1：实现 BM25 + RRF 混合检索

**文件**：`src/rust/mcp/tools/acemcp/hybrid_search.rs`（新建）

```rust
/// RRF 融合（k 值可配置，默认 60.0）
pub fn rrf_merge(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    k: f32,  // 从 McpConfig.sou_rrf_k 读取，默认 60.0
) -> Vec<SearchResult> { ... }
```

---

### Step 3.2：前端设置页面新增配置项

**文件**：`src/frontend/components/settings/`（修改现有设置组件）

新增两个配置区块：

**enhance 配置区块**：
- 提供者选择（Ollama / OpenAI 兼容 / 规则引擎）
- base_url 输入框
- API Key 输入框（密码类型，显示脱敏值）
- 模型名称输入框
- 连接测试按钮（调用 `is_available()` 健康检查）

**sou 配置区块**：
- 模式选择（本地索引 / acemcp）
- 嵌入提供者选择
- API Key 输入框
- 索引路径配置

---

### 阶段 3 验收标准

| 验收项 | 验证方式 |
|--------|---------|
| 混合检索结果优于单一检索 | 对比 BM25 / 向量 / RRF 三种结果的相关性 |
| 前端配置可保存 | 修改 enhance 提供者，重启后配置持久化 |
| 连接测试按钮可用 | 点击测试按钮，显示延迟和可用状态 |
| API Key 不明文显示 | 设置页面 Key 输入框为 password 类型 |

---

## 文件变更清单

### 新建文件

| 文件 | 阶段 | 说明 |
|------|------|------|
| `src/rust/mcp/tools/enhance/chat_client.rs` | 1 | 统一 Chat 客户端 |
| `src/rust/mcp/tools/enhance/rule_engine.rs` | 1 | 规则引擎降级 |
| `src/rust/mcp/tools/enhance/provider_factory.rs` | 1 | 提供者工厂 + 降级链 |
| `src/rust/mcp/tools/enhance/utils.rs` | 1 | mask_api_key 等工具函数 |
| `src/rust/mcp/tools/acemcp/embedding_client.rs` | 2 | 嵌入客户端 |
| `src/rust/mcp/tools/acemcp/local_index.rs` | 2 | 本地向量索引管理 |
| `src/rust/mcp/tools/acemcp/hybrid_search.rs` | 3 | BM25 + RRF 混合检索 |

### 修改文件

| 文件 | 阶段 | 修改内容 |
|------|------|---------|
| `src/rust/mcp/tools/enhance/core.rs` | 1 | 替换硬编码 API 调用，接入 ChatClient |
| `src/rust/config/settings.rs` | 1+2 | McpConfig 追加 enhance_* 和 sou_* 字段 |
| `src/rust/mcp/tools/enhance/mod.rs` | 1 | 导出新模块 |
| `src/rust/mcp/tools/acemcp/mod.rs` | 2 | 导出新模块 |
| `src/frontend/components/settings/` | 3 | 新增配置 UI |

---

## 依赖变更

### `Cargo.toml` 新增（阶段 2）

```toml
# 向量存储序列化（已有 serde，仅需 bincode）
bincode = "1.3"

# BM25 实现（可选，也可自行实现）
# bm25 = "0.1"  # 如果 crate 质量不足则手动实现

# tree-sitter AST 分片（可选，阶段 2 后期）
# tree-sitter = { version = "0.22", optional = true }
# [features]
# ast-chunking = ["tree-sitter"]
```

---

## 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| Ollama 本地模型质量不足 | 中 | 中 | 规则引擎作为最终降级，保证基础可用 |
| 嵌入 API 免费额度耗尽 | 低 | 高 | 支持多提供者切换，Ollama 本地作为备选 |
| 向量索引文件损坏 | 低 | 高 | 原子写入 + 索引重建命令 |
| reqwest 版本与现有冲突 | 低 | 中 | 复用现有 `create_http_client()`，不引入新版本 |
| 阶段 1 破坏现有 enhance 功能 | 中 | 高 | 保留 `from_acemcp_config()` 兼容路径，新字段全部 Option |

---

## 执行顺序建议

```
Step 1.6（mask_api_key）→ Step 1.1（ChatClient）→ Step 1.2（RuleEngine）
→ Step 1.3（ProviderFactory）→ Step 1.5（McpConfig 字段）→ Step 1.4（core.rs 修改）
→ 阶段 1 验收
→ Step 2.1（EmbeddingClient）→ Step 2.3（McpConfig sou 字段）→ Step 2.2（LocalIndex）
→ 阶段 2 验收
→ Step 3.1（HybridSearch）→ Step 3.2（前端 UI）
→ 阶段 3 验收
```

**先做 Step 1.6 的原因**：mask_api_key 是独立工具函数，零风险，且后续所有步骤都会用到。
