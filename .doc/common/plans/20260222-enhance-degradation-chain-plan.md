# Enhance 降级链修复实施计划（v2 — team-exec 零决策格式）

> **日期**: 2026-02-22（v2 更新）
> **来源**: `.doc/agent-teams/research/20260221-enhance-degradation-chain-research.md`
> **范围**: enhance 模块降级链断裂修复 + 超时优化 + 前端渠道排序 + 开发环境体验 + MCP 协议诊断 + Telegram 链路加固
> **涉及文件**: 12 个核心文件
> **双模型交叉验证**: Codex（后端链路 + 超时分析）+ Gemini（前端方案 + 架构格式）

---

## 执行架构

### Builder 分配

```
Builder A（核心链路）: core.rs + provider_factory.rs + enhance/mcp.rs
Builder B（前端 + 命令）: commands.rs + EnhanceConfig.vue + settings.rs（仅验证）
Builder C（独立修复）: chat_client.rs + popup.rs + server.rs + cli.rs + mcp_handler.rs
```

### 并行/串行依赖图

```
T0 ──────────────────────────────────────────────── T_end
  Builder A: [步骤1 → 步骤2 → 步骤3]  (阶段1 P0 核心)
  Builder B: [步骤6 → 步骤7 → 步骤8]  (阶段3 P1 前端)  ← 并行
  Builder C: [步骤4 → 步骤5 → 步骤9 → 步骤10 → 步骤11]  ← 并行
                                                    │
                                                    ▼
                                              统一编译验证
                                              cargo check
                                              cargo test
                                              pnpm vitest
```

**并行安全性**：
- Builder A 修改 `core.rs` 的结构体字段，Builder C 修改 `chat_client.rs` 的超时/Client 复用 → 无编译依赖（ChatClient 接口不变）
- Builder B 的 `commands.rs` DTO 扩展与 Builder A 的 `core.rs` 无交叉
- Builder C 的 `popup.rs` / `server.rs` / `cli.rs` 与 Builder A/B 完全隔离

---

## 阶段总览

```
阶段 1（P0 核心修复）：修复降级链断裂 + 错误码误导          [Builder A]
  ├── 步骤 1: core.rs — PromptEnhancer 持有 Vec<ChatClient>
  ├── 步骤 2: provider_factory.rs — 删除单候选兼容接口
  └── 步骤 3: enhance/mcp.rs — CallToolResult 错误码修正

阶段 2（P1 稳定性）：超时优化 + 重试增强                    [Builder C]
  ├── 步骤 4: chat_client.rs — 超时调整 + 指数退避 + Client 复用
  └── 步骤 5: chat_client.rs — 重试日志级别提升

阶段 3（P1 功能完善）：前端渠道排序 UI                      [Builder B]
  ├── 步骤 6: EnhanceConfigDto — 新增 channel_order 字段
  ├── 步骤 7: commands.rs — get/save 命令支持 channel_order
  └── 步骤 8: EnhanceConfig.vue — 渠道排序 UI 组件

阶段 4（P2 开发体验）：开发环境弹窗优化                     [Builder C]
  └── 步骤 9: popup.rs — 开发环境前置检测与错误提示

阶段 5（P0/P1 补充）：MCP 协议诊断 + Telegram 链路加固      [Builder C]
  ├── 步骤 10: server.rs — ConnectionClosed 结构化诊断
  └── 步骤 11: cli.rs + mcp_handler.rs — Telegram 失败错误响应

阶段 6（P2 文档）：文档不一致修复                           [任意 Builder 收尾]
  └── 步骤 12: 文档修复（test_mcp_connection.ps1 + mcp_wrapper.py 集成说明）
```

**依赖关系**：阶段 1 内部串行，阶段 2/3/4/5 与阶段 1 并行，阶段 6 最后执行。

---

## 阶段 1：P0 核心修复 — 降级链断裂 + 错误码

### 步骤 1：PromptEnhancer 持有 Vec<ChatClient>，实现 fallback 循环

**文件**: `src/rust/mcp/tools/enhance/core.rs`
**Builder**: A
**验收命令**: `cargo check -p sanshu`

**1.1 结构体字段变更** (L61-72)

```rust
// 修改前（L63）
chat_client: Option<ChatClient>,

// 修改后
chat_clients: Vec<ChatClient>,
```

**1.2 构造方法 `with_chat_client` → `with_chat_clients`** (L117-133)

```rust
// 修改前
pub fn with_chat_client(chat_client: ChatClient) -> Result<Self> {
    // ...
    Ok(Self {
        chat_client: Some(chat_client),
        // ...
    })
}

// 修改后
pub fn with_chat_clients(chat_clients: Vec<ChatClient>) -> Result<Self> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    // 从第一个候选获取 base_url 和 token（用于旧路径兼容）
    let (base_url, token) = chat_clients.first()
        .map(|c| (c.base_url.clone(), c.api_key.clone().unwrap_or_default()))
        .unwrap_or_default();

    Ok(Self {
        chat_clients,
        base_url,
        token,
        client,
        project_root: None,
    })
}
```

**1.3 `new()` 构造方法同步更新** (L102-115)

```rust
Ok(Self {
    chat_clients: Vec::new(),  // 修改: Option -> Vec
    // 其余字段不变
})
```

**1.4 `from_mcp_config` 改用 `build_enhance_candidates_async`** (L155-178)

```rust
// 修改前（L158, L162）
use crate::mcp::tools::enhance::provider_factory::build_enhance_client_async;
let chat_client = build_enhance_client_async(&mcp_config).await;

// 修改后
use crate::mcp::tools::enhance::provider_factory::build_enhance_candidates_async;
let chat_clients = build_enhance_candidates_async(&mcp_config).await;

log_important!(
    info,
    "enhance 候选列表: {} 个候选 [{}]",
    chat_clients.len(),
    chat_clients.iter()
        .map(|c| format!("{:?}", c.provider))
        .collect::<Vec<_>>()
        .join(" -> ")
);

// 为第一个非 RuleEngine 候选记录详细日志
if let Some(primary) = chat_clients.iter().find(|c| c.provider != ChatProvider::RuleEngine) {
    log_important!(info, "主候选: {:?}, model={}", primary.provider, primary.model);
    if let Some(ref key) = primary.api_key {
        if !key.is_empty() {
            log_important!(info, "API Key: {}", mask_api_key(key));
        }
    }
}

Self::with_chat_clients(chat_clients)
```
**1.5 `enhance()` 方法实现 fallback 循环** (L540-579)

```rust
// 修改前（L566-579）
if let Some(ref chat_client) = self.chat_client {
    return self.enhance_via_chat_client(
        chat_client,
        &request.prompt,
        &response_original_prompt,
        blob_count,
        history_count,
        history_load_error,
        history_fallback_used,
        project_root_path,
        blob_source_root,
        request_id,
    ).await;
}

// 修改后
if !self.chat_clients.is_empty() {
    let mut last_error: Option<String> = None;
    for (idx, chat_client) in self.chat_clients.iter().enumerate() {
        log_important!(
            info,
            "enhance fallback: 尝试候选 {}/{} — {:?} (model={})",
            idx + 1,
            self.chat_clients.len(),
            chat_client.provider,
            chat_client.model
        );
        match self.enhance_via_chat_client(
            chat_client,
            &request.prompt,
            &response_original_prompt,
            blob_count,
            history_count,
            history_load_error.clone(),
            history_fallback_used,
            project_root_path.clone(),
            blob_source_root.clone(),
            request_id.clone(),
        ).await {
            Ok(resp) if resp.success => return Ok(resp),
            Ok(resp) => {
                let err_msg = resp.error.unwrap_or_else(|| "未知错误".to_string());
                log_important!(
                    warn,
                    "enhance fallback: 候选 {:?} 失败 — {}，尝试下一个",
                    chat_client.provider,
                    err_msg
                );
                last_error = Some(err_msg);
            }
            Err(e) => {
                log_important!(
                    warn,
                    "enhance fallback: 候选 {:?} 异常 — {}，尝试下一个",
                    chat_client.provider,
                    e
                );
                last_error = Some(format!("{}", e));
            }
        }
    }
    // 所有候选均失败
    return Ok(EnhanceResponse {
        enhanced_prompt: String::new(),
        original_prompt: response_original_prompt,
        success: false,
        error: Some(format!(
            "所有 {} 个候选均失败，最后错误: {}",
            self.chat_clients.len(),
            last_error.unwrap_or_default()
        )),
        blob_count,
        history_count,
        history_load_error,
        history_fallback_used,
        project_root_path,
        blob_source_root,
        request_id: Some(request_id),
    });
}
```

> **注意**: `enhance_via_chat_client` 内部（L742-758）当前在 `Err(e)` 分支返回 `Ok(EnhanceResponse { success: false, ... })`，
> 这个行为保持不变——外层 fallback 循环通过检查 `resp.success` 判断是否继续。

**1.6 `enhance_stream()` 方法同步改造** (L813-828)

```rust
// 修改前（L814-828）
if let Some(ref chat_client) = self.chat_client {
    return self.enhance_stream_via_chat_client(
        chat_client,
        &request.prompt,
        &response_original_prompt,
        blob_count,
        history_count,
        history_load_error,
        history_fallback_used,
        project_root_path,
        blob_source_root,
        request_id,
        on_event,
    ).await;
}

// 修改后
if !self.chat_clients.is_empty() {
    let mut last_error: Option<String> = None;
    for (idx, chat_client) in self.chat_clients.iter().enumerate() {
        log_important!(
            info,
            "enhance_stream fallback: 尝试候选 {}/{} — {:?}",
            idx + 1,
            self.chat_clients.len(),
            chat_client.provider
        );
        match self.enhance_stream_via_chat_client(
            chat_client,
            &request.prompt,
            &response_original_prompt,
            blob_count,
            history_count,
            history_load_error.clone(),
            history_fallback_used,
            project_root_path.clone(),
            blob_source_root.clone(),
            request_id.clone(),
            &mut on_event,
        ).await {
            Ok(resp) if resp.success => return Ok(resp),
            Ok(resp) => {
                let err_msg = resp.error.unwrap_or_else(|| "未知错误".to_string());
                log_important!(warn, "enhance_stream fallback: {:?} 失败 — {}", chat_client.provider, err_msg);
                last_error = Some(err_msg);
            }
            Err(e) => {
                log_important!(warn, "enhance_stream fallback: {:?} 异常 — {}", chat_client.provider, e);
                last_error = Some(format!("{}", e));
            }
        }
    }
    let final_err = format!(
        "所有 {} 个候选均失败，最后错误: {}",
        self.chat_clients.len(),
        last_error.unwrap_or_default()
    );
    on_event(EnhanceStreamEvent::error(&request_id, &final_err));
    return Ok(EnhanceResponse {
        enhanced_prompt: String::new(),
        original_prompt: response_original_prompt,
        success: false,
        error: Some(final_err),
        blob_count,
        history_count,
        history_load_error,
        history_fallback_used,
        project_root_path,
        blob_source_root,
        request_id: Some(request_id),
    });
}
```

> **注意**: `enhance_stream_via_chat_client` 的签名中 `on_event` 参数需改为 `&mut F`（可变引用），
> 因为 fallback 循环中多次调用需要复用同一个回调。同时 `enhance_stream` 的签名中 `mut on_event: F` 保持不变。

---

### 步骤 2：provider_factory.rs — 删除单候选兼容接口

**文件**: `src/rust/mcp/tools/enhance/provider_factory.rs`
**Builder**: A
**验收命令**: `cargo check -p sanshu`

**2.1 删除 `build_enhance_client`** (L143-147)

```rust
// 删除以下函数（L143-147）
/// 兼容旧接口：返回第一个候选（同步）
pub fn build_enhance_client(config: &McpConfig) -> ChatClient {
    build_enhance_candidates(config).into_iter().next()
        .unwrap_or_else(build_rule_engine)
}
```

**2.2 删除 `build_enhance_client_async`** (L149-153)

```rust
// 删除以下函数（L149-153）
/// 兼容旧接口：返回第一个候选（异步）
pub async fn build_enhance_client_async(config: &McpConfig) -> ChatClient {
    build_enhance_candidates_async(config).await.into_iter().next()
        .unwrap_or_else(build_rule_engine)
}
```

**2.3 更新 `mod.rs` 导出** (`src/rust/mcp/tools/enhance/mod.rs` L23)

```rust
// 修改前（L23）
pub use provider_factory::{build_enhance_client, build_enhance_client_async};

// 修改后
pub use provider_factory::build_enhance_candidates_async;
```

> **注意**: 删除后需全局搜索 `build_enhance_client` 确认无其他调用点。
> 已知调用点仅在 `core.rs:from_mcp_config()`（步骤 1.4 已改为 `build_enhance_candidates_async`）。

---

### 步骤 3：enhance/mcp.rs — CallToolResult 错误码修正

**文件**: `src/rust/mcp/tools/enhance/mcp.rs`
**Builder**: A
**验收命令**: `cargo check -p sanshu`

**3.1 修正失败时的 CallToolResult** (L130-136)

```rust
// 修改前（L130-136）
} else {
    // 失败：返回错误信息
    let error_text = format!(
        "增强失败: {}",
        response.error.unwrap_or_else(|| "未知错误".to_string())
    );
    Ok(CallToolResult::success(vec![Content::text(error_text)]))
}

// 修改后
} else {
    // 失败：返回带 is_error 标记的错误信息
    let error_text = format!(
        "增强失败: {}",
        response.error.unwrap_or_else(|| "未知错误".to_string())
    );
    Ok(CallToolResult {
        content: vec![Content::text(error_text)],
        is_error: Some(true),
        ..Default::default()
    })
}
```

> **注意**: 需确认 rmcp 0.12.0 的 `CallToolResult` 是否支持 `is_error` 字段和 `Default` trait。
> 如果不支持 `Default`，改用 `CallToolResult::error(vec![Content::text(error_text)])` 构造方法。
> Builder 执行前先 `grep -rn "is_error" src/rust/` 和 `grep -rn "CallToolResult" src/rust/mcp/tools/enhance/mcp.rs` 确认 API。

---

## 阶段 2：P1 稳定性 — 超时优化 + 重试增强

### 步骤 4：chat_client.rs — 超时调整 + 指数退避 + Client 复用

**文件**: `src/rust/mcp/tools/enhance/chat_client.rs`
**Builder**: C
**验收命令**: `cargo check -p sanshu`

**4.1 超时调整** (L57-61)

```rust
// 修改前（L59）
ChatProvider::OpenAICompat | ChatProvider::Gemini | ChatProvider::Anthropic => (10_000, 30_000, 120_000),

// 修改后
ChatProvider::OpenAICompat | ChatProvider::Gemini | ChatProvider::Anthropic => (10_000, 60_000, 120_000),
```

> 将 L2 云端 API 的 `request_timeout` 从 30s 提升到 60s，与 Ollama 对齐。

**4.2 Client 复用 — 结构体新增字段** (L40-48)

```rust
// 修改前（L40-48）
pub struct ChatClient {
    pub provider: ChatProvider,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub stream_timeout_ms: u64,
}

// 修改后
pub struct ChatClient {
    pub provider: ChatProvider,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub connect_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub stream_timeout_ms: u64,
    /// 复用的 HTTP 客户端（非流式请求）
    http_client: reqwest::Client,
    /// 复用的 HTTP 客户端（流式请求）
    http_client_stream: reqwest::Client,
}
```

**4.3 构造方法中初始化 Client** (L50-71)

```rust
// 修改前（L62-71）
Self {
    provider,
    base_url,
    api_key,
    model,
    connect_timeout_ms: connect_ms,
    request_timeout_ms: request_ms,
    stream_timeout_ms: stream_ms,
}

// 修改后
let http_client = reqwest::Client::builder()
    .connect_timeout(Duration::from_millis(connect_ms))
    .timeout(Duration::from_millis(request_ms))
    .build()
    .unwrap_or_default();
let http_client_stream = reqwest::Client::builder()
    .connect_timeout(Duration::from_millis(connect_ms))
    .timeout(Duration::from_millis(stream_ms))
    .build()
    .unwrap_or_default();

Self {
    provider,
    base_url,
    api_key,
    model,
    connect_timeout_ms: connect_ms,
    request_timeout_ms: request_ms,
    stream_timeout_ms: stream_ms,
    http_client,
    http_client_stream,
}
```

**4.4 `build_client` 改为返回已有 Client** (L73-84)

```rust
// 修改前（L73-84）
fn build_client(&self, is_stream: bool) -> Result<reqwest::Client> {
    let timeout_ms = if is_stream {
        self.stream_timeout_ms
    } else {
        self.request_timeout_ms
    };
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(self.connect_timeout_ms))
        .timeout(Duration::from_millis(timeout_ms))
        .build()?;
    Ok(client)
}

// 修改后
fn build_client(&self, is_stream: bool) -> Result<reqwest::Client> {
    if is_stream {
        Ok(self.http_client_stream.clone())
    } else {
        Ok(self.http_client.clone())
    }
}
```

> **注意**: `reqwest::Client` 内部使用 `Arc`，`clone()` 是廉价的引用计数增加。

**4.5 `chat_with_retry` 增加指数退避** (L248-263)

```rust
// 修改前（L248-263）
/// 带简单重试的 chat（最多 2 次）
pub async fn chat_with_retry(&self, messages: &[Message]) -> Result<String> {
    let mut last_err = None;
    for attempt in 0..2 {
        match self.chat(messages).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == 0 {
                    log::debug!("chat 第 1 次失败，重试: {}", e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("chat_with_retry 未知错误")))
}

// 修改后
/// 带指数退避的重试（最多 3 次尝试，退避 1s → 2s）
pub async fn chat_with_retry(&self, messages: &[Message]) -> Result<String> {
    let max_attempts = 3;
    let mut last_err = None;
    for attempt in 0..max_attempts {
        match self.chat(messages).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                log_important!(
                    warn,
                    "chat 第 {} 次失败 (共 {} 次): provider={:?}, error={}",
                    attempt + 1,
                    max_attempts,
                    self.provider,
                    e
                );
                last_err = Some(e);
                if attempt < max_attempts - 1 {
                    let backoff_ms = 1000 * (1 << attempt); // 1s, 2s
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("chat_with_retry 未知错误")))
}
```

> **注意**: 需在文件顶部确认 `use std::time::Duration;` 已导入（当前 L3 已有）。
> 需确认 `log_important!` 宏已导入（当前文件顶部应有 `use crate::log_important;`，若无需添加）。

---

### 步骤 5：chat_client.rs — 重试日志级别提升

**文件**: `src/rust/mcp/tools/enhance/chat_client.rs`
**Builder**: C
**验收命令**: `cargo check -p sanshu`

> 此步骤已在步骤 4.5 中合并完成。原 `log::debug!` (L256) 已改为 `log_important!(warn, ...)`。
> 无需额外修改。

---

## 阶段 3：P1 功能完善 — 前端渠道排序 UI

### 步骤 6：EnhanceConfigDto — 新增 channel_order 字段

**文件**: `src/rust/mcp/commands.rs`
**Builder**: B
**验收命令**: `cargo check -p sanshu`

**6.1 DTO 结构体新增字段** (L687-701)

```rust
// 修改前（L687-701）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnhanceConfigDto {
    pub provider: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

// 修改后
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnhanceConfigDto {
    pub provider: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// 渠道降级顺序，如 ["ollama", "cloud", "rule_engine"]
    #[serde(default)]
    pub channel_order: Option<Vec<String>>,
}
```

### 步骤 7：commands.rs — get/save 命令支持 channel_order

**文件**: `src/rust/mcp/commands.rs`
**Builder**: B
**验收命令**: `cargo check -p sanshu`

**7.1 get_enhance_config 读取 channel_order** (L705-717)

```rust
// 修改前（L709-716）
Ok(EnhanceConfigDto {
    provider: mcp.enhance_provider.clone().unwrap_or_else(|| "ollama".to_string()),
    ollama_url: mcp.enhance_ollama_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string()),
    ollama_model: mcp.enhance_ollama_model.clone().unwrap_or_else(|| "qwen2.5-coder:7b".to_string()),
    base_url: mcp.enhance_base_url.clone().unwrap_or_default(),
    api_key: mcp.enhance_api_key.clone().unwrap_or_default(),
    model: mcp.enhance_model.clone().unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string()),
})

// 修改后
Ok(EnhanceConfigDto {
    provider: mcp.enhance_provider.clone().unwrap_or_else(|| "ollama".to_string()),
    ollama_url: mcp.enhance_ollama_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string()),
    ollama_model: mcp.enhance_ollama_model.clone().unwrap_or_else(|| "qwen2.5-coder:7b".to_string()),
    base_url: mcp.enhance_base_url.clone().unwrap_or_default(),
    api_key: mcp.enhance_api_key.clone().unwrap_or_default(),
    model: mcp.enhance_model.clone().unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string()),
    channel_order: mcp.enhance_channel_order.clone(),
})
```

**7.2 save_enhance_config 保存 channel_order** (L726-736)

```rust
// 修改前（L730-735）
mcp.enhance_provider = Some(config_dto.provider);
mcp.enhance_ollama_url = Some(config_dto.ollama_url);
mcp.enhance_ollama_model = Some(config_dto.ollama_model);
mcp.enhance_base_url = if config_dto.base_url.is_empty() { None } else { Some(config_dto.base_url) };
mcp.enhance_api_key = if config_dto.api_key.is_empty() { None } else { Some(config_dto.api_key) };
mcp.enhance_model = Some(config_dto.model);

// 修改后
mcp.enhance_provider = Some(config_dto.provider);
mcp.enhance_ollama_url = Some(config_dto.ollama_url);
mcp.enhance_ollama_model = Some(config_dto.ollama_model);
mcp.enhance_base_url = if config_dto.base_url.is_empty() { None } else { Some(config_dto.base_url) };
mcp.enhance_api_key = if config_dto.api_key.is_empty() { None } else { Some(config_dto.api_key) };
mcp.enhance_model = Some(config_dto.model);
mcp.enhance_channel_order = config_dto.channel_order;
```

**7.3 settings.rs — 确认 McpConfig 字段存在** (需验证)

```rust
// 确认 src/rust/config/settings.rs 中 McpConfig 已有以下字段：
pub enhance_channel_order: Option<Vec<String>>,
```

> **Builder 执行前验证**：`grep -n "enhance_channel_order" src/rust/config/settings.rs`
> 若不存在需添加到 `McpConfig` 结构体中。

---

### 步骤 8：EnhanceConfig.vue — 渠道排序 UI 组件

**文件**: `src/frontend/components/tools/EnhanceConfig.vue`
**Builder**: B
**验收命令**: `pnpm vitest run EnhanceConfig`

**8.1 config ref 新增 channel_order** (L19-26)

```typescript
// 修改前（L19-26）
const config = ref({
  provider: 'ollama',
  ollama_url: 'http://localhost:11434',
  ollama_model: 'qwen2.5-coder:7b',
  base_url: '',
  api_key: '',
  model: '',
})

// 修改后
const config = ref({
  provider: 'ollama',
  ollama_url: 'http://localhost:11434',
  ollama_model: 'qwen2.5-coder:7b',
  base_url: '',
  api_key: '',
  model: '',
  channel_order: ['ollama', 'cloud', 'rule_engine'] as string[],
})
```

**8.2 loadConfig 解析 channel_order** (L84-91)

```typescript
// 修改前（L84-91）
config.value = {
  provider: res.provider || 'ollama',
  ollama_url: res.ollama_url || 'http://localhost:11434',
  ollama_model: res.ollama_model || 'qwen2.5-coder:7b',
  base_url: res.base_url || '',
  api_key: res.api_key || '',
  model: res.model || '',
}

// 修改后
config.value = {
  provider: res.provider || 'ollama',
  ollama_url: res.ollama_url || 'http://localhost:11434',
  ollama_model: res.ollama_model || 'qwen2.5-coder:7b',
  base_url: res.base_url || '',
  api_key: res.api_key || '',
  model: res.model || '',
  channel_order: res.channel_order || ['ollama', 'cloud', 'rule_engine'],
}
```

**8.3 template 新增渠道排序 UI** (在供应商选择器下方添加)

```vue
<!-- 在 provider 选择器的 ConfigSection 后添加 -->
<ConfigSection title="降级顺序" description="拖拽调整渠道优先级，失败时按顺序尝试">
  <n-dynamic-tags
    v-model:value="config.channel_order"
    :render-tag="renderChannelTag"
  >
    <template #trigger="{ activate }">
      <n-button size="small" @click="activate">
        + 添加渠道
      </n-button>
    </template>
  </n-dynamic-tags>
  <n-text depth="3" style="font-size: 12px; margin-top: 8px; display: block;">
    可选值: ollama, cloud, rule_engine
  </n-text>
</ConfigSection>
```

**8.4 新增 renderChannelTag 函数** (在 script setup 中添加)

```typescript
import { NTag } from 'naive-ui'
import { h } from 'vue'

const CHANNEL_LABELS: Record<string, string> = {
  ollama: 'Ollama 本地',
  cloud: '云端 API',
  rule_engine: '规则引擎',
}

function renderChannelTag(tag: string, index: number) {
  return h(
    NTag,
    {
      type: index === 0 ? 'success' : 'default',
      closable: true,
      onClose: () => {
        config.value.channel_order.splice(index, 1)
      },
    },
    { default: () => CHANNEL_LABELS[tag] || tag }
  )
}
```

> **注意**: 需在文件顶部 import 中添加 `NTag` 和 `h`。

---

## 阶段 4：P2 开发体验 — 开发环境弹窗优化

### 步骤 9：popup.rs — 开发环境前置检测与错误提示

**文件**: `src/rust/mcp/handlers/popup.rs`
**Builder**: C
**验收命令**: `cargo check -p sanshu`

**9.1 find_ui_command 增加开发环境检测** (L95-126)

```rust
// 修改前（L119-125）
// 3. Return detailed error when command cannot be found.
anyhow::bail!(
    "UI command not found (tried sanshu-gui). Please ensure:\n\
     1. Build is done: cargo build --release\n\
     2. Or install script has run: ./install.sh\n\
     3. Or the executable is in the same directory as MCP server"
)

// 修改后
// 3. 检测开发环境并提供针对性错误
let is_dev_env = std::env::var("CARGO_MANIFEST_DIR").is_ok()
    || std::env::current_dir()
        .map(|p| p.join("Cargo.toml").exists())
        .unwrap_or(false);

if is_dev_env {
    anyhow::bail!(
        "开发环境未启动前端服务。请先执行:\n\
         1. 终端 1: pnpm tauri:dev\n\
         2. 等待 Vite 启动后再测试 MCP 工具\n\
         或者使用 cargo build --release 构建完整版本"
    )
} else {
    anyhow::bail!(
        "UI command not found (tried sanshu-gui). Please ensure:\n\
         1. Build is done: cargo build --release\n\
         2. Or install script has run: ./install.sh\n\
         3. Or the executable is in the same directory as MCP server"
    )
}
```

---

## 阶段 5：P0/P1 补充 — MCP 协议诊断 + Telegram 链路加固

### 步骤 10：server.rs — ConnectionClosed 结构化诊断

**文件**: `src/rust/mcp/server.rs`
**Builder**: C
**验收命令**: `cargo check -p sanshu`

**10.1 增强 ConnectionClosed 错误提示** (L468-472)

```rust
// 修改前（L468-472）
ServerInitializeError::ConnectionClosed(_) => {
    log_important!(
        error,
        "启动服务器失败：初始化阶段连接已关闭。通常是未通过 MCP 客户端以 stdio 管道启动，或客户端启动后立即退出。请检查 MCP 客户端配置（command/args/stdio），不要直接双击运行。"
    );
}

// 修改后
ServerInitializeError::ConnectionClosed(_) => {
    log_important!(
        error,
        "启动服务器失败：初始化阶段连接已关闭。\n\
         \n\
         可能原因：\n\
         1. 直接双击运行 MCP 服务器（应通过 AI 助手配置启动）\n\
         2. MCP 客户端未发送 initialize 请求即断开\n\
         3. MCP 客户端配置错误（command/args/stdio）\n\
         \n\
         解决方案：\n\
         - Claude Desktop: 检查 claude_desktop_config.json 中的 mcpServers 配置\n\
         - Cursor: 检查 ~/.cursor/mcp.json 配置\n\
         - 确保 command 指向正确的可执行文件路径\n\
         \n\
         调试命令: echo '{{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"params\":{{...}},\"id\":1}}' | ./三术.exe"
    );
}
```

---

### 步骤 11：cli.rs + mcp_handler.rs — Telegram 失败错误响应

**文件**: `src/rust/app/cli.rs`
**Builder**: C
**验收命令**: `cargo check -p sanshu`

**11.1 handle_mcp_request 增强 Telegram 错误处理** (L205-211)

```rust
// 修改前（L205-211）
if let Err(e) = tokio::runtime::Runtime::new()
    .unwrap()
    .block_on(handle_telegram_only_mcp_request(request_file))
{
    log_important!(error, "处理Telegram请求失败: {}", e);
    std::process::exit(1);
}

// 修改后
match tokio::runtime::Runtime::new()
    .unwrap()
    .block_on(handle_telegram_only_mcp_request(request_file))
{
    Ok(_) => {}
    Err(e) => {
        let error_msg = format!(
            "Telegram 交互失败: {}\n\
             \n\
             可能原因：\n\
             1. Telegram Bot Token 无效或过期\n\
             2. Chat ID 配置错误\n\
             3. 网络连接超时（Telegram API 不可达）\n\
             4. Bot 未添加到指定聊天\n\
             \n\
             请检查配置文件中的 telegram.bot_token 和 telegram.chat_id",
            e
        );
        log_important!(error, "{}", error_msg);
        // 输出到 stdout 供 MCP 客户端捕获
        println!("{}", serde_json::json!({
            "success": false,
            "error": error_msg
        }));
        std::process::exit(1);
    }
}
```

> **注意**: 需确保输出 JSON 格式，供 MCP 客户端解析错误信息。

---

## 阶段 6：P2 文档 — 文档不一致修复

### 步骤 12：文档修复

**Builder**: 任意 Builder 收尾
**验收命令**: 手动检查文档一致性

**12.1 test_mcp_connection.ps1 语法修复**

```powershell
# 文件: test_mcp_connection.ps1
# 修复语法错误，确保脚本可直接运行

# 检查当前脚本语法
powershell -NoProfile -Command "& { . .\test_mcp_connection.ps1 }"
```

**12.2 mcp_wrapper.py 集成说明**

在项目根目录 README 或 MCP 配置文档中添加：

```markdown
## MCP Python 包装器

`mcp_wrapper.py` 提供了 Python 封装，可用于自动化测试：

```python
from mcp_wrapper import McpClient

client = McpClient("./三术.exe")
result = client.call_tool("zhi", {"message": "测试"})
```

安装：无需安装，直接引用项目根目录的 `mcp_wrapper.py`
```

**12.3 日志路径统一**

检查并统一代码和文档中的日志路径引用：
- 代码中使用 `log/`
- 文档中应同步为 `log/`（而非 `logs/`）

---

## 验收清单

### 编译验证（所有 Builder 完成后）

```bash
# Rust 编译检查
cargo check -p sanshu

# Rust 测试
cargo test -p sanshu

# 前端测试
pnpm vitest run EnhanceConfig
```

### 功能验证

| 步骤 | 验证方法 | 预期结果 |
|------|----------|----------|
| 1-3 | 配置 Ollama 超时，观察日志 | 看到 fallback 循环日志，最终触发 L3 |
| 4-5 | 配置云端 API 并断网 | 看到 3 次重试 + 指数退避日志 |
| 6-8 | 前端拖拽渠道顺序并保存 | 配置持久化，重启后保持 |
| 9 | 开发环境直接运行 MCP | 看到"请先执行 pnpm tauri:dev"提示 |
| 10 | 直接双击三术.exe | 看到结构化诊断信息 |
| 11 | 配置错误 Telegram Token | 看到 JSON 格式错误响应 |

### Builder 完成确认

- [ ] Builder A: 步骤 1-3 完成，`cargo check` 通过
- [ ] Builder B: 步骤 6-8 完成，`pnpm vitest` 通过
- [ ] Builder C: 步骤 4-5, 9-11 完成，`cargo check` 通过
- [ ] 任意 Builder: 步骤 12 完成
- [ ] 统一编译验证通过

---

## 风险与回滚

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| rmcp `CallToolResult` API 不兼容 | 低 | 步骤 3 需调整 | 执行前 grep 确认 API |
| 前端 n-dynamic-tags 组件不存在 | 低 | 步骤 8 需换组件 | 改用 n-select + draggable |
| settings.rs 缺少字段 | 中 | 步骤 7 需补充 | 执行前 grep 验证 |

**回滚方案**：所有修改均为独立文件，`git checkout -- <file>` 可逐个回滚。
