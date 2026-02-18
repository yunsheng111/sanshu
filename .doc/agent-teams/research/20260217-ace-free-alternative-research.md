# 三术（sanshu）免费平替 Augment Code API — 可行性方案 v4

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

---

## 十、v4 补充：技术实现细节深度分析

> **v4 更新说明**（2026-02-18）：基于代码库实际结构（`enhance/core.rs`、`acemcp/mcp.rs`、`config/settings.rs`）补充 v3 未覆盖的技术细节。

### 10.1 AST 分片 vs 行级分片对比

**现有实现**（`acemcp/mcp.rs:collect_blobs()`）：
- 按 `max_lines_per_blob`（默认 800 行）分割大文件
- 分片命名：`file.rs#chunk1`、`file.rs#chunk2`（已有 `strip_chunk_suffix()` 处理）
- 优点：实现简单，零依赖
- 缺点：可能在函数/类中间切断，破坏语义完整性

**Tree-sitter AST 分片**（← ContextWeaver 方案）：
- 按语法单元分割（函数、类、模块）
- 分片命名：`file.rs::function_name`、`file.rs::ClassName`
- 优点：保持语义完整性，搜索精度更高
- 缺点：需引入 `tree-sitter` crate（~2MB 编译产物）+ 各语言 parser

**推荐策略**：
```rust
// 阶段 1：保持现有行级分片（零改动）
// 阶段 2（可选）：新增 AST 分片模式（配置开关）
pub enum ChunkStrategy {
    LinesBased { max_lines: usize },        // 现有模式
    AstBased { max_tokens: usize },         // 新增：按 AST 节点分割
    Hybrid { max_lines: usize, prefer_ast: bool }, // 混合：优先 AST，超长回退行级
}
```

**配置扩展**（`McpConfig`）：
```rust
pub chunk_strategy: Option<String>,  // "lines" | "ast" | "hybrid"，默认 "lines"
pub ast_languages: Option<Vec<String>>, // ["rust", "typescript", "python"]
```

### 10.2 RRF 融合算法 Rust 实现

**Reciprocal Rank Fusion 公式**：
```
RRF_score(doc) = Σ [ 1 / (k + rank_i(doc)) ]
```
- `k`：常数（通常 60），防止除零
- `rank_i(doc)`：文档在第 i 个检索器中的排名（从 1 开始）

**Rust 实现骨架**：
```rust
use std::collections::HashMap;

/// RRF 融合多个检索结果
pub fn reciprocal_rank_fusion(
    results: Vec<Vec<SearchResult>>, // 多个检索器的结果列表
    k: f32,                           // RRF 常数，默认 60.0
) -> Vec<SearchResult> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for result_list in results {
        for (rank, result) in result_list.iter().enumerate() {
            let rrf_score = 1.0 / (k + (rank + 1) as f32);
            *scores.entry(result.id.clone()).or_insert(0.0) += rrf_score;
        }
    }

    // 按 RRF 分数降序排序
    let mut fused: Vec<_> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // 重建 SearchResult（需从原始结果中查找）
    fused.into_iter()
        .map(|(id, score)| SearchResult { id, score, ..Default::default() })
        .collect()
}

/// 混合检索入口
pub async fn hybrid_search(
    query: &str,
    vector_results: Vec<SearchResult>,  // 向量检索结果
    bm25_results: Vec<SearchResult>,    // BM25 关键词结果
) -> Vec<SearchResult> {
    reciprocal_rank_fusion(vec![vector_results, bm25_results], 60.0)
}
```

**集成位置**：`acemcp/local/search.rs`（新增文件）

### 10.3 RuleEnhancer 规则设计

**基于现有 `ENHANCE_SYSTEM_PROMPT` 模板**（`enhance/core.rs:490`）：

```rust
pub struct RuleEnhancer {
    rules: Vec<EnhanceRule>,
}

pub struct EnhanceRule {
    pub trigger: Regex,           // 触发条件（正则匹配）
    pub template: String,         // 增强模板
    pub priority: u8,             // 优先级（高优先级规则先匹配）
}

impl RuleEnhancer {
    pub fn enhance(&self, prompt: &str, context: &EnhanceContext) -> String {
        let mut enhanced = prompt.to_string();

        // 按优先级排序规则
        let mut rules = self.rules.clone();
        rules.sort_by_key(|r| std::cmp::Reverse(r.priority));

        for rule in rules {
            if rule.trigger.is_match(&enhanced) {
                enhanced = rule.template
                    .replace("{prompt}", &enhanced)
                    .replace("{project_root}", &context.project_root)
                    .replace("{tech_stack}", &context.tech_stack.join(", "));
            }
        }

        // 包装为 <augment-enhanced-prompt> 标签（兼容现有提取逻辑）
        format!("<augment-enhanced-prompt>\n{}\n</augment-enhanced-prompt>", enhanced)
    }
}
```

**内置规则示例**：
```rust
vec![
    EnhanceRule {
        trigger: Regex::new(r"(?i)(bug|error|fix)").unwrap(),
        template: "【缺陷修复增强】\n原始需求：{prompt}\n\n请补充：\n1. 错误现象描述\n2. 预期行为\n3. 复现步骤\n4. 相关日志或错误信息",
        priority: 90,
    },
    EnhanceRule {
        trigger: Regex::new(r"(?i)(add|new|create|implement)").unwrap(),
        template: "【新功能增强】\n原始需求：{prompt}\n\n请明确：\n1. 功能目标和使用场景\n2. 输入输出规格\n3. 边界条件和异常处理\n4. 性能和安全要求",
        priority: 80,
    },
    EnhanceRule {
        trigger: Regex::new(r"(?i)(refactor|optimize|improve)").unwrap(),
        template: "【重构优化增强】\n原始需求：{prompt}\n\n请说明：\n1. 当前问题或瓶颈\n2. 优化目标（性能/可读性/可维护性）\n3. 约束条件（向后兼容/API 稳定性）",
        priority: 70,
    },
]
```

**配置扩展**（`McpConfig`）：
```rust
pub enhance_rule_engine_enabled: Option<bool>, // 默认 true（作为 L3 兜底）
pub enhance_custom_rules: Option<Vec<CustomRule>>, // 用户自定义规则
```

### 10.4 本地向量存储格式设计

**结合现有 `projects.json` 命名规范**（`acemcp/mcp.rs`）：

```
.sanshu-index/
├── projects.json              — 项目元数据（现有格式保持不变）
└── <project_id>/
    ├── metadata.json          — 索引元数据（文件数、chunk 数、更新时间）
    ├── chunks.json            — chunk 列表（路径、行号、BM25 权重）
    ├── vectors.bin            — 向量数据（bincode 序列化）
    └── bm25_index.json        — BM25 倒排索引
```

**数据结构**：
```rust
/// 索引元数据
#[derive(Serialize, Deserialize)]
pub struct IndexMetadata {
    pub project_id: String,
    pub project_root: String,
    pub total_files: usize,
    pub total_chunks: usize,
    pub embedding_model: String,      // "jina-embeddings-v3"
    pub embedding_dimension: usize,   // 512
    pub last_updated: i64,            // Unix timestamp
    pub version: String,              // "1.0"
}

/// Chunk 元数据
#[derive(Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub id: String,                   // "file.rs#chunk1" 或 "file.rs::function_name"
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content_hash: String,         // SHA256（用于增量更新）
    pub bm25_weight: f32,             // BM25 预计算权重
}

/// 向量存储（bincode 序列化）
#[derive(Serialize, Deserialize)]
pub struct VectorStore {
    pub vectors: Vec<Vec<f32>>,       // 按 chunk_id 顺序存储
    pub dimension: usize,
}
```

**bincode 序列化示例**：
```rust
use bincode;

// 写入
let vectors = VectorStore { vectors: vec![...], dimension: 512 };
let encoded = bincode::serialize(&vectors)?;
fs::write("vectors.bin", encoded)?;

// 读取
let encoded = fs::read("vectors.bin")?;
let vectors: VectorStore = bincode::deserialize(&encoded)?;
```

**存储大小估算**：
- 1000 个 chunk × 512 维 × 4 字节（f32）= ~2MB
- bincode 压缩后约 1.5MB（比 JSON 节省 60%）

### 10.5 新增免费 Embedding 提供者

**v3 已覆盖**：Jina、SiliconFlow、Voyage、Ollama

**v4 新增 3 个提供者**：

#### 1. Cloudflare AI Workers

**优势**：
- 免费额度：10,000 次/天（Workers 免费计划）
- 模型：`@cf/baai/bge-base-en-v1.5`（768 维）
- 延迟：全球 CDN，平均 < 100ms

**API 格式**（OpenAI 兼容）：
```bash
POST https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/baai/bge-base-en-v1.5
Authorization: Bearer {api_token}

{
  "text": ["文本1", "文本2"]
}
```

**适配器实现**：
```rust
// api/adapters/cloudflare.rs
pub async fn embed_cloudflare(
    client: &reqwest::Client,
    account_id: &str,
    api_token: &str,
    texts: &[String],
) -> Result<Vec<Vec<f32>>> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/@cf/baai/bge-base-en-v1.5",
        account_id
    );

    let response = client.post(&url)
        .bearer_auth(api_token)
        .json(&serde_json::json!({ "text": texts }))
        .send()
        .await?;

    let result: CloudflareResponse = response.json().await?;
    Ok(result.result.data)
}
```

#### 2. Nomic Embed

**优势**：
- 免费额度：无限制（开源模型 + 免费 API）
- 模型：`nomic-embed-text-v1.5`（768 维）
- 特点：支持长文本（8192 tokens）

**API 格式**（OpenAI 兼容）：
```bash
POST https://api-atlas.nomic.ai/v1/embedding/text
Authorization: Bearer {api_key}

{
  "model": "nomic-embed-text-v1.5",
  "texts": ["文本1", "文本2"]
}
```

**适配器**：直接复用 OpenAI 适配器（格式完全兼容）

#### 3. Cohere Embed v3

**优势**：
- 免费额度：1,000 次/月（Trial API Key）
- 模型：`embed-english-v3.0`（1024 维）
- 特点：支持多语言 + 压缩嵌入（384/512/1024 维可选）

**API 格式**：
```bash
POST https://api.cohere.ai/v1/embed
Authorization: Bearer {api_key}

{
  "model": "embed-english-v3.0",
  "texts": ["文本1", "文本2"],
  "input_type": "search_document",
  "embedding_types": ["float"]
}
```

**适配器实现**：
```rust
// api/adapters/cohere.rs
pub async fn embed_cohere(
    client: &reqwest::Client,
    api_key: &str,
    texts: &[String],
    dimension: usize, // 384 | 512 | 1024
) -> Result<Vec<Vec<f32>>> {
    let response = client.post("https://api.cohere.ai/v1/embed")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": "embed-english-v3.0",
            "texts": texts,
            "input_type": "search_document",
            "embedding_types": ["float"],
            "truncate": "END"
        }))
        .send()
        .await?;

    let result: CohereResponse = response.json().await?;
    Ok(result.embeddings.float)
}
```

**配置扩展**（`McpConfig`）：
```rust
// 新增提供者枚举值
pub embedding_provider: Option<String>,
// "jina" | "siliconflow" | "voyage" | "ollama" | "cloudflare" | "nomic" | "cohere" | "custom"

// Cloudflare 专用配置
pub cloudflare_account_id: Option<String>,

// Cohere 压缩维度配置
pub cohere_dimension: Option<usize>, // 384 | 512 | 1024，默认 1024
```

**提供者对比表**：

| 提供者 | 免费额度 | 模型 | 维度 | 延迟 | 推荐场景 |
|--------|---------|------|------|------|---------|
| Jina | 1M tokens/月 | jina-embeddings-v3 | 512/1024 | ~200ms | 通用首选 |
| SiliconFlow | 无限制 | bge-m3 | 1024 | ~300ms | 国内用户 |
| Cloudflare | 10k 次/天 | bge-base-en-v1.5 | 768 | ~100ms | 低延迟需求 |
| Nomic | 无限制 | nomic-embed-text-v1.5 | 768 | ~250ms | 长文本（8k tokens） |
| Cohere | 1k 次/月 | embed-english-v3.0 | 384/512/1024 | ~150ms | 多语言 + 压缩嵌入 |
| Voyage | 无限制 | voyage-code-2 | 1536 | ~200ms | 代码专用 |
| Ollama | 无限制 | nomic-embed-text | 768 | ~50ms | 本地离线 |

---

## 十一、v4 实施路线图更新

### 阶段 1：统一 API 接口层 + enhance 替换（优先）

**新增文件**：
```
src/rust/mcp/tools/
├── api/                          — 【新增】统一 API 接口层
│   ├── mod.rs
│   ├── chat.rs                   — ChatClient（LLM 调用）
│   ├── embedding.rs              — EmbeddingClient（向量生成）
│   ├── adapters/
│   │   ├── openai.rs             — OpenAI 兼容（直接透传）
│   │   ├── ollama.rs             — Ollama 格式转换
│   │   ├── gemini.rs             — Gemini 格式转换
│   │   ├── cloudflare.rs         — 【v4 新增】Cloudflare AI Workers
│   │   └── cohere.rs             — 【v4 新增】Cohere Embed v3
│   └── types.rs                  — 统一类型定义
```

**修改文件**：
- `enhance/core.rs` — 新增本地模式分支
- `enhance/local_enhance.rs` — 【新增】LocalEnhancer + RuleEnhancer
- `config/settings.rs` — 扩展 `McpConfig` 字段（见 10.5 节）

### 阶段 2：sou 备选模式（Embedding API + 本地搜索）

**新增文件**：
```
src/rust/mcp/tools/acemcp/
├── local/                        — 【新增】本地索引模块
│   ├── mod.rs
│   ├── store.rs                  — 向量存储（JSON + bincode）
│   ├── bm25.rs                   — BM25 关键词索引
│   ├── search.rs                 — 混合检索 + RRF 融合
│   └── ast_chunker.rs            — 【可选】AST 分片器
```

**修改文件**：
- `acemcp/mcp.rs` — 三模式路由（ace/embedding/auto）
- `mcp.rs` — 工具注册时检测模式

### 阶段 3：前端适配

**新增组件**：
- `EnhanceProviderConfig.vue` — enhance 提供者配置
- `SouModeSwitch.vue` — sou 模式切换
- `EmbeddingProviderConfig.vue` — embedding 提供者配置

**修改组件**：
- `Settings.vue` — 新增配置区块

---

## 十二、v4 风险与缓解（更新）

| 风险 | 概率 | v4 缓解措施 |
|------|------|------------|
| Ollama 未安装 | 中 | 自动降级到 Cloudflare（10k/天免费）或 RuleEnhancer |
| 免费 LLM API 限速 | 中 | 支持 7 个提供者切换；RuleEnhancer 零 API 兜底 |
| Cloudflare Workers 配额用完 | 低 | 降级到 Nomic（无限制）或 Ollama 本地 |
| Cohere 免费额度低（1k/月） | 预期内 | 仅作为备选，推荐 Jina/SiliconFlow/Cloudflare |
| AST 分片增加编译产物 | 低 | 作为可选功能，默认关闭；用户按需启用 |
| RRF 融合性能开销 | 极低 | 算法复杂度 O(n log n)，1000 结果 < 1ms |

---

## 十三、v4 方案优势总结（更新）

1. **零成本**：7 个免费提供者（Ollama/Jina/SiliconFlow/Cloudflare/Nomic/Voyage/Cohere）+ RuleEnhancer 零依赖
2. **极轻量**：enhance 替换零新增依赖；sou 备选仅需 `bincode`（~50KB）
3. **统一接口**：一套 ChatClient/EmbeddingClient 适配 OpenAI/Ollama/Gemini/Cloudflare/Cohere/任意端点
4. **双模式 sou**：ace 可用时继续用，不可用时无缝切换免费 Embedding API
5. **向后兼容**：现有配置和 MCP 工具接口完全不变
6. **渐进式**：阶段 1 只改 enhance（最小改动），阶段 2 再扩展 sou
7. **高可用**：7 个提供者互为备份，单点故障自动降级
8. **灵活分片**：支持行级/AST/混合三种分片策略，按需选择
9. **智能融合**：RRF 算法融合向量 + BM25，召回率提升 15-30%（← ContextWeaver 实测数据）
10. **规则兜底**：RuleEnhancer 零 API 成本，作为 L3 兜底确保服务可用性

---

## 十四、v5 安全与健壮性补充

> **v5 更新说明**（2026-02-18）：基于代码审查报告补充 v4 遗漏的安全性、健壮性和一致性设计细节。

### 14.1 [C1 修复] ChatClient 超时配置设计

**问题**：v4 的 `ChatClient` 缺少超时字段，与现有 `src/rust/constants/network.rs` 已定义的常量脱节。

**修复后的结构体**：

```rust
use crate::constants::network::{
    CONNECTION_TIMEOUT_MS, READ_TIMEOUT_MS, DEFAULT_TIMEOUT_MS,
};

pub struct ChatClient {
    provider: ChatProvider,
    base_url: String,
    api_key: Option<String>,
    model: String,
    // 三段超时（复用现有常量）
    connect_timeout_ms: u64,   // 默认 CONNECTION_TIMEOUT_MS (10_000)
    request_timeout_ms: u64,   // 默认 DEFAULT_TIMEOUT_MS (30_000)
    stream_timeout_ms: u64,    // 流式响应专用，默认 120_000
    // reqwest::Client 按超时配置懒构建，不在结构体中持有
}

impl ChatClient {
    pub fn new(provider: ChatProvider, base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            provider,
            base_url,
            api_key,
            model,
            connect_timeout_ms: CONNECTION_TIMEOUT_MS,
            request_timeout_ms: DEFAULT_TIMEOUT_MS,
            stream_timeout_ms: 120_000,
        }
    }

    /// 构建 reqwest::Client（按调用类型选择超时）
    fn build_client(&self, is_stream: bool) -> anyhow::Result<reqwest::Client> {
        let timeout_ms = if is_stream { self.stream_timeout_ms } else { self.request_timeout_ms };
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(self.connect_timeout_ms))
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| anyhow::anyhow!("构建 HTTP 客户端失败: {}", e))
    }

    pub async fn chat(&self, messages: &[Message]) -> anyhow::Result<String> {
        let client = self.build_client(false)?;
        // ... 实际调用逻辑
        todo!()
    }

    pub async fn chat_stream<F>(&self, messages: &[Message], on_chunk: F) -> anyhow::Result<String>
    where F: FnMut(&str) + Send {
        let client = self.build_client(true)?;
        // ... 流式调用逻辑
        todo!()
    }
}
```

**各提供者推荐超时**：

| 提供者 | connect_timeout | request_timeout | stream_timeout |
|--------|----------------|----------------|----------------|
| Ollama（本地） | 3_000 ms | 60_000 ms | 300_000 ms |
| OpenAI / Groq | 10_000 ms | 30_000 ms | 120_000 ms |
| SiliconFlow | 10_000 ms | 30_000 ms | 120_000 ms |
| Gemini | 10_000 ms | 30_000 ms | 120_000 ms |

---

### 14.2 [C2 修复] API Key 安全存储方案

**问题**：`enhance_api_key`、`embedding_api_key` 明文存储在 `settings.json`，存在泄露风险。

**三层防护方案**：

#### 层 1：日志脱敏（必须实施）

```rust
/// API Key 脱敏显示（用于日志输出）
pub fn mask_api_key(key: &str) -> String {
    match key.len() {
        0 => "(空)".to_string(),
        1..=8 => "****".to_string(),
        _ => format!("{}****{}", &key[..4], &key[key.len()-4..]),
    }
}

// 使用示例（替换现有 log::info! 中的 key 输出）
log::info!("enhance API Key 已配置: {}", mask_api_key(&api_key));
```

#### 层 2：配置文件权限控制（建议实施）

```rust
// storage.rs 中写入配置后设置文件权限
#[cfg(unix)]
fn set_config_file_permissions(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

// Windows 下配置文件已在用户目录（%APPDATA%），权限由系统控制
```

#### 层 3：.gitignore 保护（必须实施）

在项目根目录 `.gitignore` 中确认包含：
```
# 用户配置（含 API Key）
*.json
!package.json
!tsconfig*.json
# 或更精确地排除配置目录
%APPDATA%/sanshu/
~/.config/sanshu/
```

**注意**：Tauri 应用的配置文件存储在系统配置目录（`%APPDATA%\sanshu\settings.json`），不在项目目录内，因此不会被 git 追踪。但需在文档中明确说明，避免用户手动复制配置文件到项目目录。

---

### 14.3 [W1 修复] 重试机制设计

**问题**：`ChatClient` 和 `EmbeddingClient` 未设计重试，而现有 `acemcp/mcp.rs` 已有成熟的 `retry_request()` 泛型函数。

**复用现有重试函数**：

```rust
// 直接复用 acemcp/mcp.rs 中的 retry_request()
// 无需重新实现，仅需在 ChatClient 调用时包装

impl ChatClient {
    pub async fn chat_with_retry(&self, messages: &[Message]) -> anyhow::Result<String> {
        // 复用现有泛型重试函数（max_retries=3, base_delay=1.0s 指数退避）
        retry_request(
            || async { self.chat(messages).await },
            3,
            1.0,
        ).await
    }
}
```

**不可重试错误的处理**（扩展现有 `is_retryable` 判断）：

```rust
// 在 retry_request 的错误判断中补充 HTTP 状态码检查
let is_retryable = error_str.contains("timeout")
    || error_str.contains("connection")
    || error_str.contains("network")
    || error_str.contains("temporary")
    || error_str.contains("503")   // 服务暂时不可用
    || error_str.contains("502");  // 网关错误

// 明确不重试的错误
let is_fatal = error_str.contains("401")   // 认证失败
    || error_str.contains("403")           // 权限不足
    || error_str.contains("400");          // 请求格式错误
```

---

### 14.4 [W2 修复] EmbeddingClient 批量限速设计

**问题**：`embed_batch()` 缺少具体的分批大小和限速实现。

**各提供者批量限制**：

| 提供者 | 最大批量条数 | 推荐批量 | 限速（RPM） |
|--------|------------|---------|------------|
| Jina | 2048 | 100 | 60 |
| SiliconFlow | 无明确限制 | 50 | 60 |
| Cloudflare | 100 | 50 | 无明确限制 |
| Nomic | 无明确限制 | 100 | 无明确限制 |
| Cohere | 96 | 50 | 10（Trial） |
| Ollama | 无限制 | 20 | 无限制 |

**实现骨架**：

```rust
pub struct EmbeddingClient {
    provider: EmbeddingProvider,
    base_url: String,
    api_key: String,
    model: String,
    dimension: usize,
    // 批量控制
    batch_size: usize,              // 每批最大条数（按提供者默认）
    rate_limit_rpm: Option<u32>,    // 每分钟请求数限制（None = 不限速）
}

impl EmbeddingClient {
    pub async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        let mut all_results: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        let chunks: Vec<&[String]> = texts.chunks(self.batch_size).collect();
        let total = chunks.len();

        for (i, chunk) in chunks.iter().enumerate() {
            // 限速：非最后一批时等待
            if let Some(rpm) = self.rate_limit_rpm {
                if i > 0 {
                    let delay_ms = 60_000u64 / rpm as u64;
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }

            let batch_result = retry_request(
                || async { self.embed_single_batch(chunk).await },
                3,
                1.0,
            ).await?;

            all_results.extend(batch_result);
            log_debug!("嵌入进度: {}/{}", i + 1, total);
        }

        Ok(all_results)
    }

    async fn embed_single_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        // 实际 API 调用
        todo!()
    }
}
```

**配置扩展**（`McpConfig` 新增）：

```rust
pub sou_embedding_batch_size: Option<usize>,    // 默认按提供者自动选择
pub sou_embedding_rate_limit_rpm: Option<u32>,  // 默认按提供者自动选择
```

---

### 14.5 [W3 修复] HTTPS 证书验证配置

**问题**：企业环境可能需要自定义 CA 证书，开发环境可能需要跳过验证。

**配置扩展**（`McpConfig` 新增）：

```rust
pub enhance_danger_accept_invalid_certs: Option<bool>,  // 默认 false，仅开发环境使用
pub enhance_custom_ca_cert_path: Option<String>,        // 自定义 CA 证书路径（PEM 格式）
```

**在 `build_client()` 中应用**：

```rust
fn build_client(&self, is_stream: bool) -> anyhow::Result<reqwest::Client> {
    let timeout_ms = if is_stream { self.stream_timeout_ms } else { self.request_timeout_ms };
    let mut builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_millis(self.connect_timeout_ms))
        .timeout(std::time::Duration::from_millis(timeout_ms));

    // 自定义 CA 证书（企业内网场景）
    if let Some(ca_path) = &self.custom_ca_cert_path {
        let ca_cert = std::fs::read(ca_path)
            .map_err(|e| anyhow::anyhow!("读取 CA 证书失败: {}", e))?;
        let cert = reqwest::Certificate::from_pem(&ca_cert)
            .map_err(|e| anyhow::anyhow!("解析 CA 证书失败: {}", e))?;
        builder = builder.add_root_certificate(cert);
    }

    // 跳过证书验证（仅开发环境，生产环境禁止）
    if self.danger_accept_invalid_certs {
        log::warn!("⚠️ 已禁用 TLS 证书验证，仅限开发环境使用");
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder.build().map_err(|e| anyhow::anyhow!("构建 HTTP 客户端失败: {}", e))
}
```

---

### 14.6 [W4 修复] RuleEnhancer 规则冲突处理

**问题**：多规则同时匹配时行为未定义（如 "fix bug" 同时匹配 `bug` 和 `fix`）。

**修复策略：首次匹配（First Match）**

```rust
impl RuleEnhancer {
    pub fn enhance(&self, prompt: &str, context: &EnhanceContext) -> String {
        // 按优先级排序（高优先级在前）
        let mut sorted_rules = self.rules.clone();
        sorted_rules.sort_by_key(|r| std::cmp::Reverse(r.priority));

        // 首次匹配策略：找到第一个匹配的规则后立即应用并停止
        let enhanced = sorted_rules.iter()
            .find(|rule| rule.trigger.is_match(prompt))
            .map(|rule| rule.template
                .replace("{prompt}", prompt)
                .replace("{project_root}", &context.project_root)
                .replace("{tech_stack}", &context.tech_stack.join(", ")))
            .unwrap_or_else(|| prompt.to_string());  // 无规则匹配时返回原始 prompt

        format!("<augment-enhanced-prompt>\n{}\n</augment-enhanced-prompt>", enhanced)
    }
}
```

**配置扩展**（支持用户选择匹配策略）：

```rust
pub struct RuleEnhancer {
    rules: Vec<EnhanceRule>,
    match_strategy: RuleMatchStrategy,
}

pub enum RuleMatchStrategy {
    FirstMatch,   // 默认：首次匹配后停止
    AllMatch,     // 全部匹配：依次应用所有匹配规则（叠加增强）
}
```

---

### 14.7 [W5 修复] 本地向量存储并发安全设计

**问题**：多个 MCP 请求可能同时触发索引更新和搜索，`VectorStore` 文件读写无并发保护。

**使用 `tokio::sync::RwLock` 保护**：

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// 本地索引管理器（线程安全）
pub struct LocalIndexManager {
    /// 读写锁：允许多个并发读，独占写
    index: Arc<RwLock<LocalIndex>>,
    project_id: String,
    index_dir: std::path::PathBuf,
}

pub struct LocalIndex {
    pub metadata: IndexMetadata,
    pub chunks: Vec<ChunkMetadata>,
    pub vectors: VectorStore,
    pub bm25_index: Bm25Index,
}

impl LocalIndexManager {
    /// 搜索（并发安全，允许多个同时搜索）
    pub async fn search(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<SearchResult>> {
        let index = self.index.read().await;
        // 在读锁保护下执行搜索
        hybrid_search(query, &index.vectors, &index.bm25_index, top_k).await
    }

    /// 更新索引（独占写，阻塞其他读写）
    pub async fn update_index(&self, new_chunks: Vec<ChunkMetadata>, new_vectors: VectorStore) -> anyhow::Result<()> {
        let mut index = self.index.write().await;
        // 先写临时文件，再原子替换（防止写入中断导致索引损坏）
        self.write_index_atomic(&new_chunks, &new_vectors).await?;
        index.chunks = new_chunks;
        index.vectors = new_vectors;
        Ok(())
    }

    /// 原子写入：写临时文件 → 重命名替换（防止中断损坏）
    async fn write_index_atomic(&self, chunks: &[ChunkMetadata], vectors: &VectorStore) -> anyhow::Result<()> {
        let tmp_path = self.index_dir.join("vectors.bin.tmp");
        let final_path = self.index_dir.join("vectors.bin");

        let encoded = bincode::serialize(vectors)?;
        tokio::fs::write(&tmp_path, &encoded).await?;
        tokio::fs::rename(&tmp_path, &final_path).await?;  // 原子替换

        Ok(())
    }
}
```

**全局单例注册**（在 MCP 服务器启动时初始化）：

```rust
// mcp_server.rs 或 server.rs 中
lazy_static::lazy_static! {
    static ref LOCAL_INDEX_MANAGERS: tokio::sync::Mutex<HashMap<String, Arc<LocalIndexManager>>> =
        tokio::sync::Mutex::new(HashMap::new());
}
```

---

### 14.8 [W6 修复] 配置字段命名统一

**问题**：v4 新增字段命名风格不一致（`enhance_*` vs `sou_mode` vs `embedding_*`）。

**统一规范**：所有新增字段按功能模块前缀命名，与现有 `acemcp_*` 风格保持一致。

**修订后的 `McpConfig` 新增字段**：

```rust
// === enhance 模块配置 ===
pub enhance_provider: Option<String>,               // "ollama" / "openai" / "gemini" / "siliconflow" / "groq" / "custom"
pub enhance_base_url: Option<String>,               // 自定义端点
pub enhance_api_key: Option<String>,                // API Key（日志中脱敏显示）
pub enhance_model: Option<String>,                  // 模型名
pub enhance_danger_accept_invalid_certs: Option<bool>, // 默认 false
pub enhance_custom_ca_cert_path: Option<String>,    // 自定义 CA 证书路径
pub enhance_rule_engine_enabled: Option<bool>,      // 默认 true
pub enhance_custom_rules: Option<Vec<CustomRule>>,  // 用户自定义规则

// === sou 模块配置（统一 sou_ 前缀）===
pub sou_mode: Option<String>,                       // "ace"（默认）/ "embedding" / "auto"
pub sou_embedding_provider: Option<String>,         // "jina" / "siliconflow" / "voyage" / "ollama" / "cloudflare" / "nomic" / "cohere" / "custom"
pub sou_embedding_base_url: Option<String>,
pub sou_embedding_api_key: Option<String>,          // 日志中脱敏显示
pub sou_embedding_model: Option<String>,
pub sou_embedding_dimension: Option<usize>,         // 默认 512
pub sou_embedding_batch_size: Option<usize>,        // 默认按提供者自动选择
pub sou_embedding_rate_limit_rpm: Option<u32>,      // 默认按提供者自动选择
pub sou_search_vector_weight: Option<f32>,          // 默认 0.6
pub sou_search_bm25_weight: Option<f32>,            // 默认 0.4
pub sou_search_top_k: Option<usize>,                // 默认 10
pub sou_search_rrf_k: Option<f32>,                  // RRF 常数，默认 60.0
pub sou_chunk_strategy: Option<String>,             // "lines" / "ast" / "hybrid"，默认 "lines"
pub sou_ast_languages: Option<Vec<String>>,         // AST 分片支持的语言列表

// === Cloudflare 专用（归属 sou 模块）===
pub sou_cloudflare_account_id: Option<String>,
pub sou_cohere_dimension: Option<usize>,            // 384 / 512 / 1024，默认 1024
```

**命名规范总结**：

| 模块 | 前缀 | 示例 |
|------|------|------|
| acemcp（现有） | `acemcp_` | `acemcp_base_url` |
| enhance（新增） | `enhance_` | `enhance_provider` |
| sou 本地模式（新增） | `sou_` | `sou_mode`、`sou_embedding_provider` |

---

## 十五、v5 实施路线图更新

### 阶段 1 验收标准（补充）

- [ ] Ollama 本地模式：`enhance` 工具返回 `<augment-enhanced-prompt>` 包裹的增强结果
- [ ] OpenAI 兼容模式：配置 SiliconFlow API Key 后可用，日志中 Key 显示为 `sk-a****b123`
- [ ] 规则引擎模式：无 API 配置时自动降级，返回规则增强结果
- [ ] 降级链测试：Ollama 不可用 → 自动切换 L2 → L2 不可用 → 自动切换 L3
- [ ] 超时测试：模拟网络延迟，验证 connect_timeout 和 request_timeout 生效
- [ ] 重试测试：模拟 503 错误，验证指数退避重试（最多 3 次）
- [ ] 401/403 不重试：验证认证失败时立即返回错误，不触发重试

### 阶段 2 验收标准（补充）

- [ ] 并发安全测试：同时触发索引更新和搜索，验证无数据竞争
- [ ] 原子写入测试：模拟写入中断，验证索引文件不损坏
- [ ] 批量限速测试：Cohere Trial Key（10 RPM）下批量嵌入不触发 429
- [ ] 维度一致性测试：切换提供者后重建索引，验证维度变更被正确检测
