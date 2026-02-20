# Team Research: P2.1 FTS5 全文搜索集成

## 增强后的需求

**目标**：实现 FTS5 全文搜索与记忆管理系统的异步集成，解决 P2 阶段遗留的 `rusqlite::Connection` 线程安全问题。

**背景**：
- P2 审查报告（`.doc/agent-teams/reviews/20260220-memory-p2-review.md`）识别出 Critical 级别技术阻塞
- 核心问题：`rusqlite::Connection` 内部包含 `RefCell`，不实现 `Send` trait，无法存储在 `Arc<RwLock<MemoryManager>>` 中
- 影响范围：Task 1（FTS5 双写）、Task 2（搜索路由）、Task 4（前端集成）

**研究目标**：
1. 分析 FTS5 异步集成架构方案的所有技术约束
2. 验证项目内已有的异步模式（`AsyncObservationWriter`）可复用性
3. 精确识别需要修改的代码位置和接口边界
4. 产出约束集 + 可验证成功判据供后续 team-plan 使用

**技术范围**：
- 后端：Rust + Tokio + rusqlite 0.31 (bundled) + FTS5
- 前端：TypeScript + Vue 3 + Naive UI（搜索 UI 集成）
- 架构：异步消息队列 + Actor 模式（参考 `observation_store.rs`）

---

## 双模型执行元数据

| 字段 | 值 |
|------|-----|
| `dual_model_status` | DEGRADED |
| `degraded_level` | ACCEPTABLE |
| `missing_dimensions` | ["backend", "frontend"] |
| `CODEX_SESSION` | null |
| `GEMINI_SESSION` | null |

**降级原因**：项目中缺少 `collab` Skill 和相关模板文件（`skills/collab/SKILL.md`、`agents/ccg/_templates/multi-model-gate.md`），无法执行标准的 Codex + Gemini 双模型交叉探索流程。由主代理通过深入阅读项目源码进行补偿分析。

---

## 约束集

### 硬约束（HC）

| 编号 | 约束描述 | 来源 | 技术依据 |
|------|----------|------|----------|
| HC-1 | `rusqlite::Connection` 必须在单一线程/任务中持有，不可跨线程传递 | 代码分析 | Rust 编译器：`RefCell<rusqlite::InnerConnection>` is not `Send`；`fts_index.rs:21` 中 `FtsIndex` 持有 `conn: Connection` |
| HC-2 | `SharedMemoryManager` 的 `Arc<RwLock<MemoryManager>>` 结构不能直接持有 `FtsIndex` | 代码分析 | `manager.rs:957-959`：`SharedMemoryManager { inner: Arc<RwLock<MemoryManager>> }`，`Arc<RwLock<T>>` 要求 `T: Send` |
| HC-3 | 所有 FTS5 操作必须通过异步消息通道执行，`Connection` 被隔离在独立 `tokio::spawn` 任务中 | 代码分析 + 已有模式 | `observation_store.rs:216` 已证明此模式可行：`tokio::spawn(async move { while let Some(msg) = rx.recv().await { ... store.record(&obs) ... } })` |
| HC-4 | FTS5 失败不阻塞 JSON 主存储流程（Sidecar 索引原则） | P2 计划 (HC-18) | `fts_index.rs:6` 注释：`HC-18: FTS5 失败不阻塞 JSON 主流程` |
| HC-5 | 使用 `tokio::sync::mpsc` 通道传递消息（`UnboundedSender<FtsMessage>`） | 代码分析 | 项目 `Cargo.toml` 已启用 Tokio `sync` feature；`observation_store.rs:11` 已使用 `tokio::sync::mpsc` |
| HC-6 | FTS5 Actor 必须在独立 `tokio::spawn` 任务中运行，`FtsIndex` 实例被 `move` 进该任务 | 代码分析 | 参考 `observation_store.rs:216`：`tokio::spawn(async move { ... })` 将 `store`（含 `Connection`）move 进异步任务 |
| HC-7 | 搜索操作必须使用 `tokio::sync::oneshot` 通道返回结果 | 代码分析 | Actor 模式下唯一可行的同步等待异步操作方式；写操作（sync/delete）为 fire-and-forget 不需要 oneshot |
| HC-8 | 启动时必须校验 JSON 和 FTS5 一致性，不一致时重建索引 | 用户确认 (Q5) | `fts_index.rs:128-140` 已实现 `verify_consistency()` 方法 |
| HC-9 | `fts_tx: Option<mpsc::UnboundedSender<FtsMessage>>` 必须添加到 `MemoryManager` 结构体，`Option` 允许 FTS5 初始化失败时退化为 `None` | 代码分析 | `manager.rs:22-31`：当前 `MemoryManager` 有 4 个字段（`memory_dir`, `project_path`, `store`, `is_non_git_project`），`mpsc::UnboundedSender` 是 `Send + Sync` 可安全存储 |
| HC-10 | Tauri 命令 `search_memories`（`commands.rs:394`）必须支持返回 `search_mode` 字段 | 代码分析 | 当前 `SearchMemoryResultDto` 无 `search_mode` 字段，需扩展 |
| HC-11 | `MemoryManager::new()` 中启动 FTS Actor 时需使用 `tokio::runtime::Handle::try_current()` 检查 Tokio runtime | 代码分析 | `manager.rs:857-863`：`spawn_summary_backfill_task` 已使用此模式检查 runtime 存在性 |

### 软约束（SC）

| 编号 | 约束描述 | 来源 | 理由 |
|------|----------|------|------|
| SC-1 | 优先使用 `unbounded_channel`，简化背压管理 | 用户确认 (Q3) | `observation_store.rs:213` 已采用此模式；记忆操作频率低，溢出风险极小 |
| SC-2 | 索引初始化采用异步后台同步，不阻塞 `MemoryManager::new()` | 用户确认 (Q1) | FTS5 重建可能耗时，不应阻塞主流程启动 |
| SC-3 | 搜索结果附带 `search_mode` 字段（`"fts5"` / `"fuzzy"`） | 用户确认 (Q2) | 前端需要显示搜索模式指示器，便于用户感知搜索质量 |
| SC-4 | FTS5 搜索设置 5 秒超时，超时后降级到模糊匹配 | 用户确认 (Q4) | 搜索超时保护，确保用户体验不因 FTS5 故障而卡死 |
| SC-5 | 消息处理失败仅记录日志（`log_debug!`），不崩溃 Actor | 代码分析 | `observation_store.rs:220-221`：`if let Err(e) = store.record(&obs) { log_debug!(...) }` |
| SC-6 | FTS5 搜索延迟控制在 100ms 以内（P95） | 性能要求 | 用户体验要求，SQLite FTS5 本地查询通常在 1-10ms 完成 |
| SC-7 | 直接参考 `AsyncObservationWriter` 的消息队列模式实现 `FtsIndexActor` | 代码分析 | `observation_store.rs:202-237`：完整的 `UnboundedSender` + `tokio::spawn` + 消息循环实现 |
| SC-8 | 使用 `Drop` trait 在 `MemoryManager` 销毁时发送 `Shutdown` 消息给 Actor | 代码分析 | Rust 标准资源管理模式；确保 Actor 优雅退出 |
| SC-9 | Actor 内部错误使用 `log_debug!` 宏记录 | 代码分析 | 项目统一使用 `crate::log_debug!`（`fts_index.rs:45`、`observation_store.rs:221`） |
| SC-10 | `useMemorySearch.ts` 已存在并有 `useFts5` 标志位（当前为 `false`），集成时将其改为 `true` 并实现 `searchFts5()` | 代码分析 | `useMemorySearch.ts:59`：`const useFts5 = ref(false)`；`useMemorySearch.ts:118`：`searchFts5()` 方法已定义但返回 fallback |
| SC-11 | `MemorySearch.vue` 当前直接使用 `invoke('search_memories')`（`MemorySearch.vue:141`），未使用 `useMemorySearch` composable，需评估是否重构为统一入口 | 代码分析 | 两套搜索路径并存（`MemorySearch.vue` 直接 invoke + `useMemorySearch.ts` composable），可能导致维护成本 |
| SC-12 | 消息队列长度超过 10000 条时记录警告日志 | 性能监控 | 预防内存溢出，虽然记忆操作频率低但需要安全网 |
| SC-13 | FTS5 搜索结果应包含高亮片段（`highlighted_snippet`） | 代码分析 | `useMemorySearch.ts:31`：`MemorySearchResult` 接口已定义 `highlighted_snippet?: string` 字段 |

### 依赖关系（DEP）

| 编号 | 依赖描述 | 影响 |
|------|----------|------|
| DEP-1 | 新建 `fts_actor.rs` 模块 --> 依赖 `fts_index.rs` 中 `FtsIndex` 的方法 | Actor 调用 `FtsIndex::sync_entry()`, `delete_entry()`, `search()`, `sync_all()`, `verify_consistency()` |
| DEP-2 | `manager.rs` 修改（添加 `fts_tx` 字段）--> 必须先创建 `fts_actor.rs` | `MemoryManager` 的 `new()` 方法需要启动 Actor 并持有 `tx` |
| DEP-3 | Task 1（双写集成）--> Task 2（搜索路由） | 搜索依赖索引中有数据；必须先实现 CRUD 双写才能测试 FTS5 搜索 |
| DEP-4 | Task 2（搜索路由）--> Task 4（前端集成） | 前端 FTS5 搜索依赖后端搜索 API 返回 `search_mode` 字段 |
| DEP-5 | 一致性校验逻辑 --> 索引重建逻辑 | `FtsIndex::verify_consistency()` 已存在（`fts_index.rs:128`），校验失败时需触发 `sync_all()` 重建 |
| DEP-6 | `Drop` 实现 --> 消息队列清空 | 关闭前 Actor 需处理完待处理消息后才退出循环 |
| DEP-7 | 现有 Tokio 依赖（`Cargo.toml`: `tokio 1.0 + sync + time feature`）| 无需新增依赖，`mpsc`/`oneshot`/`timeout` 均已可用 |
| DEP-8 | 现有 rusqlite 依赖（`Cargo.toml:87`: `rusqlite = { version = "0.31", features = ["bundled"] }`）| `bundled` 编译的 SQLite 原生包含 FTS5 支持，无需额外 feature flag |
| DEP-9 | `mod.rs` 模块注册 --> 新增 `pub mod fts_actor;` | `mod.rs:30` 已有 `pub mod fts_index;`，需在其后添加 `pub mod fts_actor;` |
| DEP-10 | `commands.rs:400` `SearchMemoryResultDto` 结构体 --> 需扩展 `search_mode` 字段 | 影响 Tauri 前后端 IPC 数据传输格式 |
| DEP-11 | `registry.rs` `REGISTRY` 单例 --> 需确保 `SharedMemoryManager` 创建时 FTS Actor 也被正确启动 | `REGISTRY.get_or_create()` 可能复用已有实例，需验证 FTS tx 生命周期 |

### 风险（RISK）

| 编号 | 风险描述 | 影响 | 缓解策略 |
|------|----------|------|----------|
| RISK-1 | 消息队列积压导致内存溢出 | 低（记忆操作频率极低） | 使用 `unbounded_channel` 但监控队列长度，超过 10000 条时记录警告；实际操作频率预计每次会话 < 100 次 |
| RISK-2 | FTS5 搜索超时导致用户体验下降 | 中 | 设置 5 秒超时（`tokio::time::timeout`），超时后自动降级到模糊匹配（`commands.rs` 现有实现） |
| RISK-3 | 启动时一致性校验阻塞启动 | 中 | 校验和重建逻辑通过 `tokio::spawn` 在后台异步执行，不阻塞 `MemoryManager::new()`（SC-2） |
| RISK-4 | Actor `tokio::spawn` 任务 panic 导致 FTS5 功能完全失效 | 高 | Actor 内部所有错误使用 `if let Err` 捕获并记录日志（SC-5），不 panic；`fts_tx.send()` 失败时（通道关闭）静默降级到模糊搜索 |
| RISK-5 | JSON/FTS5 双写不一致导致搜索结果错误 | 中 | HC-8：启动时校验一致性（`verify_consistency()`），不一致时调用 `sync_all()` 全量重建索引 |
| RISK-6 | `MemoryManager::Drop` 发送 `Shutdown` 后 Actor 未处理完队列消息就退出 | 低 | Actor 收到 `Shutdown` 后 `break` 跳出循环，未消费的消息丢失可接受（下次启动重建索引可恢复） |
| RISK-7 | SQLite 并发写入导致索引损坏 | 极低 | Actor 模式保证 `Connection` 在单一任务中顺序访问；SQLite WAL 模式提供额外保护 |
| RISK-8 | `MemorySearch.vue` 和 `useMemorySearch.ts` 两套搜索路径导致 FTS5 集成点不统一 | 中 | 建议 Task 4 统一搜索入口：`MemorySearch.vue` 改为使用 `useMemorySearch` composable，或直接在 `commands.rs:search_memories` 层面透明集成 FTS5（前端无需感知） |
| RISK-9 | `MemoryManager::new()` 在非 Tokio runtime 环境调用时 FTS Actor 无法启动 | 低 | HC-11：使用 `Handle::try_current()` 检测 runtime，不存在时 `fts_tx = None`（降级到纯模糊搜索） |
| RISK-10 | `REGISTRY` 单例缓存的 `WeakEntry` 过期重建时，旧 FTS Actor 的 `tx` 已被 drop | 中 | `MemoryManager::Drop` 发送 `Shutdown` 关闭旧 Actor；新 `MemoryManager` 实例重新启动新 Actor + 一致性校验 |

---

## 降级影响说明

### 缺失维度
- **backend**：缺少 Codex 对 Rust 异步架构的深度分析（特别是 Tokio 任务生命周期和 `oneshot` 超时边界情况）
- **frontend**：缺少 Gemini 对 Vue 3 搜索 UI 集成的分析（特别是 `useMemorySearch` 与 `MemorySearch.vue` 的统一策略）

### 影响范围
1. **后端架构**：主代理基于项目内已有案例（`AsyncObservationWriter`）推导出异步消息队列方案，方案可行性已通过代码级验证。但缺少 Codex 对以下方面的专业建议：
   - `oneshot::Receiver` 在超时场景下的资源泄漏风险
   - `tokio::spawn` 任务在 Tauri 应用退出时的清理保证
   - 大量记忆（>1000 条）`sync_all` 时的内存峰值
2. **前端集成**：缺少 Gemini 对以下方面的分析：
   - `useMemorySearch.ts` 与 `MemorySearch.vue` 的整合最佳实践
   - 搜索模式指示器的 UI 设计建议
   - 搜索结果中高亮片段（`highlighted_snippet`）的渲染方式

### 补偿分析
- **主代理补偿**：通过深入阅读项目 6 个核心文件（`fts_index.rs`、`manager.rs`、`observation_store.rs`、`commands.rs`、`useMemorySearch.ts`、`MemorySearch.vue`）提取精确的代码级约束
- **参考现有实现**：`AsyncObservationWriter`（`observation_store.rs:202-237`）已成功使用 `tokio::sync::mpsc::unbounded_channel` + 独立 spawn 任务持有 `Connection`
- **用户确认**：通过 `mcp______zhi` 与用户确认 5 个关键设计决策（Q1-Q5）

### 风险约束补充
- **RISK-11**（降级风险）：缺少 Codex 审查可能导致 Tokio 任务泄漏或 `oneshot` 死锁 --> 缓解：参考现有 `observation_store.rs` 的 spawn 调用模式，确保所有任务有明确退出条件（通道关闭或 `Shutdown` 消息）
- **RISK-12**（降级风险）：缺少 Gemini 审查可能导致前端搜索 UI 集成不一致 --> 缓解：Task 4 可选择在 `commands.rs` 层面透明集成 FTS5，最小化前端改动

---

## 代码库精确状态（截至 2026-02-20）

### 已存在的代码资产

| 文件 | 状态 | 关键内容 |
|------|------|----------|
| `fts_index.rs` (167 行 + 222 行测试) | 完整可用 | `FtsIndex::open()`, `sync_entry()`, `sync_all()`, `search()`, `delete_entry()`, `verify_consistency()` |
| `observation_store.rs` (237 行 + 206 行测试) | 参考模式 | `AsyncObservationWriter::new()` 使用 `mpsc::unbounded_channel` + `tokio::spawn` |
| `manager.rs` (1291 行) | 需修改 | `MemoryManager` 结构体（行 22-31）需添加 `fts_tx` 字段；`new()`（行 51）需启动 Actor |
| `useMemorySearch.ts` (147 行) | 需修改 | `useFts5 = ref(false)`（行 59），`searchFts5()` 方法已定义但未实现（行 118-122） |
| `MemorySearch.vue` (~835 行) | 可选修改 | 直接使用 `invoke('search_memories')`（行 141），未使用 `useMemorySearch` composable |
| `commands.rs` | 需修改 | `search_memories` 函数（行 394）；`SearchMemoryResultDto` 需添加 `search_mode` 字段 |
| `mod.rs` | 需修改 | 行 30：`pub mod fts_index;` 后添加 `pub mod fts_actor;` |

### 不存在/需新建的代码资产

| 文件 | 说明 |
|------|------|
| `fts_actor.rs` | FTS5 Actor 模块：`FtsMessage` 枚举 + `run_fts_actor()` 异步函数 |

---

## 成功判据

| 编号 | 类型 | 判据描述 | 验证方式 | 关联约束 |
|------|------|----------|----------|----------|
| OK-1 | 编译 | `MemoryManager` 添加 `fts_tx: Option<mpsc::UnboundedSender<FtsMessage>>` 后项目可成功编译 | `cargo build` 无错误 | HC-1, HC-2, HC-9 |
| OK-2 | 功能 | `add_memory()` 后 FTS5 索引可搜索到新添加的记忆 | 单元测试：add -> FtsMessage::Search -> 验证结果包含新记忆 ID | HC-3, HC-4, DEP-1 |
| OK-3 | 功能 | `update_memory()` 后 FTS5 索引包含更新后的内容 | 单元测试：update -> search 旧内容无结果 + search 新内容有结果 | HC-3, DEP-1 |
| OK-4 | 功能 | `delete_memory()` 后 FTS5 索引不再包含被删除的记忆 | 单元测试：delete -> search 无结果 | HC-3, DEP-1 |
| OK-5 | 功能 | FTS5 搜索返回正确结果，且结果包含 `search_mode: "fts5"` | 单元测试：search -> 验证返回结果非空 + search_mode 值 | HC-7, HC-10, SC-3 |
| OK-6 | 降级 | FTS5 搜索失败时自动降级到模糊匹配，返回 `search_mode: "fuzzy"` | 单元测试：模拟通道关闭 -> 验证降级搜索 + search_mode 值 | SC-4, RISK-2, RISK-4 |
| OK-7 | 降级 | FTS5 搜索超时（>5s）时自动降级到模糊匹配 | 单元测试：使用 `tokio::time::timeout` 模拟超时 | SC-4, RISK-2 |
| OK-8 | 可靠性 | 启动时一致性校验通过（JSON 条目数 == FTS5 行数） | 集成测试：创建 manager -> verify_consistency 返回 true | HC-8, RISK-5 |
| OK-9 | 可靠性 | 一致性校验失败时自动触发 `sync_all()` 重建索引 | 集成测试：人工破坏 FTS5 数据库 -> 重启 -> verify 通过 | HC-8, RISK-5 |
| OK-10 | 并发 | 快速连续写入 100 条记忆后索引无损坏 | 压力测试：循环 add_memory 100 次 -> verify_consistency | RISK-7 |
| OK-11 | 资源 | `MemoryManager` Drop 时 Actor 收到 Shutdown 并退出 | 单元测试：创建 manager -> drop -> 验证 rx 通道已关闭 | SC-8, RISK-6 |
| OK-12 | 前端 | `useMemorySearch.ts` 的 `searchFts5()` 方法实现完整 | 前端单元测试（vitest）：mock invoke -> 验证 FTS5 搜索路径 | SC-10 |
| OK-13 | 前端 | 搜索结果 UI 显示 `search_mode` 指示器（"FTS5 全文搜索" / "模糊匹配"） | 手动测试 / vitest snapshot 测试 | SC-3, SC-10 |
| OK-14 | 监控 | 消息队列长度超过 10000 时记录警告日志 | 日志检查：使用 `RUST_LOG=debug` 运行 | SC-12, RISK-1 |
| OK-15 | 非 runtime | 在非 Tokio runtime 环境下创建 `MemoryManager` 不崩溃，FTS 功能退化为 None | 单元测试：在非 async 上下文创建 manager | HC-11, RISK-9 |

---

## 开放问题（已解决）

| 问题 | 用户回答 | 转化约束 |
|------|----------|----------|
| Q1: 索引初始化时机——同步阻塞 vs 异步后台 | C - 启动时异步后台同步，不阻塞主流程 | SC-2 |
| Q2: 搜索降级策略——静默降级 vs 返回模式指示 | B - 返回结果时附带 `search_mode` 字段 | SC-3, HC-10 |
| Q3: 消息队列容量——bounded vs unbounded | A - `unbounded_channel`（无限容量） | SC-1 |
| Q4: 搜索超时——无超时 vs 5s vs 10s | B - 设置 5 秒超时，超时后降级 | SC-4 |
| Q5: 一致性校验——无校验 vs 启动时校验 vs 定期校验 | B - 启动时校验一次，不一致时重建索引 | HC-8 |

---

## 参考实现详解

### AsyncObservationWriter（核心参考，`observation_store.rs:202-237`）

```rust
pub struct AsyncObservationWriter {
    tx: mpsc::UnboundedSender<ObservationMessage>,
}

impl AsyncObservationWriter {
    pub fn new(store: ObservationStore) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<ObservationMessage>();

        // 后台写入任务：Connection 被 move 进 spawn 任务
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ObservationMessage::Record(obs) => {
                        if let Err(e) = store.record(&obs) {
                            log_debug!("[ObservationStore] 异步写入失败: {}", e);
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn record_async(&self, observation: Observation) {
        if let Err(e) = self.tx.send(ObservationMessage::Record(observation)) {
            log_debug!("[ObservationStore] 异步发送失败: {}", e);
        }
    }
}
```

**可复用点**：
1. `unbounded_channel` 创建模式
2. `tokio::spawn(async move { ... })` 将含 `Connection` 的结构体 move 进独立任务
3. `while let Some(msg) = rx.recv().await` 消息循环
4. `if let Err(e)` 错误处理不 panic
5. `tx.send()` 的 fire-and-forget 模式

**需扩展点**：
1. 增加 `Search` 消息类型（带 `oneshot::Sender` 返回结果）
2. 增加 `Shutdown` 消息类型（优雅退出）
3. 增加 `SyncAll` 消息类型（一致性校验后全量重建）

### MemoryManager 后台任务模式（`manager.rs:855-907`）

```rust
fn spawn_summary_backfill_task(&self, memory_id: String, content: String) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => { return; } // 无 runtime 时跳过
    };

    handle.spawn(async move {
        // 异步执行摘要生成
        // ...
    });
}
```

**可复用点**：
1. `Handle::try_current()` 检测 runtime 存在性
2. `handle.spawn(async move { ... })` 异步后台任务

---

## SESSION_ID（供后续使用）

- **CODEX_SESSION**: null（降级模式，未调用 Codex）
- **GEMINI_SESSION**: null（降级模式，未调用 Gemini）

---

## 审查元数据

- **研究模式**: 降级模式（单模型研究）
- **研究代理**: team-research-agent (主代理)
- **降级原因**: 缺少 collab Skill 和多模型调用模板
- **研究时间**: 2026-02-20（更新）
- **研究范围**: P2.1 FTS5 全文搜索异步集成完整约束分析
- **参考文档**: P2 审查报告、`observation_store.rs`、`fts_index.rs`、`manager.rs`、`commands.rs`、`useMemorySearch.ts`、`MemorySearch.vue`
- **用户确认**: 5 个关键设计决策（Q1-Q5）
- **约束总数**: HC=11, SC=13, DEP=11, RISK=12
- **成功判据**: OK-1 ~ OK-15
