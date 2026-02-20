# 上下文检索优化 — 零决策可执行计划

**日期**: 2026-02-19
**来源**: `.doc/agent-teams/research/20260218-context-retrieval-research.md`（v3）
**规划模式**: Claude 独立分析（collab Skill 不可用，双模型降级）
**状态**: 待审批

---

## 执行元数据

| 字段 | 值 |
|------|-----|
| 约束集版本 | v3（2026-02-19） |
| 硬约束数 | 15（HC-1 ~ HC-15） |
| 软约束数 | 20（SC-1 ~ SC-20） |
| 依赖关系数 | 9（DEP-1 ~ DEP-9） |
| 风险点数 | 10（R-1 ~ R-10） |
| 计划步骤数 | 32 |
| 双模型状态 | DEGRADED（collab Skill 缺失 → Claude 独立规划） |

---

## 依赖拓扑排序

基于 DEP-1~DEP-9 和优先级 P0~P3，确定以下执行顺序：

```
P0 阶段:
  HC-5 (并发保护) ──→ DEP-3 (多客户端支持)
  HC-6 (统一错误分类) ──→ DEP-4 (智能降级)
  HC-9 (密钥安全)
  HC-10 (记忆大小限制)
  HC-15 (资源上限)

P1 阶段:
  HC-7 (搜索缓存)
  HC-11 (后端测试) ──→ DEP-8 (前端开发保障)
  SC-5 (记忆更新) ──→ DEP-6 (记忆 UI)
  SC-15 (数据迁移)
  SC-17 (IPC 弹性)
  HC-8 (记忆 UI) ← 依赖 HC-2/SC-5/DEP-6

P2 阶段:
  HC-14 (前端测试) ← 依赖 DEP-8
  SC-6 (记忆版本控制)
  SC-8 (磁盘缓存)
  SC-9 (搜索 UI 预览)
  SC-10 (搜索实时反馈)
  SC-13 (配置热更新)
  SC-14 (可观测性)
  SC-16 (前端状态持久化)

P3 阶段:
  HC-12 (A11y)
  HC-13 (i18n)
  SC-7 (Embedding 相似度)
  SC-11 (响应式设计)
  SC-12 (状态中心)
  SC-18 (Worker 化)
  SC-19 (Skill 安全)
  SC-20 (配置恢复)
```

---

## P0 阶段：安全与稳定性基础（5 个硬约束）

### Step 1: MemoryManager 并发保护

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-5, DEP-3, R-4 |
| 目标文件 | `src/rust/mcp/tools/memory/manager.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

1. 将 `MemoryManager` 结构体包装为 `Arc<RwLock<MemoryManager>>`
2. 修改 `pub fn new()` 返回 `Arc<RwLock<Self>>`
3. 所有 `&mut self` 方法改为通过 `write()` 获取写锁：
   - `add_memory()`
   - `deduplicate()`
   - `update_config()`
   - `delete_memory()`
4. 所有 `&self` 方法改为通过 `read()` 获取读锁：
   - `get_all_memories()`
   - `get_memories_by_category()`
   - `search_memories()`
   - `get_compressed_memories()`
   - `config()`

**具体实现方案**：

```rust
// manager.rs — 新增包装类型
use std::sync::{Arc, RwLock};

pub struct SharedMemoryManager {
    inner: Arc<RwLock<MemoryManager>>,
}

impl SharedMemoryManager {
    pub fn new(project_path: &str) -> Result<Self> {
        let manager = MemoryManager::new(project_path)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(manager)),
        })
    }

    pub fn add_memory(&self, content: &str, category: MemoryCategory) -> Result<Option<String>> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.add_memory(content, category)
    }

    pub fn get_all_memories(&self) -> Result<Vec<MemoryEntry>> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.get_all_memories().into_iter().cloned().collect())
    }
    // ... 其余方法类似
}
```

**参考**：`src/rust/mcp/tools/acemcp/local_index.rs` 中的 `RwLock<HashMap>` 模式

### Step 2: MemoryManager 原子写入

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-5, R-4 |
| 目标文件 | `src/rust/mcp/tools/memory/manager.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

修改 `save_store()` 方法实现原子写入：

```rust
fn save_store(&self) -> Result<()> {
    let store_path = self.memory_dir.join(Self::STORE_FILE);
    let json = serde_json::to_string_pretty(&self.store)?;

    // 原子写入：先写临时文件，再 rename
    let tmp_path = store_path.with_extension("json.tmp");
    fs::write(&tmp_path, &json)?;
    fs::rename(&tmp_path, &store_path)?;
    Ok(())
}
```

### Step 3: MCP 工具调用层适配 SharedMemoryManager

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-5 |
| 目标文件 | `src/rust/mcp/tools/memory/mcp.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` + `cargo build` |

**变更内容**：

将 `MemoryTool::jiyi()` 中的 `MemoryManager::new()` 替换为 `SharedMemoryManager::new()`，调整所有 `&mut manager` 调用为 `manager.xxx()` 代理方法。

### Step 4: Tauri 命令层适配 SharedMemoryManager

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-5 |
| 目标文件 | `src/rust/mcp/commands.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

更新 `save_memory_config`、`get_memory_config`、`get_memory_stats`、`delete_memory` 等 Tauri 命令，使用 `SharedMemoryManager`。

### Step 5: 统一 MCP 错误分类体系

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-6, DEP-4, R-4 |
| 目标文件 | `src/rust/mcp/utils/errors.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

扩展 `McpToolError` 枚举，增加语义化错误分类：

```rust
#[derive(Debug, thiserror::Error)]
pub enum McpToolError {
    // 现有类型保留...

    // 新增网络错误分类
    #[error("网络超时: {0}")]
    NetworkTimeout(String),

    #[error("网络连接失败: {0}")]
    NetworkConnection(String),

    #[error("认证失败: {0}")]
    AuthenticationFailed(String),

    #[error("API 限流: {0}")]
    RateLimited(String),

    #[error("外部服务不可用: {0}")]
    ServiceUnavailable(String),

    #[error("参数验证失败: {0}")]
    ValidationError(String),
}

/// 错误是否可重试
impl McpToolError {
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            McpToolError::NetworkTimeout(_) |
            McpToolError::NetworkConnection(_) |
            McpToolError::RateLimited(_) |
            McpToolError::ServiceUnavailable(_)
        )
    }

    pub fn should_degrade(&self) -> bool {
        matches!(self,
            McpToolError::AuthenticationFailed(_) |
            McpToolError::ServiceUnavailable(_)
        )
    }
}
```

### Step 6: retry_request 适配统一错误分类

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-6, DEP-4 |
| 目标文件 | `src/rust/mcp/tools/acemcp/mcp.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

重构 `retry_request` 函数（第 569-606 行），用 `McpToolError` 替代字符串匹配：

```rust
async fn retry_request<F, Fut, T>(mut f: F, max_retries: usize, base_delay_secs: f64) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, McpToolError>>,
{
    let mut attempt = 0usize;
    while attempt < max_retries {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                attempt += 1;
                if attempt >= max_retries || !e.is_retryable() {
                    return Err(e.into());
                }
                let delay = base_delay_secs * 2f64.powi((attempt as i32) - 1);
                tokio::time::sleep(Duration::from_millis((delay * 1000.0) as u64)).await;
            }
        }
    }
    Err(anyhow::anyhow!("请求失败，已达最大重试次数"))
}
```

### Step 7: API 密钥安全存储

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 + 修改 |
| 关联约束 | HC-9, DEP-7, R-7 |
| 目标文件 | `src/rust/config/keyring.rs`（新增）, `src/rust/config/settings.rs`, `src/rust/config/mod.rs` |
| 验证方式 | `cargo build` + 手动测试密钥存取 |

**变更内容**：

1. 新增 `src/rust/config/keyring.rs`：

```rust
//! 安全密钥存储模块
//! 使用操作系统密钥管理（Windows Credential Manager / macOS Keychain）

use anyhow::Result;

const SERVICE_NAME: &str = "sanshu";

/// 存储密钥
pub fn store_secret(key: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)?;
    entry.set_password(value)?;
    Ok(())
}

/// 读取密钥
pub fn get_secret(key: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)?;
    match entry.get_password() {
        Ok(pwd) => Ok(Some(pwd)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 删除密钥
pub fn delete_secret(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)?;
    let _ = entry.delete_credential(); // 忽略不存在的情况
    Ok(())
}
```

2. 在 `Cargo.toml` 添加依赖：`keyring = "3"`
3. 修改 `settings.rs` 中 `McpConfig`：将 `acemcp_token`、`enhance_api_key`、`context7_api_key`、`acemcp_proxy_password` 字段标注为 `#[serde(skip)]`，改由 keyring 模块读写
4. 提供迁移逻辑：首次运行时检测 config.json 中是否有明文密钥，自动迁移到 keyring 并从 JSON 中移除

### Step 8: 记忆内容大小限制

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-10, R-9 |
| 目标文件 | `src/rust/mcp/tools/memory/manager.rs`, `src/rust/mcp/tools/memory/types.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

1. 在 `types.rs` 的 `MemoryConfig` 中新增：

```rust
pub struct MemoryConfig {
    // 现有字段...

    /// 单条记忆最大字节数（默认 10240 = 10KB）
    #[serde(default = "default_max_entry_bytes")]
    pub max_entry_bytes: usize,

    /// 最大记忆条目数（默认 1000）
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_max_entry_bytes() -> usize { 10240 }
fn default_max_entries() -> usize { 1000 }
```

2. 在 `manager.rs` 的 `add_memory()` 中添加检查：

```rust
pub fn add_memory(&mut self, content: &str, category: MemoryCategory) -> Result<Option<String>> {
    let content = content.trim();
    if content.is_empty() {
        return Err(anyhow::anyhow!("记忆内容不能为空"));
    }

    // 新增：大小限制检查
    if content.len() > self.store.config.max_entry_bytes {
        return Err(anyhow::anyhow!(
            "记忆内容超过大小限制: {} 字节 > {} 字节上限",
            content.len(), self.store.config.max_entry_bytes
        ));
    }

    // 新增：条目数量限制
    if self.store.entries.len() >= self.store.config.max_entries {
        return Err(anyhow::anyhow!(
            "记忆条目数已达上限: {} / {}",
            self.store.entries.len(), self.store.config.max_entries
        ));
    }

    // 后续逻辑不变...
}
```

### Step 9: Icon 缓存容量上限

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-15, R-9 |
| 目标文件 | `src/rust/mcp/tools/icon/api.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

为 `SEARCH_CACHE` 添加容量上限（LRU 淘汰）：

```rust
/// 最大缓存条目数
const MAX_CACHE_ENTRIES: usize = 200;

fn put_to_cache(key: String, result: IconSearchResult) {
    if let Ok(mut cache) = SEARCH_CACHE.write() {
        // 容量上限检查：超限时淘汰最旧的条目
        if cache.len() >= MAX_CACHE_ENTRIES {
            // 找到最旧的条目并移除
            if let Some(oldest_key) = cache.iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }
        cache.insert(key, CacheEntry {
            result,
            created_at: Instant::now(),
        });
    }
}
```

### Step 10: 索引目录磁盘空间限制

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-15 |
| 目标文件 | `src/rust/mcp/tools/acemcp/local_index.rs`（如存在 LocalIndexManager） |
| 验证方式 | `cargo build` |

**变更内容**：

在索引写入时检查目录大小，超过阈值（默认 500MB）时触发清理或拒绝索引新文件并记录警告日志。

```rust
/// 索引目录最大大小（字节），默认 500MB
const MAX_INDEX_DIR_SIZE: u64 = 500 * 1024 * 1024;

fn check_index_size_limit(index_dir: &Path) -> Result<()> {
    let total_size = fs_extra::dir::get_size(index_dir).unwrap_or(0);
    if total_size > MAX_INDEX_DIR_SIZE {
        log_important!(warn, "索引目录已超过大小限制: {}MB / {}MB",
            total_size / 1024 / 1024, MAX_INDEX_DIR_SIZE / 1024 / 1024);
        return Err(anyhow::anyhow!("索引目录大小超限"));
    }
    Ok(())
}
```

---

## P1 阶段：核心功能完善（6 个约束）

### Step 11: sou/enhance 搜索结果内存缓存

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | HC-7, R-1 |
| 目标文件 | `src/rust/mcp/tools/acemcp/cache.rs`（新增）, `src/rust/mcp/tools/acemcp/mcp.rs` |
| 验证方式 | `cargo test` + 手动验证缓存命中日志 |

**变更内容**：

1. 新建 `cache.rs`，参考 `icon/api.rs` 的缓存模式实现 LRU + TTL 缓存：

```rust
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

const DEFAULT_CACHE_TTL_SECS: u64 = 300; // 5 分钟
const MAX_CACHE_ENTRIES: usize = 100;

struct CacheEntry {
    result: String, // JSON 字符串
    created_at: Instant,
}

static SEARCH_CACHE: Lazy<RwLock<HashMap<String, CacheEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn get_cached(key: &str) -> Option<String> { /* TTL 检查 + 返回 */ }
pub fn put_cache(key: String, result: String) { /* LRU 淘汰 + 存入 */ }
pub fn clear_cache() { /* 清空 */ }
```

2. 在 `mcp.rs` 的搜索函数入口处，先查缓存，命中则直接返回

### Step 12: enhance 工具搜索结果缓存

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-7 |
| 目标文件 | `src/rust/mcp/tools/enhance/core.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

在 enhance 核心逻辑中复用 Step 11 的缓存模块，对相同 prompt + context 的增强结果进行缓存。

### Step 13: 后端核心模块测试骨架

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | HC-11, DEP-8, R-8 |
| 目标文件 | `src/rust/mcp/tools/enhance/core.rs`（添加 `#[cfg(test)]` 模块）, `src/rust/mcp/tools/acemcp/mcp.rs`（同上）, `src/rust/mcp/tools/context7/mcp.rs`（同上）, `src/rust/mcp/tools/interaction/mcp.rs`（同上）, `src/rust/mcp/tools/skills/mod.rs`（同上） |
| 验证方式 | `cargo test` |

**变更内容**：

为每个模块添加测试骨架，每模块至少包含：
- 1 个单元测试（核心函数 happy path）
- 1 个错误路径测试（参数无效/API 不可用）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_with_empty_prompt() {
        // 空 prompt 应返回错误
    }

    #[test]
    fn test_enhance_rule_engine_basic() {
        // 规则引擎基本匹配
    }
}
```

### Step 14: 记忆更新机制（Patch/Append）

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-5, HC-2, DEP-6 |
| 目标文件 | `src/rust/mcp/tools/memory/manager.rs`, `src/rust/mcp/tools/memory/mcp.rs`, `src/rust/mcp/tools/memory/types.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

1. 在 `manager.rs` 新增 `update_memory()` 方法：

```rust
/// 更新记忆内容
/// mode: "replace" | "append"
pub fn update_memory(&mut self, id: &str, content: &str, mode: &str) -> Result<()> {
    let entry = self.store.entries.iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| anyhow::anyhow!("记忆不存在: {}", id))?;

    match mode {
        "replace" => {
            entry.content = content.to_string();
            entry.content_normalized = TextSimilarity::normalize(content);
        }
        "append" => {
            entry.content.push_str("\n");
            entry.content.push_str(content);
            entry.content_normalized = TextSimilarity::normalize(&entry.content);
        }
        _ => return Err(anyhow::anyhow!("不支持的更新模式: {}", mode)),
    }

    entry.updated_at = Utc::now();
    self.save_store()
}
```

2. 在 `mcp.rs` 的 `jiyi()` 函数中新增 `"更新"` action 分支
3. MCP 工具参数中新增 `update_mode` 和 `memory_id` 可选字段

### Step 15: 数据迁移策略（JSON schema 版本校验）

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-15, R-9 |
| 目标文件 | `src/rust/mcp/tools/memory/types.rs`, `src/rust/mcp/tools/memory/manager.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

1. 在 `MemoryStore` 反序列化后校验 `version` 字段：

```rust
// manager.rs — new() 中加载 store 后
let store = if store_path.exists() {
    let content = fs::read_to_string(&store_path)?;
    let store: MemoryStore = serde_json::from_str(&content).unwrap_or_else(|e| {
        log_debug!("解析存储文件失败，备份并使用默认值: {}", e);
        // 备份损坏文件
        let backup_path = store_path.with_extension("json.bak");
        let _ = fs::copy(&store_path, &backup_path);
        MemoryStore { project_path: project_path_str.clone(), ..Default::default() }
    });

    // 版本校验
    if store.version != "2.0" {
        log_important!(warn, "存储版本不匹配: 期望 2.0, 实际 {}", store.version);
        // 未来升级时在此添加迁移逻辑
    }
    store
} else {
    MemoryStore { project_path: project_path_str.clone(), ..Default::default() }
};
```

### Step 16: IPC 弹性错误处理（SafeInvoke 包装器）

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-17, R-10 |
| 目标文件 | `src/frontend/composables/useSafeInvoke.ts`（新增） |
| 验证方式 | `pnpm typecheck`（如有） |

**变更内容**：

```typescript
// useSafeInvoke.ts
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

export function useSafeInvoke() {
  const error = ref<string | null>(null)
  const loading = ref(false)

  async function safeInvoke<T>(
    command: string,
    args?: Record<string, unknown>,
    options?: { timeout?: number }
  ): Promise<T | null> {
    const timeout = options?.timeout ?? 30000 // 默认 30 秒
    loading.value = true
    error.value = null

    try {
      const result = await Promise.race([
        invoke<T>(command, args),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error(`IPC 调用超时 (${timeout}ms): ${command}`)), timeout)
        ),
      ])
      return result
    }
    catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    }
    finally {
      loading.value = false
    }
  }

  return { safeInvoke, error, loading }
}
```

### Step 17: 记忆管理 UI — 后端 API 完善

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-8, HC-2, DEP-6 |
| 目标文件 | `src/rust/mcp/commands.rs`, `src/rust/app/commands.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

确保以下 Tauri 命令已注册并可用（部分已存在，需补全）：
- `list_memories` — 列出记忆（支持分页/过滤）
- `search_memories` — 搜索记忆
- `update_memory` — 更新记忆（新增，对接 Step 14）
- `delete_memory` — 删除记忆
- `get_memory_stats` — 获取统计信息
- `export_memories` — 导出记忆（JSON 格式）

### Step 18: 记忆管理 UI — 前端组件

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 + 新增 |
| 关联约束 | HC-8 |
| 目标文件 | `src/frontend/components/tools/MemoryConfig.vue`（扩展）, `src/frontend/components/tools/MemoryList.vue`（新增）, `src/frontend/components/tools/MemorySearch.vue`（新增） |
| 验证方式 | `pnpm dev` + 手动 UI 测试 |

**变更内容**：

1. `MemoryList.vue`：记忆列表组件，支持分页、分类过滤、删除、编辑
2. `MemorySearch.vue`：记忆搜索组件，支持关键词搜索、结果高亮
3. 扩展 `MemoryConfig.vue`：集成列表和搜索组件，添加导出功能

---

## P2 阶段：工程基础设施（8 个约束）

### Step 19: 前端测试框架搭建

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | HC-14, DEP-8, R-8 |
| 目标文件 | `package.json`, `vitest.config.ts`（新增）, `src/frontend/test/setup.ts`（新增） |
| 验证方式 | `pnpm test:unit` |

**变更内容**：

1. `pnpm add -D vitest @vue/test-utils jsdom`
2. 创建 `vitest.config.ts` 基础配置
3. 创建测试 setup 文件
4. 在 `package.json` 添加 `"test:unit": "vitest"` script
5. 为 `useSafeInvoke` 编写第一个单元测试

### Step 20: 记忆版本控制（快照）

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-6, HC-2 |
| 目标文件 | `src/rust/mcp/tools/memory/manager.rs`, `src/rust/mcp/tools/memory/types.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

1. 在 `MemoryEntry` 中新增 `version: u32` 字段（默认 1）
2. 新增 `MemorySnapshot` 结构体存储历史版本
3. `update_memory()` 执行前自动创建快照
4. 新增 `rollback_memory(id, version)` 方法

### Step 21: 磁盘级查询缓存

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-8 |
| 目标文件 | `src/rust/mcp/tools/acemcp/cache.rs`（扩展 Step 11） |
| 验证方式 | `cargo test` |

**变更内容**：

在内存缓存（Step 11）基础上，增加持久化到 `.sanshu-index/cache/` 目录的功能：
- 缓存命中时先查内存，未命中查磁盘，再未命中查 API
- 持久化格式：`{hash}.json`
- 磁盘缓存 TTL 默认 24 小时

### Step 22: 搜索结果 UI 预览

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-9 |
| 目标文件 | `src/frontend/components/tools/SearchPreview.vue`（新增） |
| 验证方式 | `pnpm dev` + 手动 UI 测试 |

**变更内容**：

搜索结果组件，包含：代码片段预览（带语法高亮）、关键词高亮、文件路径面包屑、相关度评分显示。

### Step 23: 搜索实时反馈

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-10 |
| 目标文件 | `src/frontend/composables/useSearchFeedback.ts`（新增） |
| 验证方式 | `pnpm dev` + 手动 UI 测试 |

**变更内容**：

搜索进度 composable：加载动画状态、已检索文件数、搜索阶段提示（"建立索引"/"语义检索"/"融合排序"）。

### Step 24: MCP Server 配置热更新

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-13 |
| 目标文件 | `src/rust/mcp/server.rs`, `src/rust/config/storage.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

1. 在 `ZhiServer` 中将 `enabled_tools` 改为 `Arc<RwLock<HashMap<String, bool>>>`
2. 启动文件监听器监控 `config.json` 变更
3. 变更时自动重新加载配置，无需重启 MCP 服务器
4. 移除 `is_tool_enabled()` 中每次读取文件的逻辑（当前已有此问题），改用缓存 + 监听模式

### Step 25: 结构化可观测性指标

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-14 |
| 目标文件 | `src/rust/mcp/metrics.rs`（新增）, `src/rust/mcp/server.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

```rust
// metrics.rs
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

pub struct McpMetrics {
    pub tool_calls: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub api_errors: AtomicU64,
    pub latency_samples: RwLock<Vec<u64>>, // 最近 1000 个延迟样本（ms）
}

impl McpMetrics {
    pub fn record_call(&self, tool: &str, latency_ms: u64) { /* ... */ }
    pub fn record_cache_hit(&self) { /* ... */ }
    pub fn record_api_error(&self, tool: &str) { /* ... */ }
    pub fn summary(&self) -> MetricsSummary { /* P50/P95/P99 延迟 + 命中率 */ }
}
```

### Step 26: 前端状态持久化（Pinia）

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | SC-16 |
| 目标文件 | `src/frontend/stores/searchStore.ts`（新增）, `package.json` |
| 验证方式 | `pnpm dev` |

**变更内容**：

1. `pnpm add pinia pinia-plugin-persistedstate`
2. 创建搜索状态 store（搜索历史、上次查询、偏好设置）
3. 配置持久化到 localStorage

---

## P3 阶段：国际化与增强功能（8 个约束）

### Step 27: 前端可访问性基线

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | HC-12 |
| 目标文件 | 所有新增前端组件（Step 18/22/23） |
| 验证方式 | `pnpm lint`（配置 eslint-plugin-vuejs-accessibility 后） |

**变更内容**：

1. `pnpm add -D eslint-plugin-vuejs-accessibility`
2. ESLint 配置中启用 a11y 规则
3. 为搜索输入框添加 `aria-label`
4. 搜索结果列表使用 `role="listbox"` + `aria-activedescendant`
5. 键盘导航：↑↓ 选择结果、Enter 确认、Esc 关闭

### Step 28: 国际化框架搭建

| 字段 | 值 |
|------|-----|
| 操作类型 | 新增 |
| 关联约束 | HC-13, DEP-9 |
| 目标文件 | `package.json`, `src/frontend/i18n/`（新增目录）, `src/frontend/i18n/index.ts`, `src/frontend/i18n/zh.ts`, `src/frontend/i18n/en.ts` |
| 验证方式 | `pnpm dev` + 切换语言测试 |

**变更内容**：

1. `pnpm add vue-i18n @intlify/unplugin-vue-i18n`
2. 创建 locale 文件（中/英），先覆盖记忆管理和搜索相关的 UI 文本
3. 在 `main.ts` 中注册 i18n 插件

### Step 29: Embedding 语义相似度检测

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-7 |
| 目标文件 | `src/rust/mcp/tools/memory/similarity.rs` |
| 验证方式 | `cargo test --package sanshu --lib mcp::tools::memory` |

**变更内容**：

新增 `embedding_similarity()` 方法作为 `calculate_enhanced()` 的可选补充。当本地 Embedding 服务（Ollama）可用时，结合语义相似度提升去重准确度。

### Step 30: 响应式设计适配

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-11 |
| 目标文件 | Step 18/22 中新增的组件 |
| 验证方式 | 手动测试不同窗口尺寸 |

**变更内容**：

使用 UnoCSS 的响应式断点确保组件在 400px~1920px 范围内可用。

### Step 31: Skill 执行安全性增强

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-19 |
| 目标文件 | `src/rust/mcp/tools/skills/mod.rs` |
| 验证方式 | `cargo build` |

**变更内容**：

1. 限制 Python 脚本 stdout 输出大小（默认 1MB）
2. 执行超时配置化（从 config.json 读取，默认 30 秒）

```rust
const MAX_STDOUT_BYTES: usize = 1_048_576; // 1MB

// 在获取输出后截断
let stdout = String::from_utf8_lossy(&output.stdout);
let truncated = if stdout.len() > MAX_STDOUT_BYTES {
    format!("{}...\n[输出已截断，超过 {}KB 限制]",
        &stdout[..MAX_STDOUT_BYTES], MAX_STDOUT_BYTES / 1024)
} else {
    stdout.to_string()
};
```

### Step 32: 配置文件损坏恢复

| 字段 | 值 |
|------|-----|
| 操作类型 | 修改 |
| 关联约束 | SC-20 |
| 目标文件 | `src/rust/config/storage.rs` |
| 验证方式 | `cargo test` |

**变更内容**：

修改 `load_standalone_config()`：反序列化失败时备份损坏文件并记录日志：

```rust
pub fn load_standalone_config() -> Result<AppConfig> {
    let config_path = get_standalone_config_path()?;
    if config_path.exists() {
        let config_json = fs::read_to_string(&config_path)?;
        match serde_json::from_str::<AppConfig>(&config_json) {
            Ok(mut config) => {
                merge_default_shortcuts(&mut config);
                merge_default_custom_prompts(&mut config);
                Ok(config)
            }
            Err(e) => {
                // 备份损坏文件
                let backup_path = config_path.with_extension("json.corrupted.bak");
                let _ = fs::copy(&config_path, &backup_path);
                log::warn!("配置文件损坏已备份到 {:?}，使用默认配置: {}", backup_path, e);
                Ok(AppConfig::default())
            }
        }
    } else {
        Ok(AppConfig::default())
    }
}
```

---

## 约束覆盖度矩阵

### 硬约束覆盖

| 约束 | 对应步骤 | 状态 |
|------|----------|------|
| HC-1 (存储架构) | 保持 JSON，Step 15 版本校验 | ✅ 覆盖 |
| HC-2 (记忆 CRUD) | Step 14 (更新), Step 17 (API) | ✅ 覆盖 |
| HC-3 (双模式检索) | 现有 hybrid_search.rs 已支持 | ✅ 已有 |
| HC-4 (MCP 规范) | Step 5/6 (错误分类) | ✅ 覆盖 |
| HC-5 (并发保护) | Step 1/2/3/4 | ✅ 覆盖 |
| HC-6 (统一错误分类) | Step 5/6 | ✅ 覆盖 |
| HC-7 (搜索缓存) | Step 11/12 | ✅ 覆盖 |
| HC-8 (记忆 UI) | Step 17/18 | ✅ 覆盖 |
| HC-9 (密钥安全) | Step 7 | ✅ 覆盖 |
| HC-10 (记忆大小限制) | Step 8 | ✅ 覆盖 |
| HC-11 (后端测试) | Step 13 | ✅ 覆盖 |
| HC-12 (A11y) | Step 27 | ✅ 覆盖 |
| HC-13 (i18n) | Step 28 | ✅ 覆盖 |
| HC-14 (前端测试) | Step 19 | ✅ 覆盖 |
| HC-15 (资源上限) | Step 9/10 | ✅ 覆盖 |

**硬约束覆盖率: 15/15 = 100%**

### 软约束覆盖

| 约束 | 对应步骤 | 状态 |
|------|----------|------|
| SC-1 (上下文扩展) | 未纳入（优先级低，依赖 AST） | ⏳ 延后 |
| SC-2 (Token 预算) | 未纳入（优先级低） | ⏳ 延后 |
| SC-3 (增量索引) | 已有实现 | ✅ 已有 |
| SC-4 (多语言 AST) | 未纳入（优先级低） | ⏳ 延后 |
| SC-5 (记忆更新) | Step 14 | ✅ 覆盖 |
| SC-6 (版本控制) | Step 20 | ✅ 覆盖 |
| SC-7 (Embedding 相似度) | Step 29 | ✅ 覆盖 |
| SC-8 (磁盘缓存) | Step 21 | ✅ 覆盖 |
| SC-9 (搜索 UI 预览) | Step 22 | ✅ 覆盖 |
| SC-10 (实时反馈) | Step 23 | ✅ 覆盖 |
| SC-11 (响应式设计) | Step 30 | ✅ 覆盖 |
| SC-12 (状态中心) | 未纳入（优先级低） | ⏳ 延后 |
| SC-13 (配置热更新) | Step 24 | ✅ 覆盖 |
| SC-14 (可观测性) | Step 25 | ✅ 覆盖 |
| SC-15 (数据迁移) | Step 15 | ✅ 覆盖 |
| SC-16 (前端状态持久化) | Step 26 | ✅ 覆盖 |
| SC-17 (IPC 弹性) | Step 16 | ✅ 覆盖 |
| SC-18 (Worker 化) | 未纳入（优先级低） | ⏳ 延后 |
| SC-19 (Skill 安全) | Step 31 | ✅ 覆盖 |
| SC-20 (配置恢复) | Step 32 | ✅ 覆盖 |

**软约束覆盖率: 16/20 = 80%**（4 个低优先级延后）

### 依赖关系覆盖

| 依赖 | 满足方式 |
|------|----------|
| DEP-1 (外部 API) | Step 11/12 缓存减少 API 依赖 |
| DEP-2 (存储兼容) | Step 15 版本校验 |
| DEP-3 (并发→多客户端) | Step 1-4 先于多客户端 |
| DEP-4 (错误→降级) | Step 5 先于 Step 6 |
| DEP-5 (FTS5→混合检索) | 保持现有 BM25 实现 |
| DEP-6 (CRUD→UI) | Step 14/17 先于 Step 18 |
| DEP-7 (密钥→发布) | Step 7 |
| DEP-8 (前端测试→前端开发) | Step 19 先于 Step 18 |
| DEP-9 (i18n→国际化) | Step 28 |

**依赖覆盖率: 9/9 = 100%**

### 风险缓解覆盖

| 风险 | 缓解步骤 |
|------|----------|
| R-1 (性能) | Step 9/10/11/12 |
| R-2 (数据一致性) | Step 1/2 |
| R-3 (用户体验) | Step 16/18/22/23 |
| R-4 (并发竞争) | Step 1/2/3/4 |
| R-5 (索引重建) | 保持现有增量索引 |
| R-6 (前端复杂度) | Step 18 组件化设计 |
| R-7 (密钥泄露) | Step 7 |
| R-8 (测试不足) | Step 13/19 |
| R-9 (memories 膨胀) | Step 8/15 |
| R-10 (IPC 超时) | Step 16 |

**风险缓解率: 10/10 = 100%**

---

## 开放问题决策记录

以下问题已在计划中做出默认决策：

| 问题 | 决策 | 理由 |
|------|------|------|
| Q8: 密钥存储方案 | Windows Credential Manager / macOS Keychain (`keyring` crate) | 跨平台标准方案，零外部依赖 |
| Q9: 测试补全优先级 | 先后端（Step 13）再前端（Step 19） | 后端是 MCP 核心，优先保障 |
| Q10: JSON→SQLite 迁移 | 暂不迁移，仅加版本校验（Step 15） | 当前 JSON 满足需求，避免过早优化 |
| Q11: i18n 范围 | 仅 UI 文本（Step 28） | 日志和错误消息保持中文 |
| Q12: memories 条目上限 | 1000 条（Step 8） | 研究报告建议值，可通过配置调整 |

---

## 执行注意事项

1. **P0 阶段必须完整完成后才能进入 P1**，因为 P1 的记忆 UI（HC-8）依赖 P0 的并发保护（HC-5）
2. P1 内部 Step 14 必须先于 Step 17/18（记忆更新 API 先于 UI）
3. P2 内部 Step 19（前端测试框架）应先于其他前端步骤
4. 每个 Step 完成后运行对应验证命令确认无回归
5. keyring crate（Step 7）需要验证 Windows/macOS/Linux 兼容性

---

## 后续可选命令

- `/ccg:spec-impl` — 按本计划执行实施
- `/ccg:spec-review` — 实施后合规审查
