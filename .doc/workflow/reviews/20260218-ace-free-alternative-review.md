# 代码审查报告

## 变更概述
- 变更范围：1 个文档文件，830+ 行
- 变更意图：设计 Augment Code API 的免费平替方案，包括 enhance 替换和 sou 索引双模式

## 审查结果摘要

| 严重程度   | 数量 |
|-----------|------|
| Critical  | 2    |
| Warning   | 6    |
| Info      | 4    |

---

## Critical（必须修复）

### [C1] ChatClient 缺少超时配置和连接超时分离

- **文件**：`20260217-ace-free-alternative-research.md:117-145`
- **问题**：`ChatClient` 结构体设计中缺少超时配置字段。现有项目代码（`src/rust/constants/network.rs`）已定义了 `CONNECTION_TIMEOUT_MS`、`READ_TIMEOUT_MS`、`WRITE_TIMEOUT_MS` 等常量，但方案中的 `ChatClient` 未复用这些配置。
- **风险**：
  - 不同提供者（Ollama 本地 vs 远程 API）需要不同的超时策略
  - 缺少连接超时会导致网络不可达时长时间阻塞
  - 与现有 `create_http_client()` 的超时设计不一致
- **建议**：
  ```rust
  pub struct ChatClient {
      provider: ChatProvider,
      client: reqwest::Client,
      base_url: String,
      api_key: Option<String>,
      model: String,
      // 新增超时配置
      connect_timeout_ms: u64,    // 连接超时（默认 10000）
      request_timeout_ms: u64,    // 请求超时（默认 30000）
      stream_timeout_ms: u64,     // 流式响应超时（默认 120000）
  }
  ```
  并在构造时复用 `crate::constants::network::*` 常量。

### [C2] API Key 存储安全性不足

- **文件**：`20260217-ace-free-alternative-research.md:299-334`
- **问题**：方案中 `enhance_api_key`、`embedding_api_key` 直接存储在 `McpConfig` 中，而现有配置（`settings.json`）是明文 JSON 文件。
- **风险**：
  - API Key 以明文形式存储在用户配置目录
  - 配置文件可能被意外提交到版本控制
  - 日志输出可能泄露敏感信息（现有代码 `log::info!("规范化后的 BASE_URL: {}", base_url)` 已有类似风险）
- **建议**：
  1. 使用系统密钥链存储敏感信息（Windows Credential Manager / macOS Keychain）
  2. 或至少使用简单的混淆/加密存储（如 base64 + XOR）
  3. 在日志输出中对 API Key 进行脱敏处理：
     ```rust
     fn mask_api_key(key: &str) -> String {
         if key.len() <= 8 { return "****".to_string(); }
         format!("{}****{}", &key[..4], &key[key.len()-4..])
     }
     ```
  4. 添加 `.gitignore` 规则确保配置文件不被提交

---

## Warning（建议修复）

### [W1] 重试机制设计不完整

- **文件**：`20260217-ace-free-alternative-research.md:136-145`
- **问题**：`ChatClient` 的 `chat()` 和 `chat_stream()` 方法未设计重试机制，但现有项目已有成熟的 `retry_request()` 实现（`acemcp/mcp.rs:569-607`）。
- **建议**：
  1. 复用现有 `retry_request()` 泛型函数
  2. 或在 `ChatClient` 中添加重试配置：
     ```rust
     pub struct ChatClient {
         // ...
         max_retries: usize,           // 默认 3
         retry_base_delay_secs: f64,   // 默认 1.0（指数退避）
     }
     ```
  3. 区分可重试错误（timeout/connection）和不可重试错误（401/403）

### [W2] EmbeddingClient 缺少批量限速设计

- **文件**：`20260217-ace-free-alternative-research.md:184-189`
- **问题**：`embed_batch()` 方法注释提到"自动分批 + 限速"，但未给出具体设计。不同提供者的批量限制差异很大：
  - Jina: 最大 2048 条/请求
  - OpenAI: 最大 2048 条/请求
  - Ollama: 无官方限制，但本地资源有限
- **建议**：
  ```rust
  pub struct EmbeddingClient {
      // ...
      batch_size: usize,            // 每批最大条数（默认 100）
      rate_limit_rpm: Option<u32>,  // 每分钟请求数限制
      rate_limit_tpm: Option<u32>,  // 每分钟 token 数限制
  }

  impl EmbeddingClient {
      pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
          let mut results = Vec::new();
          for chunk in texts.chunks(self.batch_size) {
              // 限速检查
              self.rate_limiter.acquire().await;
              let batch_result = self.embed_batch_internal(chunk).await?;
              results.extend(batch_result);
          }
          Ok(results)
      }
  }
  ```

### [W3] 适配器层缺少 HTTPS 证书验证配置

- **文件**：`20260217-ace-free-alternative-research.md:154-161`
- **问题**：方案中未提及 HTTPS 证书验证策略。现有代码（`network/client.rs`）使用 reqwest 默认配置，但某些企业环境可能需要自定义 CA 证书或跳过验证（开发环境）。
- **建议**：
  1. 添加可选的证书验证配置：
     ```rust
     pub struct ChatClient {
         // ...
         danger_accept_invalid_certs: bool,  // 默认 false
         custom_ca_cert: Option<PathBuf>,    // 自定义 CA 证书路径
     }
     ```
  2. 在文档中明确说明安全风险

### [W4] RuleEnhancer 规则优先级冲突处理缺失

- **文件**：`20260217-ace-free-alternative-research.md:479-536`
- **问题**：`RuleEnhancer` 设计中，多个规则可能同时匹配同一输入，但未定义冲突处理策略。例如 "fix bug" 同时匹配 `bug` 和 `fix` 两个规则。
- **建议**：
  1. 明确规则匹配策略：首次匹配 vs 全部匹配
  2. 添加规则互斥组配置
  3. 或改为单次匹配（最高优先级规则生效后停止）：
     ```rust
     for rule in rules {
         if rule.trigger.is_match(&enhanced) {
             enhanced = rule.apply(&enhanced, context);
             break;  // 首次匹配后停止
         }
     }
     ```

### [W5] 本地向量存储缺少并发安全设计

- **文件**：`20260217-ace-free-alternative-research.md:547-604`
- **问题**：`VectorStore` 使用 JSON + bincode 文件存储，但未设计并发读写保护。多个 MCP 请求可能同时触发索引更新和搜索。
- **建议**：
  1. 使用文件锁（`fs2::FileExt`）保护写操作
  2. 或使用读写锁模式：
     ```rust
     pub struct LocalIndexManager {
         index_lock: tokio::sync::RwLock<()>,
         // ...
     }

     impl LocalIndexManager {
         pub async fn update_index(&self, ...) {
             let _guard = self.index_lock.write().await;
             // 写入操作
         }

         pub async fn search(&self, ...) {
             let _guard = self.index_lock.read().await;
             // 读取操作
         }
     }
     ```

### [W6] 配置字段命名不一致

- **文件**：`20260217-ace-free-alternative-research.md:299-322`
- **问题**：新增配置字段命名与现有字段风格不一致：
  - 现有：`acemcp_base_url`、`acemcp_token`（模块前缀 + 下划线）
  - 新增：`enhance_provider`、`embedding_provider`（功能前缀 + 下划线）
  - 但 `sou_mode` 使用了不同的前缀
- **建议**：统一命名规范：
  ```rust
  // 方案 A：按模块分组（推荐，与现有风格一致）
  pub enhance_provider: Option<String>,
  pub enhance_base_url: Option<String>,
  pub enhance_api_key: Option<String>,
  pub enhance_model: Option<String>,

  pub sou_embedding_provider: Option<String>,  // 改为 sou_ 前缀
  pub sou_embedding_base_url: Option<String>,
  pub sou_embedding_api_key: Option<String>,
  pub sou_mode: Option<String>,
  ```

---

## Info（可选修复）

### [I1] AST 分片依赖未明确版本

- **文件**：`20260217-ace-free-alternative-research.md:411-428`
- **问题**：提到可选引入 `tree-sitter` crate，但未指定版本和所需的语言 parser crates。
- **建议**：明确依赖版本：
  ```toml
  # Cargo.toml（可选依赖）
  [dependencies]
  tree-sitter = { version = "0.22", optional = true }
  tree-sitter-rust = { version = "0.21", optional = true }
  tree-sitter-typescript = { version = "0.21", optional = true }
  tree-sitter-python = { version = "0.21", optional = true }

  [features]
  ast-chunking = ["tree-sitter", "tree-sitter-rust", "tree-sitter-typescript", "tree-sitter-python"]
  ```

### [I2] RRF 融合算法缺少 k 值配置化

- **文件**：`20260217-ace-free-alternative-research.md:432-475`
- **问题**：RRF 常数 `k=60` 硬编码在函数中，但不同场景可能需要调整。
- **建议**：将 `k` 值配置化：
  ```rust
  pub search_rrf_k: Option<f32>,  // 默认 60.0
  ```

### [I3] 提供者对比表缺少错误码映射

- **文件**：`20260217-ace-free-alternative-research.md:739-748`
- **问题**：提供者对比表详细列出了免费额度和延迟，但未说明各提供者的错误码差异，这对适配器实现很重要。
- **建议**：补充错误码映射表：
  | 提供者 | 限流错误码 | 认证错误码 | 服务不可用 |
  |--------|-----------|-----------|-----------|
  | OpenAI | 429 | 401 | 500-599 |
  | Jina | 429 | 401 | 503 |
  | Cloudflare | 429 | 403 | 500 |
  | Cohere | 429 | 401 | 503 |

### [I4] 阶段划分缺少验收标准

- **文件**：`20260217-ace-free-alternative-research.md:349-368`
- **问题**：三个实施阶段的验收标准描述较简略（"端到端可用"），缺少具体的测试用例。
- **建议**：为每个阶段补充验收清单：
  ```markdown
  ### 阶段 1 验收标准
  - [ ] Ollama 本地模式：`enhance` 工具返回增强后的 prompt
  - [ ] OpenAI 兼容模式：配置 SiliconFlow API Key 后可用
  - [ ] 规则引擎模式：无 API 配置时自动降级
  - [ ] 降级链测试：Ollama 不可用时自动切换到 L2/L3
  - [ ] 错误处理：API 超时/限流时返回友好错误信息
  ```

---

## 改进建议

### [S1] 建议添加健康检查机制

- **说明**：方案中提到 `is_available()` 方法检测服务可用性，建议扩展为完整的健康检查机制：
  ```rust
  pub struct HealthStatus {
      pub provider: String,
      pub is_available: bool,
      pub latency_ms: Option<u64>,
      pub last_check: DateTime<Utc>,
      pub error_message: Option<String>,
  }

  impl ChatClient {
      pub async fn health_check(&self) -> HealthStatus;
  }
  ```
  并在前端设置页面展示各提供者的健康状态。

### [S2] 建议添加 Token 消耗追踪

- **说明**：方案借鉴了 claude-mem 的"Token 成本可见性"，建议在 `ChatClient` 和 `EmbeddingClient` 中添加 token 计数：
  ```rust
  pub struct UsageStats {
      pub prompt_tokens: u64,
      pub completion_tokens: u64,
      pub embedding_tokens: u64,
      pub total_cost_usd: f64,  // 估算成本
  }
  ```

---

## 总结

该方案整体设计合理，充分借鉴了 5 个开源项目的优点，统一 API 接口层的设计思路清晰。主要问题集中在：

1. **安全性**：API Key 明文存储需要改进
2. **健壮性**：超时、重试、并发控制等细节需要补充
3. **一致性**：配置命名风格需要统一

建议在阶段 1 实施前先完成 C1、C2 两个 Critical 问题的设计补充，W1-W3 可在实施过程中逐步完善。
