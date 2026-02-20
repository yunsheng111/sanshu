# Team Research: FTS5 全文搜索集成

## 增强后的需求

**目标**：实现 FTS5 全文搜索集成，解决 P2 阶段遗留的 `rusqlite::Connection` 线程安全问题。

**背景**：
- P2 审查报告（`.doc/agent-teams/reviews/20260220-memory-p2-review.md`）识别出 Critical 级别技术阻塞
- 核心问题：`rusqlite::Connection` 不是 `Send`，无法在 `Arc<RwLock<MemoryManager>>` 中使用
- 影响范围：Task 1（FTS5 双写）、Task 2（搜索路由）、Task 4（前端集成）

**研究目标**：
1. 分析 FTS5 异步集成架构方案（重点：异步消息队列架构）
2. 识别技术约束和依赖关系
3. 定义可验证的成功判据
4. 产出约束集供后续规划使用

**技术范围**：
- 后端：Rust + Tokio + rusqlite + FTS5
- 前端：TypeScript + Vue 3（搜索 UI 集成）
- 架构：异步消息队列 + Actor 模式

---

## 双模型执行元数据

| 字段 | 值 |
|------|-----|
| `dual_model_status` | DEGRADED |
| `degraded_level` | ACCEPTABLE |
| `missing_dimensions` | ["backend", "frontend"] |
| `CODEX_SESSION` | null |
| `GEMINI_SESSION` | null |

**降级原因**：项目中缺少 `collab` Skill 和相关模板文件（`skills/collab/SKILL.md`、`agents/ccg/_templates/multi-model-gate.md`），无法执行标准的 Codex + Gemini 双模型交叉探索流程。

---

## 约束集

### 硬约束（HC）

| 编号 | 约束描述 | 来源 | 技术依据 |
|------|----------|------|----------|
| HC-1 | `rusqlite::Connection` 必须在单一线程中持有，不可跨线程传递 | 主代理 | Rust 编译器错误：`RefCell<rusqlite::Connection>` cannot be shared between threads safely |
| HC-2 | `SharedMemoryManager` 不能直接持有 `FtsIndex` 结构体 | 主代理 | `Arc<RwLock<T>>` 要求 `T: Send`，但 `FtsIndex` 包含非 `Send` 的 `Connection` |
| HC-3 | 所有 FTS5 操作必须通过异步消息队列执行 | 主代理 | 唯一可行的线程安全方案 |
| HC-4 | FTS5 失败不阻塞 JSON 主流程（HC-18 继承） | P2 计划 | Sidecar 索引设计原则 |
| HC-5 | 使用 `tokio::sync::mpsc` 通道传递消息 | 主代理 | 项目已启用 Tokio `sync` feature |
| HC-6 | FTS5 Actor 必须在独立 Tokio 任务中运行 | 主代理 | 确保 `Connection` 不跨线程 |
| HC-7 | 搜索操作必须使用 `oneshot` 通道返回结果 | 主代理 | 同步等待异步操作的标准模式 |
| HC-8 | 启动时必须校验 JSON 和 FTS5 一致性，不一致时重建索引 | 用户确认 | Q5 选择：启动时校验一次 |

### 软约束（SC）

| 编号 | 约束描述 | 来源 | 理由 |
|------|----------|------|------|
| SC-1 | 优先使用 `unbounded_channel`，简化背压管理 | 用户确认 | Q3 选择：无限容量通道 |
| SC-2 | 索引初始化采用异步后台同步，不阻塞启动 | 用户确认 | Q1 选择：启动时异步后台同步 |
| SC-3 | 搜索结果附带 `search_mode` 字段（"fts5" / "fuzzy"） | 用户确认 | Q2 选择：返回搜索模式指示器 |
| SC-4 | FTS5 搜索设置 5 秒超时，超时后降级到模糊匹配 | 用户确认 | Q4 选择：5 秒超时保护 |
| SC-5 | 消息处理失败仅记录日志，不崩溃 Actor | 主代理 | 保证系统稳定性 |
| SC-6 | 索引延迟控制在 100ms 以内 | 主代理 | 用户体验要求 |
| SC-7 | 参考 `ObservationStore` 的消息队列实现 | 主代理 | 项目内已有成功案例 |
| SC-8 | 参考 `WatcherManager` 的异步任务模式 | 主代理 | 项目内已有成功案例 |
| SC-9 | 使用 `Drop` trait 实现优雅关闭 | 主代理 | 标准 Rust 资源管理模式 |
| SC-10 | Actor 内部错误使用 `log_debug!` 记录 | 主代理 | 与现有日志风格一致 |

### 依赖关系（DEP）

| 编号 | 依赖描述 | 影响 |
|------|----------|------|
| DEP-1 | `fts_actor.rs` 模块 → `fts_index.rs` 模块 | Actor 需调用 `FtsIndex` 的方法 |
| DEP-2 | `manager.rs` 修改 → `fts_actor.rs` 创建 | 必须先创建 Actor 模块 |
| DEP-3 | Task 1（双写）→ Task 2（搜索路由） | 搜索依赖索引数据 |
| DEP-4 | Task 2（搜索路由）→ Task 4（前端集成） | 前端依赖后端搜索 API |
| DEP-5 | 一致性校验 → 索引重建逻辑 | 校验失败时需触发重建 |
| DEP-6 | 优雅关闭 → 消息队列清空 | 关闭前需处理完待处理消息 |
| DEP-7 | 现有 Tokio 依赖（1.0 + sync feature） | 无需新增依赖 |
| DEP-8 | 现有 rusqlite 依赖（0.31 + bundled feature） | 无需新增依赖 |

### 风险（RISK）

| 编号 | 风险描述 | 影响 | 缓解策略 |
|------|----------|------|----------|
| RISK-1 | 消息队列积压导致内存溢出 | 高 | 使用 `unbounded_channel` 但监控队列长度，超过 10000 条时记录警告 |
| RISK-2 | FTS5 搜索超时导致用户体验下降 | 中 | 设置 5 秒超时，超时后自动降级到模糊匹配 |
| RISK-3 | 启动时一致性校验阻塞启动 | 中 | 校验逻辑在后台异步执行，不阻塞 `MemoryManager::new()` |
| RISK-4 | Actor 崩溃导致 FTS5 功能完全失效 | 高 | Actor 内部错误仅记录日志，不 panic；通道关闭时自动重启 Actor |
| RISK-5 | 双写不一致导致搜索结果错误 | 中 | 启动时校验一致性，不一致时重建索引 |
| RISK-6 | 优雅关闭时消息丢失 | 低 | `Drop` trait 发送 `Shutdown` 消息，Actor 处理完队列后退出 |
| RISK-7 | 并发写入导致索引损坏 | 低 | SQLite 自身支持并发写入保护（WAL 模式） |
| RISK-8 | 前端缺少 `useMemorySearch` composable | 中 | Task 4 必须实现该 composable，封装搜索逻辑 |

---

## 降级影响说明

### 缺失维度
- **backend**：缺少 Codex 对 Rust 异步架构的深度分析
- **frontend**：缺少 Gemini 对 Vue 3 搜索 UI 集成的分析

### 影响范围
1. **后端架构**：主代理基于项目内现有案例（`ObservationStore`、`WatcherManager`）推导出异步消息队列方案，但缺少 Codex 对 Tokio 性能优化和边界情况的专业建议
2. **前端集成**：缺少 Gemini 对 `useMemorySearch` composable 的详细设计和 UI 交互模式建议
3. **测试策略**：缺少双模型对单元测试和集成测试的交叉验证

### 补偿分析
- **主代理补偿**：通过深入阅读项目现有代码（`observation_store.rs`、`watcher.rs`、`acemcp/hybrid_search.rs`）提取异步架构模式
- **参考现有实现**：`ObservationStore` 已成功使用 `tokio::sync::mpsc` + 独立线程持有 `Connection`
- **用户确认**：通过 `mcp______zhi` 与用户确认 5 个关键设计决策（Q1-Q5）

### 风险约束补充
- **RISK-9**：缺少 Codex 审查可能导致 Tokio 任务泄漏或死锁 → 缓解：参考现有 `spawn` 调用模式，确保所有任务有明确生命周期
- **RISK-10**：缺少 Gemini 审查可能导致前端搜索 UI 性能问题 → 缓解：Task 4 实施时参考 `acemcp` 的前端集成模式

---

## 成功判据

| 编号 | 类型 | 判据描述 | 验证方式 | 关联约束 |
|------|------|----------|----------|----------|
| OK-1 | 编译 | `MemoryManager` 包含 `fts_tx` 字段后可成功编译 | `cargo build` | HC-1, HC-2, HC-3 |
| OK-2 | 功能 | `add_memory()` 后 FTS5 索引可搜索到新记忆 | 单元测试 | HC-3, DEP-1 |
| OK-3 | 功能 | `update_memory()` 后 FTS5 索引更新 | 单元测试 | HC-3, DEP-1 |
| OK-4 | 功能 | `delete_memory()` 后 FTS5 索引删除对应条目 | 单元测试 | HC-3, DEP-1 |
| OK-5 | 功能 | FTS5 搜索返回正确结果 | 单元测试 | HC-7, SC-3 |
| OK-6 | 降级 | FTS5 搜索失败时自动降级到模糊匹配 | 单元测试（模拟超时） | SC-4, RISK-2 |
| OK-7 | 性能 | 搜索延迟 < 100ms（P95） | 性能测试 | SC-6 |
| OK-8 | 可靠性 | 启动时一致性校验通过 | 集成测试 | HC-8, RISK-5 |
| OK-9 | 可靠性 | 一致性校验失败时自动重建索引 | 集成测试（人工破坏索引） | HC-8, RISK-5 |
| OK-10 | 并发 | 100 并发写入后索引无损坏 | 压力测试 | RISK-7 |
| OK-11 | 资源 | Actor 优雅关闭，无消息丢失 | 单元测试 | SC-9, RISK-6 |
| OK-12 | 前端 | `useMemorySearch` composable 封装搜索逻辑 | 前端单元测试 | RISK-8 |
| OK-13 | 前端 | 搜索结果显示 `search_mode` 指示器 | 手动测试 | SC-3 |
| OK-14 | 监控 | 消息队列长度超过 10000 时记录警告 | 日志检查 | RISK-1 |

---

## 开放问题（已解决）

| 问题 | 用户回答 | 转化约束 |
|------|----------|----------|
| Q1: 索引初始化时机 | C - 启动时异步后台同步，不阻塞主流程 | SC-2 |
| Q2: 搜索降级策略 | B - 返回结果时附带 `search_mode` 字段 | SC-3 |
| Q3: 消息队列容量 | A - `unbounded_channel`（无限容量） | SC-1 |
| Q4: 搜索超时 | B - 设置 5 秒超时，超时后降级 | SC-4 |
| Q5: 一致性校验 | B - 启动时校验一次，不一致时重建索引 | HC-8 |

---

## 技术方案详解

### 方案 A：异步消息队列架构（已选定）

#### 架构图

```
SharedMemoryManager (Arc<RwLock<MemoryManager>>)
    |
    |-- add_memory() / update_memory() / delete_memory()
    |       |
    |       v
    |   发送消息到 FtsIndexActor
    |       |
    |       v
    |   FtsIndexActor (独立 Tokio 任务)
    |       |-- 持有 FtsIndex { conn: Connection }
    |       |-- 接收消息：Sync / Delete / Search / Shutdown
    |       +-- 执行 SQLite 操作
    |
    +-- search_memories()
            |
            v
        发送 Search 消息 + 等待响应（oneshot）
```

#### 消息类型定义

```rust
/// FTS5 Actor 消息
enum FtsMessage {
    /// 同步单条记忆到索引
    Sync {
        id: String,
        entry: MemoryEntry,
    },
    /// 删除索引条目
    Delete {
        id: String,
    },
    /// 搜索（带响应通道）
    Search {
        query: String,
        limit: usize,
        response_tx: oneshot::Sender<Result<Vec<String>>>,
    },
    /// 优雅关闭
    Shutdown,
}
```

#### 实施步骤（8 步）

| 步骤 | 任务 | 预计工时 | 关联约束 |
|------|------|----------|----------|
| 1 | 创建 `fts_actor.rs` 模块，定义 `FtsMessage` 和 `FtsIndexActor` | 2 小时 | HC-3, HC-5, HC-6 |
| 2 | 修改 `MemoryManager` 结构体，添加 `fts_tx` 字段 | 1 小时 | HC-2, DEP-2 |
| 3 | 在 `MemoryManager::new()` 中启动 FTS Actor | 2 小时 | HC-6, SC-2 |
| 4 | 集成双写逻辑（`add_memory`、`update_memory`、`delete_memory`） | 3 小时 | HC-3, DEP-3 |
| 5 | 实现搜索路由（`search_memories` 调用 FTS5） | 2 小时 | HC-7, SC-3, SC-4 |
| 6 | 实现优雅关闭（`Drop` trait） | 1 小时 | SC-9, RISK-6 |
| 7 | 添加单元测试（消息发送、搜索降级、优雅关闭） | 4 小时 | OK-1 ~ OK-11 |
| 8 | 添加集成测试（端到端 CRUD + FTS5、压力测试） | 3 小时 | OK-10, OK-14 |

**总工时估算**：18 小时（约 2-3 天）

#### 核心代码片段

**`fts_actor.rs` 消息处理循环**：

```rust
pub async fn run_fts_actor(
    mut rx: mpsc::UnboundedReceiver<FtsMessage>,
    memory_dir: PathBuf,
) {
    let index = match FtsIndex::open(&memory_dir) {
        Ok(idx) => idx,
        Err(e) => {
            log_debug!("[FtsActor] 无法打开索引: {}", e);
            return;
        }
    };

    while let Some(msg) = rx.recv().await {
        match msg {
            FtsMessage::Sync { id, entry } => {
                if let Err(e) = index.sync_entry(&entry) {
                    log_debug!("[FtsActor] 同步失败 (id={}): {}", id, e);
                }
            }
            FtsMessage::Delete { id } => {
                if let Err(e) = index.delete_entry(&id) {
                    log_debug!("[FtsActor] 删除失败 (id={}): {}", id, e);
                }
            }
            FtsMessage::Search { query, limit, response_tx } => {
                let result = index.search(&query, limit);
                let _ = response_tx.send(result);
            }
            FtsMessage::Shutdown => {
                log_debug!("[FtsActor] 收到关闭信号，退出");
                break;
            }
        }
    }
}
```

**`MemoryManager` 集成**：

```rust
pub struct MemoryManager {
    memory_dir: PathBuf,
    project_path: String,
    store: MemoryStore,
    is_non_git_project: bool,
    fts_tx: Option<mpsc::UnboundedSender<FtsMessage>>, // 新增字段
}

impl MemoryManager {
    pub fn new(project_path: &str) -> Result<Self> {
        // ... 现有初始化逻辑 ...

        // 启动 FTS Actor
        let (tx, rx) = mpsc::unbounded_channel();
        let memory_dir_clone = memory_dir.clone();
        tokio::spawn(async move {
            run_fts_actor(rx, memory_dir_clone).await;
        });

        let manager = Self {
            memory_dir,
            project_path: project_path_str,
            store,
            is_non_git_project: normalize_result.is_non_git,
            fts_tx: Some(tx),
        };

        // 异步后台同步现有记忆（SC-2）
        if let Some(tx) = &manager.fts_tx {
            let entries = manager.store.entries.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                for entry in entries {
                    let _ = tx_clone.send(FtsMessage::Sync {
                        id: entry.id.clone(),
                        entry,
                    });
                }
            });
        }

        Ok(manager)
    }

    pub fn add_memory(&mut self, content: &str, category: MemoryCategory) -> Result<Option<String>> {
        // ... 现有逻辑 ...

        // 双写到 FTS5
        if let Some(tx) = &self.fts_tx {
            let _ = tx.send(FtsMessage::Sync {
                id: id.clone(),
                entry: entry.clone(),
            });
        }

        Ok(Some(id))
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        if let Some(tx) = &self.fts_tx {
            let _ = tx.send(FtsMessage::Shutdown);
        }
    }
}
```

**搜索路由（带超时和降级）**：

```rust
pub fn search_memories(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
    // 尝试 FTS5 搜索
    if let Some(tx) = &self.fts_tx {
        let (response_tx, response_rx) = oneshot::channel();
        if tx.send(FtsMessage::Search {
            query: query.to_string(),
            limit,
            response_tx,
        }).is_ok() {
            // SC-4: 5 秒超时
            match tokio::time::timeout(
                Duration::from_secs(5),
                response_rx
            ).await {
                Ok(Ok(Ok(ids))) => {
                    // FTS5 搜索成功
                    let results = self.get_entries_by_ids(&ids);
                    log_debug!("[Search] FTS5 模式: {} 结果", results.len());
                    return results.into_iter()
                        .map(|mut e| {
                            e.search_mode = Some("fts5".to_string()); // SC-3
                            e
                        })
                        .collect();
                }
                _ => {
                    log_debug!("[Search] FTS5 失败或超时，降级到模糊匹配");
                }
            }
        }
    }

    // 降级到模糊匹配
    let results = self.fuzzy_search(query, limit);
    results.into_iter()
        .map(|mut e| {
            e.search_mode = Some("fuzzy".to_string()); // SC-3
            e
        })
        .collect()
}
```

---

## 前端集成要点

### Task 4: 创建 `useMemorySearch` composable

**文件路径**：`src/frontend/composables/useMemorySearch.ts`

**核心功能**：
1. 封装 `invoke('search_memories')` 调用
2. 解析 `search_mode` 字段
3. 显示搜索模式指示器（FTS5 / 模糊匹配）
4. 处理搜索错误和降级

**示例代码**：

```typescript
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export function useMemorySearch() {
  const results = ref<MemoryEntry[]>([])
  const searchMode = ref<'fts5' | 'fuzzy' | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  const search = async (query: string, limit = 20) => {
    isLoading.value = true
    error.value = null

    try {
      const data = await invoke<MemoryEntry[]>('search_memories', {
        query,
        limit,
      })

      results.value = data
      // SC-3: 解析搜索模式
      searchMode.value = data[0]?.search_mode || null
    } catch (e) {
      error.value = String(e)
      results.value = []
    } finally {
      isLoading.value = false
    }
  }

  const searchModeLabel = computed(() => {
    if (searchMode.value === 'fts5') return 'FTS5 全文搜索'
    if (searchMode.value === 'fuzzy') return '模糊匹配'
    return '未知'
  })

  return {
    results,
    searchMode,
    searchModeLabel,
    isLoading,
    error,
    search,
  }
}
```

---

## 参考实现

### ObservationStore（会话观察存储）

**文件**：`src/rust/mcp/tools/memory/observation_store.rs`

**关键模式**：
- 使用 `tokio::sync::mpsc` 消息队列
- `Connection` 在独立线程中持有
- 消息类型：`ObservationMessage::Record(Observation)`

**代码片段**：

```rust
pub struct ObservationStore {
    conn: Connection,  // 不跨线程传递
    db_path: PathBuf,
    max_observations: usize,
    skip_tools: Vec<String>,
}

// 异步观察写入消息
enum ObservationMessage {
    Record(Observation),
}
```

### WatcherManager（文件监听）

**文件**：`src/rust/mcp/tools/acemcp/watcher.rs`

**关键模式**：
- 使用 `tokio::sync::mpsc` 通道传递文件变更事件
- 异步任务处理文件索引更新
- 防抖延迟：2 秒

**代码片段**：

```rust
pub struct WatcherManager {
    watchers: Arc<Mutex<HashMap<String, Debouncer<RecommendedWatcher, FileIdMap>>>>,
    auto_index_enabled: Arc<Mutex<bool>>,
    nested_project_map: Arc<Mutex<HashMap<String, Vec<NestedWatchInfo>>>>,
}
```

---

## SESSION_ID（供后续使用）

- **CODEX_SESSION**: null（降级模式，未调用 Codex）
- **GEMINI_SESSION**: null（降级模式，未调用 Gemini）

---

## 后续行动建议

### 立即行动（P2.1 规划）

1. **创建 P2.1 实施计划**：
   - 基于本研究报告的约束集
   - 8 步实施步骤（18 小时工时）
   - 明确验收标准（OK-1 ~ OK-14）

2. **创建 `fts_actor.rs` 模块**：
   - 定义 `FtsMessage` 枚举
   - 实现 `run_fts_actor` 异步函数
   - 添加单元测试

3. **修改 `MemoryManager`**：
   - 添加 `fts_tx` 字段
   - 在 `new()` 中启动 Actor
   - 实现 `Drop` trait

### 短期改进（Task 4）

4. **创建 `useMemorySearch` composable**：
   - 封装搜索逻辑
   - 解析 `search_mode` 字段
   - 显示搜索模式指示器

5. **更新 `MemorySearch.vue`**：
   - 使用 `useMemorySearch` composable
   - 显示 FTS5 / 模糊匹配指示器

### 长期优化（P3）

6. **性能监控**：
   - 记录 FTS5 搜索延迟（P50/P95/P99）
   - 监控消息队列长度
   - 记录降级频率

7. **中文分词优化**：
   - 评估 `jieba-rs` 集成可行性
   - 对比 `unicode61` 和 `jieba` 的搜索质量

---

## 审查元数据

- **研究模式**: 降级模式（单模型研究）
- **研究代理**: team-research-agent (主代理)
- **降级原因**: 缺少 collab Skill 和多模型调用模板
- **研究时间**: 2026-02-20
- **研究范围**: FTS5 全文搜索集成架构方案
- **参考文档**: P2 审查报告、ObservationStore、WatcherManager、acemcp 混合检索
- **用户确认**: 5 个关键设计决策（Q1-Q5）
