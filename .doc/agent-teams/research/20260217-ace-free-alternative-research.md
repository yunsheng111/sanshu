# 三术（sanshu）免费平替 Augment Code API — 可行性方案 v3

> **最新状态**：ace 的上下文检索（sou）仍可用，仅提示词增强（enhance）不可用。
>
> **双轨策略**：
> - **enhance（优先实施）**：替换 Augment chat-stream API，支持 Ollama / OpenAI / Gemini 等统一接口
> - **sou 索引（双模式）**：保留 ace API（当前可用）+ 新增免费 Embedding API 备选（ace 不可用时切换）
> - **统一 API 接口层**：所有外部 API 调用统一为 OpenAI 兼容格式，一套代码适配所有提供者
>
> 设计原则：集成现有免费 API / 本地 LLM，存储尽可能轻量参考记忆模式

## 一、参考项目分析

### 5 个 GitHub 项目对比

| 项目 | 核心方案 | 嵌入来源 | 存储方式 | 可借鉴点 |
|------|---------|---------|---------|---------|
| **ContextWeaver** | 免费 Embedding API + LanceDB + SQLite FTS5 + Tree-sitter AST 分片 + RRF 混合检索 + Reranker | SiliconFlow 免费 API（bge-m3, 1024维） | LanceDB（向量）+ SQLite FTS5（全文） | **最佳参考**：免费 API + 混合检索 + AST 分片 + 三阶段上下文扩展 |
| **fast-context-mcp** | Windsurf Devstral API 生成 ripgrep 命令，多轮搜索 | 无嵌入（AI 驱动搜索） | 无持久化 | 零索引方案的兜底思路：AI 生成搜索命令 |
| **nocturne_memory** | URI 路径 + SQLite，无向量，结构化语义 | 无 | SQLite（2 张表：memories + paths） | **轻量存储参考**：URI 路径组织 + 版本快照 + 极简架构 |
| **claude-mem** | AI 压缩观察 → 3 层渐进检索 → 注入 CLAUDE.md | Chroma 向量库（可选） | SQLite + 文件系统 | **3 层渐进检索** + 观察压缩 + OpenAI 兼容 API + token 成本可见性 |
| **ace-tool-rs** | 仍调 Augment API（Rust 重写） | 远程 Augment API | 远程 | AIMD 自适应上传策略、多编码支持（已有） |

### nocturne_memory 深度分析

**核心架构**：拒绝 RAG/向量数据库，回归 URI 路径 + 结构化语义。Python + FastAPI + SQLite，仅两张表。

**存储优势**：

1. **内容-路径分离**（memories + paths 两张表）：一条记忆可有多个 URI 路径入口，每个路径有独立触发条件。类似文件系统硬链接，比 ji 的 `id → content` 一对一灵活得多。
2. **URI 层级命名空间**（`domain://path/subpath`）：天然支持层级查询和前缀匹配。`core://agent`、`writer://novel`、`system://boot` 等，用户可自定义任意命名空间。比 ji 的四分类（Rule/Preference/Pattern/Context）灵活。
3. **版本控制 + 快照回滚**：每次修改前自动创建 Snapshot，支持一键回滚。ji 中完全没有此机制。
4. **启动引导**（`system://boot`）：通过 `CORE_MEMORY_URIS` 配置，AI 启动时自动加载核心记忆。主动推送而非被动搜索。
5. **priority 权重系统**：高优先级记忆检索时排在前面，比 ji 的纯时间排序更智能。

**搜寻优势**：

- **URI 前缀匹配**：零成本结构化检索，不需要嵌入模型。查 `core://agent` → 返回所有 agent 相关记忆。
- **特殊入口**：`system://index`（全量索引）、`system://recent`（最近修改），内置常用查询模式。

**可借鉴到三术的点**：

| 借鉴点 | nocturne 做法 | 三术应用 |
|--------|-------------|---------|
| 内容-路径分离 | memories + paths 两张表 | 代码块可有多个语义入口（函数名、类名、文件路径） |
| URI 命名空间 | `domain://path` 层级 | 索引按 `project://module/file/chunk` 组织，前缀查询快速缩小范围 |
| 快照回滚 | 修改前自动 Snapshot | 索引重建前保留旧索引，失败可回滚 |
| 启动引导 | `system://boot` 自动加载 | enhance 自动注入项目核心上下文（技术栈、架构约定） |
| priority 权重 | 记忆有优先级 | 搜索结果中核心文件/高频访问 chunk 权重更高 |

**局限**：无语义搜索（纯 URI 匹配），无法处理自然语言查询；Python 生态无法直接移植 Rust。

### claude-mem 深度分析

**核心机制**：Claude Code 插件，通过钩子捕获工具调用 → AI 压缩为 ~500 token 摘要 → SQLite 存储 → 下次会话自动注入。版本 v10.1.0，1385 commits，非常活跃。

**最核心设计：3 层渐进式检索工作流**（极致节省 token）：

| 层级 | MCP 工具 | 返回内容 | Token 消耗 |
|------|---------|---------|-----------|
| L1 | `search` | 轻量索引（标题 + 时间 + ID 列表） | 极低 |
| L2 | `timeline` | 时间线上下文（摘要级别） | 中等 |
| L3 | `get_observations` | 完整观察内容 | 较高 |

AI 先用 L1 定位，确认需要后才拉 L2/L3，号称比直接返回全部内容节省约 **10 倍 token**。

**其他关键设计**：

- **观察压缩**：使用 Claude Agent SDK 将工具输出压缩为 ~500 token 语义摘要
- **Claim→Process→Delete**：防消息丢失的可靠处理模式（JillVernus fork 关键修复）
- **多 AI 提供者**：Claude / Gemini / OpenAI，统一 OpenAI 兼容 API 格式
- **会话滚动**：上下文超 150k token 时自动重启 SDK 会话
- **Token 成本可见性**：搜索结果附带 token 消耗估算

**可借鉴到三术的点**：

| 借鉴点 | claude-mem 做法 | 三术应用 |
|--------|---------------|---------|
| **3 层渐进检索** | search → timeline → get_observations | sou 分层返回：L1 文件路径列表（轻量）→ L2 代码摘要 → L3 完整代码片段 |
| **观察压缩** | AI 压缩为 ~500 token | enhance 时先压缩项目上下文再注入，避免 token 爆炸 |
| **OpenAI 兼容 API** | 统一接口支持多提供者 | enhance 的 LLM 调用统一用 OpenAI 格式，一套代码支持 Ollama/SiliconFlow/任意端点 |
| **Token 成本可见性** | 结果附带 token 估算 | sou 返回时附带 chunk 数量和预估 token，帮 AI 决定是否需要更多上下文 |
| **Claim→Process→Delete** | 防消息丢失 | 索引 blob 处理：标记→嵌入→确认写入，中断可恢复 |

**局限**：依赖 Claude Agent SDK（压缩需 LLM API 成本）；Node.js/Bun 生态；面向会话记忆而非代码语义搜索。

### 现有 ji（记忆工具）模式

- **存储**：单 JSON 文件（`memories.json`），`MemoryStore { entries: Vec<MemoryEntry> }`
- **相似度**：纯文本算法（Levenshtein 0.4 + Phrase 0.4 + Jaccard 0.2），零外部依赖
- **去重**：基于文本相似度阈值（默认 0.70）
- **优点**：极轻量、零依赖、即开即用
- **局限**：无语义理解，纯字符级匹配

### 综合借鉴策略

从 5 个项目 + ji 中提炼出的设计原则：

1. **免费 API 嵌入**（← ContextWeaver）：用 Jina/SiliconFlow 免费 API，不搞本地 ONNX
2. **混合检索 + RRF 融合**（← ContextWeaver）：向量 + BM25 双通道
3. **3 层渐进式返回**（← claude-mem）：先轻量索引，按需拉完整内容，节省 token
4. **URI 结构化前缀查询**（← nocturne_memory）：按文件路径/模块前缀快速缩小搜索范围
5. **JSON 轻量存储**（← ji）：元数据 JSON + 向量 bincode，零重依赖
6. **OpenAI 兼容 API 统一接口**（← claude-mem）：一套代码支持所有 LLM/Embedding 提供者
7. **索引快照回滚**（← nocturne_memory）：重建前保留旧索引
8. **Token 成本可见性**（← claude-mem）：返回结果附带 token 估算

---

## 二、统一 API 接口层设计（核心基础设施）

所有外部 API 调用统一为 OpenAI 兼容格式，一套代码适配所有提供者（← claude-mem 多提供者思路）。

### 2.1 Chat Completion 统一接口（用于 enhance）

```rust
/// 统一的 LLM Chat API 客户端
pub struct ChatClient {
    provider: ChatProvider,
    client: reqwest::Client,    // 复用现有 reqwest
    base_url: String,
    api_key: Option<String>,    // Ollama 不需要
    model: String,
}

pub enum ChatProvider {
    Ollama,         // http://localhost:11434/api/generate（自动转换格式）
    OpenAI,         // https://api.openai.com/v1/chat/completions
    Gemini,         // https://generativelanguage.googleapis.com/v1beta/（适配层转换）
    SiliconFlow,    // https://api.siliconflow.cn/v1/chat/completions（OpenAI 兼容）
    Groq,           // https://api.groq.com/openai/v1/chat/completions（OpenAI 兼容）
    Custom,         // 任意 OpenAI 兼容端点
}

impl ChatClient {
    /// 统一调用接口（内部自动适配不同提供者的请求/响应格式）
    pub async fn chat(&self, messages: &[Message]) -> Result<String>;

    /// 流式调用（SSE），回调推送 chunk
    pub async fn chat_stream<F>(&self, messages: &[Message], on_chunk: F) -> Result<String>
    where F: FnMut(&str) + Send;

    /// 检测服务是否可用
    pub async fn is_available(&self) -> bool;
}

/// 统一消息格式（OpenAI 标准）
pub struct Message {
    pub role: String,       // "system" | "user" | "assistant"
    pub content: String,
}
```

**各提供者适配逻辑**：

| 提供者 | 请求端点 | 请求格式 | 响应格式 | 适配方式 |
|--------|---------|---------|---------|---------|
| OpenAI / SiliconFlow / Groq / Custom | `POST /v1/chat/completions` | OpenAI 标准 | OpenAI 标准 | 直接透传 |
| Ollama | `POST /api/chat` | Ollama 格式 | Ollama 格式 | 请求/响应双向转换 |
| Gemini | `POST /v1beta/models/{model}:generateContent` | Gemini 格式 | Gemini 格式 | 请求/响应双向转换 |

### 2.2 Embedding 统一接口（用于 sou 备选模式）

```rust
/// 统一的 Embedding API 客户端
pub struct EmbeddingClient {
    provider: EmbeddingProvider,
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    dimension: usize,
}

pub enum EmbeddingProvider {
    Jina,           // https://api.jina.ai/v1/embeddings（OpenAI 兼容）
    SiliconFlow,    // https://api.siliconflow.cn/v1/embeddings（OpenAI 兼容）
    Voyage,         // https://api.voyageai.com/v1/embeddings（OpenAI 兼容）
    OpenAI,         // https://api.openai.com/v1/embeddings
    Ollama,         // http://localhost:11434/api/embed（适配层转换）
    Custom,         // 任意 OpenAI 兼容端点
}

impl EmbeddingClient {
    /// 批量嵌入（自动分批 + 限速）
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    /// 单条嵌入
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
}
```

**统一请求格式**（OpenAI 标准，Jina/SiliconFlow/Voyage 均兼容）：
```json
POST /v1/embeddings
{
  "model": "jina-embeddings-v3",
  "input": ["文本1", "文本2"],
  "dimensions": 512
}
```

### 2.3 统一接口模块结构

```
src/rust/mcp/tools/
├── api/                    — 【新增】统一 API 接口层
│   ├── mod.rs              — 模块入口
│   ├── chat.rs             — ChatClient（LLM 调用，enhance 用）
│   ├── embedding.rs        — EmbeddingClient（向量生成，sou 备选用）
│   ├── adapters/           — 各提供者适配器
│   │   ├── openai.rs       — OpenAI 兼容（直接透传）
│   │   ├── ollama.rs       — Ollama 格式转换
│   │   └── gemini.rs       — Gemini 格式转换
│   └── types.rs            — Message, EmbeddingRequest, 统一错误类型
```

这个 `api/` 模块是 enhance 和 sou 共享的基础设施，一次实现，两处复用。

---

## 三、enhance 替换方案（优先实施）

### 当前依赖

`enhance/core.rs:490-596` 调用 `POST {base_url}/chat-stream`（Augment API，SSE 流式）

### 替换为

三级降级链，全部通过统一 `ChatClient` 调用：

```
prompt + 代码上下文 + 对话历史 [构建方式不变]
  → 检测可用后端：
    L1: Ollama (localhost:11434) → ChatClient(Ollama)
    L2: 用户配置的 OpenAI 兼容 API → ChatClient(Custom/SiliconFlow/Groq)
    L3: 规则增强引擎 → 零 API 调用，纯本地
  → 复用现有 ENHANCE_SYSTEM_PROMPT 模板
  → 复用现有 <augment-enhanced-prompt> 标签提取
  → EnhanceResponse [接口完全不变]
```

### 修改文件

| 文件 | 变更 |
|------|------|
| `enhance/core.rs` | `PromptEnhancer` 新增本地模式分支，`enhance()` / `enhance_stream()` 路由到 ChatClient 或 RuleEnhancer |
| `enhance/local_enhance.rs` | 【新增】`LocalEnhancer`（ChatClient 封装）+ `RuleEnhancer`（规则引擎） |
| `api/chat.rs` | 【新增】统一 ChatClient |
| `api/adapters/*.rs` | 【新增】Ollama / Gemini 适配器 |
| `config/settings.rs` | McpConfig 新增 enhance 相关配置字段 |

### 保留不变

- `EnhanceRequest` / `EnhanceResponse` / `EnhanceStreamEvent` 类型定义
- `ChatHistoryManager` 对话历史管理
- `build_request_payload()` 的上下文构建逻辑（blob 加载、zhi 历史、对话历史）
- `extract_enhanced_prompt()` 标签提取
- 前端 EnhanceConfig / EnhanceModal 组件（仅新增配置项）

---

## 四、sou 索引双模式（后续实施）

### 当前模式（保留）

ace API 可用时继续使用：
- `POST /batch-upload` → 远程索引
- `POST /agents/codebase-retrieval` → 远程搜索

### 备选模式（新增）

ace API 不可用时切换到免费 Embedding API：
- 通过统一 `EmbeddingClient` 调用 Jina / SiliconFlow / Voyage / Ollama
- 本地向量存储（JSON + bincode）
- BM25 + 余弦相似度混合检索

### 模式切换

```rust
// mcp.rs 中的路由逻辑
if ace_api_available() {
    // 现有逻辑不变
    update_index_remote();  // batch-upload
    search_only_remote();   // codebase-retrieval
} else if embedding_api_configured() {
    // 新增：免费 API 模式
    update_index_local();   // EmbeddingClient + 本地存储
    search_only_local();    // 本地混合检索
} else {
    // 兜底：纯 BM25 关键词搜索（零 API）
    search_bm25_only();
}
```

---

## 五、配置系统扩展

`McpConfig` 新增字段（全部 `Option`，向后兼容）：

```rust
// === enhance 用的 Chat API ===
pub enhance_provider: Option<String>,               // "ollama" / "openai" / "gemini" / "siliconflow" / "groq" / "custom"
pub enhance_base_url: Option<String>,               // 自定义端点（默认按 provider 自动填充）
pub enhance_api_key: Option<String>,                // API Key（Ollama 不需要）
pub enhance_model: Option<String>,                  // 模型名（默认按 provider 推荐）

// === sou 备选模式用的 Embedding API ===
pub embedding_provider: Option<String>,             // "jina" / "siliconflow" / "voyage" / "ollama" / "custom"
pub embedding_base_url: Option<String>,
pub embedding_api_key: Option<String>,
pub embedding_model: Option<String>,
pub embedding_dimension: Option<usize>,             // 默认 512

// === sou 模式切换 ===
pub sou_mode: Option<String>,                       // "ace"（默认）/ "embedding" / "auto"

// === 搜索参数 ===
pub search_vector_weight: Option<f32>,              // 默认 0.6
pub search_bm25_weight: Option<f32>,                // 默认 0.4
pub search_top_k: Option<usize>,                    // 默认 10
```

**各提供者默认值**：

| provider | 默认 base_url | 默认 model | 需要 api_key |
|----------|--------------|-----------|-------------|
| ollama | `http://localhost:11434` | `qwen2.5:7b` | 否 |
| openai | `https://api.openai.com/v1` | `gpt-4o-mini` | 是 |
| gemini | `https://generativelanguage.googleapis.com/v1beta` | `gemini-2.0-flash` | 是 |
| siliconflow | `https://api.siliconflow.cn/v1` | `Qwen/Qwen2.5-7B-Instruct` | 是（免费注册） |
| groq | `https://api.groq.com/openai/v1` | `llama-3.3-70b-versatile` | 是（免费注册） |
| jina | `https://api.jina.ai/v1` | `jina-embeddings-v3` | 是（免费注册） |

---

## 六、Cargo.toml 变更

```toml
# 无需新增重依赖！复用现有：reqwest / serde / serde_json / ring / tokio
# 仅新增（sou 备选模式用）：
bincode = "1.3"    # 向量数据紧凑序列化（~50KB 编译产物）
```

---

## 七、实施阶段

### 阶段 1（优先）：统一 API 接口层 + enhance 替换
- `api/chat.rs` — ChatClient 统一接口
- `api/adapters/ollama.rs` / `gemini.rs` — 格式适配器
- `enhance/local_enhance.rs` — LocalEnhancer + RuleEnhancer
- 修改 `enhance/core.rs` — 本地模式分支
- 修改 `config/settings.rs` — enhance 配置字段
- 验证：enhance MCP 工具端到端可用（Ollama / OpenAI 兼容 / 规则引擎）

### 阶段 2：sou 备选模式（Embedding API + 本地搜索）
- `api/embedding.rs` — EmbeddingClient 统一接口
- `acemcp/local/store.rs` — 本地向量存储（JSON + bincode）
- `acemcp/local/bm25.rs` — BM25 关键词索引
- `acemcp/local/search.rs` — 混合检索 + RRF 融合
- 修改 `mcp.rs` — ace/embedding/auto 三模式路由
- 验证：sou 在 embedding 模式下端到端可用

### 阶段 3：前端适配
- 设置页新增 enhance 提供者配置区
- 设置页新增 sou 模式切换 + embedding 提供者配置
- API Key 输入、连接测试、模型选择

---

## 八、风险与缓解

| 风险 | 概率 | 缓解 |
|------|------|------|
| Ollama 未安装 | 中 | 自动降级到 OpenAI 兼容 API 或规则引擎 |
| 免费 LLM API 限速 | 中 | 支持多提供者切换；规则引擎零 API 兜底 |
| Gemini API 格式差异 | 低 | 适配器层隔离，不影响其他提供者 |
| Jina 免费额度用完 | 低 | 支持切换 SiliconFlow/Voyage/Ollama 本地嵌入 |
| 规则增强质量有限 | 预期内 | 作为 L3 兜底，推荐用户配置 Ollama 或免费 API |

---

## 九、方案优势总结

1. **零成本**：Ollama 本地免费 + SiliconFlow/Groq 免费注册 + 规则引擎零依赖
2. **极轻量**：enhance 替换零新增依赖；sou 备选仅需 `bincode`（~50KB）
3. **统一接口**：一套 ChatClient/EmbeddingClient 适配 OpenAI/Ollama/Gemini/任意端点
4. **双模式 sou**：ace 可用时继续用，不可用时无缝切换免费 Embedding API
5. **向后兼容**：现有配置和 MCP 工具接口完全不变
6. **渐进式**：阶段 1 只改 enhance（最小改动），阶段 2 再扩展 sou
