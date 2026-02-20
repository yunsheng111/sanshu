# FTS5 Actor 接口契约文档

**任务**: T0 - Interface Contract Freeze
**状态**: 已冻结 (Frozen)
**创建时间**: 2026-02-20
**约束来源**: HC-1, HC-2, HC-3, HC-5, HC-6, HC-7, HC-9

---

## 1. 接口契约概述

本文档定义 FTS5 Actor 的完整接口契约，包括消息类型、数据结构和返回类型。**接口一旦冻结，后续任务（T1-T4）不得修改**，仅允许实现细节调整。

---

## 2. 核心消息枚举 (`FtsMessage`)

### 2.1 消息定义

```rust
/// FTS5 Actor 消息类型
#[derive(Debug)]
pub enum FtsMessage {
    /// 同步单条记忆到 FTS5 索引
    ///
    /// 参数:
    /// - `MemoryEntry`: 要同步的记忆条目
    Sync(MemoryEntry),

    /// 删除单条记忆的 FTS5 索引
    ///
    /// 参数:
    /// - `String`: 记忆 ID
    Delete(String),

    /// FTS5 全文搜索
    ///
    /// 参数:
    /// - `SearchRequest`: 搜索请求
    /// - `oneshot::Sender<Result<Vec<MemorySearchResult>>>`: 结果返回通道
    Search(SearchRequest, oneshot::Sender<Result<Vec<MemorySearchResult>>>),

    /// 批量同步所有记忆到 FTS5 索引
    ///
    /// 参数:
    /// - `Vec<MemoryEntry>`: 所有记忆条目
    /// - `oneshot::Sender<Result<SyncResult>>`: 结果返回通道
    SyncAll(Vec<MemoryEntry>, oneshot::Sender<Result<SyncResult>>),

    /// 关闭 Actor（优雅退出）
    Shutdown,
}
```

### 2.2 消息类型说明

| 消息类型 | 返回方式 | 阻塞性 | 约束来源 |
|----------|----------|--------|----------|
| `Sync` | Fire-and-forget | 非阻塞 | HC-3, HC-4 |
| `Delete` | Fire-and-forget | 非阻塞 | HC-3, HC-4 |
| `Search` | `oneshot::Sender` | 阻塞等待 | HC-7 |
| `SyncAll` | `oneshot::Sender` | 阻塞等待 | HC-8 |
| `Shutdown` | 无返回 | 非阻塞 | HC-6 |

---

## 3. 搜索请求结构 (`SearchRequest`)

### 3.1 结构定义

```rust
/// FTS5 搜索请求
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// 搜索查询字符串（FTS5 查询语法）
    pub query: String,

    /// 返回结果数量限制
    pub limit: usize,
}
```

### 3.2 字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `query` | `String` | 是 | - | FTS5 查询字符串，支持 `AND`/`OR`/`NOT` 等操作符 |
| `limit` | `usize` | 是 | - | 最大返回结果数，建议范围 10-100 |

### 3.3 查询语法示例

```rust
// 简单关键词搜索
SearchRequest { query: "rust".to_string(), limit: 20 }

// 多关键词 AND 搜索
SearchRequest { query: "rust AND tokio".to_string(), limit: 20 }

// 短语搜索
SearchRequest { query: "\"async actor\"".to_string(), limit: 20 }

// 排除关键词
SearchRequest { query: "rust NOT python".to_string(), limit: 20 }
```

---

## 4. 搜索响应结构 (`SearchResponse`)

### 4.1 结构定义

```rust
/// FTS5 搜索响应（通过 oneshot::Sender 返回）
pub type SearchResponse = Result<Vec<MemorySearchResult>>;
```

### 4.2 成功响应 (`MemorySearchResult`)

```rust
/// 记忆搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// 记忆 ID
    pub id: String,

    /// 记忆内容
    pub content: String,

    /// 记忆分类
    pub category: MemoryCategory,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 更新时间
    pub updated_at: DateTime<Utc>,

    /// 搜索模式（"fts5" 或 "fuzzy"）
    pub search_mode: String,

    /// 高亮片段（可选）
    pub highlighted_snippet: Option<String>,

    /// URI 路径（可选）
    pub uri_path: Option<String>,

    /// 域名（可选）
    pub domain: Option<String>,

    /// 标签（可选）
    pub tags: Option<Vec<String>>,

    /// 摘要（可选）
    pub summary: Option<String>,
}
```

### 4.3 错误响应

```rust
/// 搜索失败时返回的错误类型
pub type SearchError = anyhow::Error;

// 常见错误场景：
// - FTS5 索引损坏
// - SQLite 连接失败
// - 查询语法错误
// - 超时（由调用方通过 tokio::time::timeout 控制）
```

---

## 5. 批量同步结果 (`SyncResult`)

### 5.1 结构定义

```rust
/// 批量同步结果
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// 成功同步的条目数
    pub synced: usize,

    /// 同步失败的条目数
    pub failed: usize,
}
```

### 5.2 字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `synced` | `usize` | 成功写入 FTS5 索引的记忆条目数 |
| `failed` | `usize` | 同步失败的记忆条目数（单条失败不阻塞其他条目，HC-4） |

---

## 6. 返回类型规范

### 6.1 `oneshot::Sender` 返回类型

```rust
// 搜索操作返回类型
oneshot::Sender<Result<Vec<MemorySearchResult>>>

// 批量同步操作返回类型
oneshot::Sender<Result<SyncResult>>
```

### 6.2 使用示例

```rust
// 发送端（MemoryManager）
let (tx, rx) = oneshot::channel();
fts_tx.send(FtsMessage::Search(request, tx))?;

// 接收端（调用方）
let results = tokio::time::timeout(
    Duration::from_secs(5),
    rx
).await??;

// Actor 内部（fts_actor.rs）
match msg {
    FtsMessage::Search(request, response_tx) => {
        let results = fts_index.search(&request.query, request.limit)?;
        let _ = response_tx.send(Ok(results));
    }
}
```

---

## 7. 约束验证清单

### 7.1 硬约束验证

| 约束编号 | 约束描述 | 接口体现 | 验证状态 |
|----------|----------|----------|----------|
| HC-1 | `rusqlite::Connection` 不可跨线程传递 | `FtsIndex` 被 move 进 Actor 任务 | ✅ 满足 |
| HC-2 | `SharedMemoryManager` 不直接持有 `FtsIndex` | 仅持有 `mpsc::UnboundedSender<FtsMessage>` | ✅ 满足 |
| HC-3 | 所有 FTS5 操作通过消息通道执行 | `FtsMessage` 枚举定义所有操作 | ✅ 满足 |
| HC-5 | 使用 `tokio::sync::mpsc` 通道 | `mpsc::UnboundedSender<FtsMessage>` | ✅ 满足 |
| HC-6 | Actor 在独立 `tokio::spawn` 任务中运行 | `FtsIndex` 被 move 进 spawn 任务 | ✅ 满足 |
| HC-7 | 搜索操作使用 `oneshot` 返回结果 | `Search` 消息携带 `oneshot::Sender` | ✅ 满足 |
| HC-9 | `MemoryManager` 添加 `fts_tx: Option<mpsc::UnboundedSender<FtsMessage>>` | 接口定义明确 `Option` 类型 | ✅ 满足 |

### 7.2 软约束验证

| 约束编号 | 约束描述 | 接口体现 | 验证状态 |
|----------|----------|----------|----------|
| SC-3 | 搜索结果附带 `search_mode` 字段 | `MemorySearchResult.search_mode` | ✅ 满足 |
| SC-13 | 搜索结果包含高亮片段 | `MemorySearchResult.highlighted_snippet` | ✅ 满足 |

---

## 8. 编译验证

### 8.1 类型依赖

```rust
// 必需的类型导入
use tokio::sync::{mpsc, oneshot};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// 项目内部类型
use super::types::{MemoryEntry, MemoryCategory};
use super::fts_index::{FtsIndex, SyncResult};
```

### 8.2 编译检查点

- ✅ `FtsMessage` 枚举所有变体可构造
- ✅ `SearchRequest` 结构体字段完整
- ✅ `MemorySearchResult` 结构体可序列化
- ✅ `oneshot::Sender<Result<T>>` 类型正确
- ✅ `mpsc::UnboundedSender<FtsMessage>` 实现 `Send + Sync`

---

## 9. 接口冻结声明

**本接口契约自 2026-02-20 起正式冻结，后续任务（T1-T4）不得修改以下内容：**

1. `FtsMessage` 枚举的变体定义
2. `SearchRequest` 结构体的字段
3. `MemorySearchResult` 结构体的字段
4. `oneshot::Sender<Result<Vec<MemorySearchResult>>>` 返回类型
5. `mpsc::UnboundedSender<FtsMessage>` 通道类型

**允许的调整：**
- Actor 内部实现细节（消息处理逻辑）
- 错误处理策略（日志记录、降级逻辑）
- 性能优化（批处理、缓存）

---

## 10. 验收标准

### 10.1 编译通过 (OK-1)

```bash
# 验证命令
cargo build --package sanshu --lib

# 预期结果
# - 无编译错误
# - 无类型不匹配警告
# - `FtsMessage` 枚举可正常使用
```

### 10.2 类型检查

```rust
// 验证 FtsMessage 可构造
let sync_msg = FtsMessage::Sync(entry);
let delete_msg = FtsMessage::Delete("id".to_string());
let (tx, rx) = oneshot::channel();
let search_msg = FtsMessage::Search(request, tx);

// 验证 Sender 类型正确
let fts_tx: mpsc::UnboundedSender<FtsMessage> = ...;
fts_tx.send(sync_msg).unwrap();
```

---

## 11. 参考实现

### 11.1 AsyncObservationWriter 模式

本接口契约参考 `observation_store.rs:202-237` 的 Actor 模式：

```rust
// 参考模式（已验证可行）
pub struct AsyncObservationWriter {
    tx: mpsc::UnboundedSender<ObservationMessage>,
}

impl AsyncObservationWriter {
    pub fn new(store: ObservationStore) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ObservationMessage::Record(obs) => {
                        if let Err(e) = store.record(&obs) {
                            log_debug!("异步写入失败: {}", e);
                        }
                    }
                }
            }
        });

        Self { tx }
    }
}
```

### 11.2 FTS5 Actor 适配

```rust
// FTS5 Actor 将采用相同模式
pub fn spawn_fts_actor(fts_index: FtsIndex) -> mpsc::UnboundedSender<FtsMessage> {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                FtsMessage::Sync(entry) => { /* ... */ }
                FtsMessage::Delete(id) => { /* ... */ }
                FtsMessage::Search(req, resp_tx) => { /* ... */ }
                FtsMessage::SyncAll(entries, resp_tx) => { /* ... */ }
                FtsMessage::Shutdown => break,
            }
        }
    });

    tx
}
```

---

## 12. 后续任务依赖

| 任务 | 依赖接口 | 使用方式 |
|------|----------|----------|
| T1 (双写集成) | `FtsMessage::Sync`, `FtsMessage::Delete` | `MemoryManager` 在 CRUD 操作中发送消息 |
| T2 (搜索路由) | `FtsMessage::Search`, `SearchRequest`, `SearchResponse` | `commands.rs` 调用 FTS5 搜索并降级 |
| T3 (一致性校验) | `FtsMessage::SyncAll`, `SyncResult` | 启动时校验并重建索引 |
| T4 (前端集成) | `MemorySearchResult.search_mode` | 前端显示搜索模式指示器 |

---

**接口契约版本**: v1.0
**冻结日期**: 2026-02-20
**审查状态**: 待 team-lead 确认
