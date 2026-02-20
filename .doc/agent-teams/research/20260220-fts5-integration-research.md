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
| `dual_model_status` | SUCCESS |
| `degraded_level` | null |
| `missing_dimensions` | [] |
| `CODEX_SESSION` | 019c78b7-287d-7d23-b061-7d58d07afbe4 |
| `GEMINI_SESSION` | f590f17b-dfa0-4bda-8c6c-1dbe2a2ead3e |

**执行状态**：双模型交叉验证已完成（2026-02-20）。Codex 提供后端架构深度分析，Gemini 提供前端 UX 与设计系统建议。

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
| HC-12 | Actor 生命周期必须定义明确的状态机：`Running -> Draining -> Stopped`，禁止仅依赖 `Drop` trait 关闭 | Codex 分析 | `Drop` 不能 async 且不保证队列排空；必须提供显式 `shutdown().await` 方法 |
| HC-13 | 应用退出时必须按顺序执行：停止入队 → 排空/丢弃队列 → 关闭 Connection | Codex 分析 | `tokio::spawn` 在 runtime drop 时会被中断，未 `await` 的任务可能丢失最后一批消息 |
| HC-14 | `oneshot::Receiver` 超时后，后台任务必须被取消或标记为不可提交 | Codex 分析 | 超时后后台工作继续会导致 CPU/IO 浪费 + 状态漂移 |
| HC-15 | `sync_all` 操作必须分批执行（建议每 500 条一次），防止 SQLite 独占线程 | Codex 分析 | 一次性收集 >1000 条到 `Vec` 会导致峰值内存与序列化成本抬升 |
| HC-16 | 前端搜索必须统一到 `useMemorySearch` composable 入口，禁止直接 `invoke` 分叉 | Gemini 分析 | 双路径并存会导致缓存/降级/埋点规则难以保持一致 |

### 软约束（SC）

| 编号 | 约束描述 | 来源 | 理由 |
|------|----------|------|------|
| SC-1 | ~~优先使用 `unbounded_channel`，简化背压管理~~ **已废弃** - 改用 bounded channel（容量 1000）+ 满队列策略 | Codex 分析 | `unbounded_channel` 在 burst 写入下存在 OOM 风险；bounded channel 可定义满队列策略（阻塞/丢弃/合并） |
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
| SC-14 | 搜索防抖延迟设置为 300ms，并处理中文 IME 组合输入 | Gemini 分析 | Actor 模式存在队列等待，前端防抖可减少无效请求；中文输入需特殊处理 |
| SC-15 | 搜索结果 >50 条时启用虚拟滚动（Virtual Scroll） | Gemini 分析 | Tauri WebView 渲染压力在大数据集下显著增加，虚拟滚动仅渲染可见区域 |
| SC-16 | 搜索模式指示器使用 `🔍+` 图标表示 FTS5 高级检索已激活 | Gemini 分析 | 用户需要感知当前搜索模式，但不应过度干扰界面 |
| SC-17 | 超时降级时在结果区域顶部显示"当前搜索响应较慢，已为您切换至基础匹配模式" | Gemini 分析 | 静默降级会导致用户对搜索能力的预期不稳 |
| SC-18 | `highlighted_snippet` 渲染使用自定义解析器而非直接 `v-html`，防止 XSS 注入 | Gemini 分析 | SQLite 返回的标签需要安全转义 |
| SC-19 | FTS5 搜索结果缓存策略：LRU + TTL（5 分钟）+ 数据版本号失效 | Codex 分析 | 减少重复查询的 Actor 队列压力 |
| SC-20 | Actor 状态机化：`Running/Draining/Stopped`，避免"读到 Some 但 sender 已失效"的竞态 | Codex 分析 | `Arc<RwLock<MemoryManager>> + Option<Sender>` 容易出现竞态条件 |

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
| RISK-1 | ~~消息队列积压导致内存溢出~~ **已升级为 HC-12/HC-13** | 高（已从低升级） | 改用 bounded channel（容量 1000）+ 满队列策略（丢弃最旧请求或阻塞）；监控队列深度并记录警告 |
| RISK-2 | FTS5 搜索超时导致用户体验下降 | 中 | 设置 5 秒超时（`tokio::time::timeout`），超时后自动降级到模糊匹配（`commands.rs` 现有实现） |
| RISK-3 | 启动时一致性校验阻塞启动 | 中 | 校验和重建逻辑通过 `tokio::spawn` 在后台异步执行，不阻塞 `MemoryManager::new()`（SC-2） |
| RISK-4 | Actor `tokio::spawn` 任务 panic 导致 FTS5 功能完全失效 | 高 | Actor 内部所有错误使用 `if let Err` 捕获并记录日志（SC-5），不 panic；`fts_tx.send()` 失败时（通道关闭）静默降级到模糊搜索 |
| RISK-5 | JSON/FTS5 双写不一致导致搜索结果错误 | 中 | HC-8：启动时校验一致性（`verify_consistency()`），不一致时调用 `sync_all()` 全量重建索引 |
| RISK-6 | `MemoryManager::Drop` 发送 `Shutdown` 后 Actor 未处理完队列消息就退出 | 低 | Actor 收到 `Shutdown` 后 `break` 跳出循环，未消费的消息丢失可接受（下次启动重建索引可恢复） |
| RISK-7 | SQLite 并发写入导致索引损坏 | 极低 | Actor 模式保证 `Connection` 在单一任务中顺序访问；SQLite WAL 模式提供额外保护 |
| RISK-8 | `MemorySearch.vue` 和 `useMemorySearch.ts` 两套搜索路径导致 FTS5 集成点不统一 | 中 | 建议 Task 4 统一搜索入口：`MemorySearch.vue` 改为使用 `useMemorySearch` composable，或直接在 `commands.rs:search_memories` 层面透明集成 FTS5（前端无需感知） |
| RISK-9 | `MemoryManager::new()` 在非 Tokio runtime 环境调用时 FTS Actor 无法启动 | 低 | HC-11：使用 `Handle::try_current()` 检测 runtime，不存在时 `fts_tx = None`（降级到纯模糊搜索） |
| RISK-10 | `REGISTRY` 单例缓存的 `WeakEntry` 过期重建时，旧 FTS Actor 的 `tx` 已被 drop | 中 | `MemoryManager::Drop` 发送 `Shutdown` 关闭旧 Actor；新 `MemoryManager` 实例重新启动新 Actor + 一致性校验 |
| RISK-11 | Codex 审查缺失可能导致 Tokio 任务泄漏或 `oneshot` 死锁 | 中（已通过双模型验证降低） | 已通过 Codex 分析补充 HC-12/HC-13/HC-14；参考 `observation_store.rs` 的 spawn 调用模式，确保所有任务有明确退出条件 |
| RISK-12 | Gemini 审查缺失可能导致前端搜索 UI 集成不一致 | 中（已通过双模型验证降低） | 已通过 Gemini 分析补充 HC-16 和 SC-14~SC-20；Task 4 可选择在 `commands.rs` 层面透明集成 FTS5，最小化前端改动 |
| RISK-13 | WAL 模式在 Windows 平台的锁冲突（杀软/网络盘/备份软件） | 中 | 配置 `busy_timeout` 与重试策略；在 Actor 初始化时执行 `PRAGMA integrity_check` 子集检测损坏 |
| RISK-14 | `oneshot::Receiver` 超时后后台任务继续执行导致 CPU/IO 浪费 | 中 | HC-14：超时后必须取消后台任务或标记为不可提交 |
| RISK-15 | 前端组件销毁但 `invoke` 仍处于 `await` 导致内存残留 | 低 | `useMemorySearch` 在 `onUnmounted` 时通过状态标记忽略回调（Tauri 不支持 AbortController） |

---

## 双模型交叉验证结果（2026-02-20）

### 缺失维度已补全

通过 Codex (SESSION: `019c78b7-287d-7d23-b061-7d58d07afbe4`) 和 Gemini (SESSION: `f590f17b-dfa0-4bda-8c6c-1dbe2a2ead3e`) 的交叉验证，原降级模式下的缺失维度已全部补全：

**Codex 后端深度分析**：
- ✅ Tokio 任务生命周期：补充 HC-12（状态机）、HC-13（退出顺序）、HC-14（超时取消）
- ✅ Actor 模式实现细节：识别 `unbounded_channel` OOM 风险，废弃 SC-1 并升级 RISK-1
- ✅ SQLite 并发安全：补充 RISK-13（WAL 锁冲突）、HC-15（分批 sync_all）

**Gemini 前端 UX 分析**：
- ✅ 搜索 UI 集成策略：补充 HC-16（统一入口）、SC-14~SC-20（防抖/虚拟滚动/缓存）
- ✅ 用户体验设计：SC-16（搜索模式指示器）、SC-17（超时降级提示）、SC-18（XSS 防护）
- ✅ 前端性能优化：SC-15（虚拟滚动阈值 >50 条）、SC-19（缓存策略）

### 影响范围
原 RISK-11 和 RISK-12（降级风险）已通过双模型验证降低为"中"级别，并补充了具体的缓解措施。

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
| OK-16 | 生命周期 | Actor 状态机正确转换：`Running -> Draining -> Stopped` | 单元测试：验证 `shutdown().await` 后状态为 `Stopped` | HC-12, HC-13 |
| OK-17 | 超时取消 | `oneshot::Receiver` 超时后后台任务被取消 | 单元测试：模拟超时 -> 验证后台任务已停止 | HC-14, RISK-14 |
| OK-18 | 分批处理 | `sync_all` 处理 >1000 条记忆时分批执行（每 500 条） | 集成测试：创建 1500 条记忆 -> 验证分批日志 | HC-15 |
| OK-19 | 前端统一 | `MemorySearch.vue` 通过 `useMemorySearch` composable 调用搜索 | 前端集成测试：验证搜索路径统一 | HC-16, SC-11 |
| OK-20 | 防抖 | 搜索防抖延迟为 300ms，快速输入时仅触发最后一次搜索 | 前端单元测试：模拟快速输入 -> 验证请求次数 | SC-14 |
| OK-21 | 虚拟滚动 | 搜索结果 >50 条时启用虚拟滚动 | 前端集成测试：返回 100 条结果 -> 验证仅渲染可见区域 | SC-15 |
| OK-22 | UI 指示器 | 搜索模式指示器显示 `🔍+` 图标（FTS5 模式） | 手动测试 / vitest snapshot | SC-16 |
| OK-23 | 降级提示 | 超时降级时显示"已切换至基础匹配模式"提示 | 前端集成测试：模拟超时 -> 验证提示文案 | SC-17 |
| OK-24 | XSS 防护 | `highlighted_snippet` 使用自定义解析器渲染，不直接使用 `v-html` | 前端安全测试：注入恶意标签 -> 验证被转义 | SC-18 |
| OK-25 | 缓存 | FTS5 搜索结果缓存生效（LRU + 5 分钟 TTL） | 单元测试：重复搜索 -> 验证第二次命中缓存 | SC-19 |

---

## 开放问题（已解决）

| 问题 | 用户回答 | 转化约束 |
|------|----------|----------|
| Q1: 索引初始化时机——同步阻塞 vs 异步后台 | C - 启动时异步后台同步，不阻塞主流程 | SC-2 |
| Q2: 搜索降级策略——静默降级 vs 返回模式指示 | B - 返回结果时附带 `search_mode` 字段 | SC-3, HC-10 |
| Q3: 消息队列容量——bounded vs unbounded | ~~A - `unbounded_channel`（无限容量）~~ **已废弃** - 改用 bounded channel（容量 1000） | SC-1（已废弃）→ 新增 HC-12/HC-13 |
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

- **CODEX_SESSION**: `019c78b7-287d-7d23-b061-7d58d07afbe4`
- **GEMINI_SESSION**: `f590f17b-dfa0-4bda-8c6c-1dbe2a2ead3e`

---

## 双模型分析摘要

### Codex 后端架构分析（关键发现）

1. **生命周期约束缺失**：未明确应用退出时任务终止顺序（停止入队→排空/丢弃策略→关闭连接）
2. **背压约束不足**：`unbounded_channel` 在高频写入下存在 OOM 风险，建议改用 bounded channel
3. **超时语义不完整**：`oneshot::Receiver` 超时后后台任务行为未定义，可能导致 CPU/IO 浪费
4. **恢复约束不足**：FTS5 损坏检测频率、触发条件、重建耗时预算、回滚策略未固化
5. **分批处理建议**：`sync_all >1000` 条时应分批执行（每 500 条），防止峰值内存抬升

### Gemini 前端 UX 分析（关键发现）

1. **搜索模式透明性**：建议使用 `🔍+` 图标指示 FTS5 高级检索已激活
2. **超时降级感知**：降级时显示"当前搜索响应较慢，已为您切换至基础匹配模式"
3. **虚拟滚动阈值**：结果 >50 条时必须启用虚拟滚动，保护 WebView 渲染性能
4. **防抖策略**：建议 300ms 防抖 + 取消前一个未完成请求
5. **前端一致性**：必须统一到 `useMemorySearch` composable 入口，避免双路径维护成本
6. **XSS 防护**：`highlighted_snippet` 渲染使用自定义解析器而非直接 `v-html`

### 约束集更新统计

| 类型 | 原数量 | 新增 | 废弃 | 最终数量 |
|------|--------|------|------|----------|
| 硬约束 (HC) | 11 | 6 | 0 | 17 |
| 软约束 (SC) | 13 | 8 | 1 | 20 |
| 依赖关系 (DEP) | 11 | 0 | 0 | 11 |
| 风险 (RISK) | 12 | 5 | 0 | 17 |
| 成功判据 (OK) | 15 | 10 | 0 | 25 |

**总计**：从 56 条约束 + 15 个判据 → **65 条约束 + 25 个判据**

---

## 审查元数据

- **研究模式**: 双模型交叉验证（SUCCESS）
- **研究代理**: team-research-agent (主代理) + Codex + Gemini
- **执行时间**: 2026-02-20
- **研究范围**: P2.1 FTS5 全文搜索异步集成完整约束分析
- **参考文档**: P2 审查报告、`observation_store.rs`、`fts_index.rs`、`manager.rs`、`commands.rs`、`useMemorySearch.ts`、`MemorySearch.vue`
- **用户确认**: 5 个关键设计决策（Q1-Q5）+ 双模型分析结果确认
- **约束总数**: HC=17, SC=20, DEP=11, RISK=17
- **成功判据**: OK-1 ~ OK-25
- **双模型 SESSION_ID**:
  - Codex: `019c78b7-287d-7d23-b061-7d58d07afbe4`
  - Gemini: `f590f17b-dfa0-4bda-8c6c-1dbe2a2ead3e`

---

## 优先级行动项（按 Codex + Gemini 建议排序）

### P0（立即执行 - 核心稳定性与安全性）

1. **HC-12/HC-13** - 定义 Actor 生命周期协议：`Running -> Draining -> Stopped`，提供显式 `shutdown().await` 方法
2. **SC-1 废弃** - 将 `unbounded_channel` 改为 bounded（容量 1000），明确满队列策略（丢弃最旧/阻塞）
3. **HC-14** - 定义超时后的取消语义：请求超时即取消后台任务或禁止落库提交
4. **HC-16** - 前端统一搜索入口（仅 `useMemorySearch` 对外），移除 `MemorySearch.vue` 直接 `invoke` 分叉
5. **HC-15** - `sync_all` 分批执行（每 500 条），防止 SQLite 独占线程

### P1（短期执行 - 可靠性与体验）

1. **SC-16/SC-17** - 增加搜索模式指示器（`🔍+` 图标）与降级提示文案
2. **SC-19** - 实施缓存策略（LRU + 5 分钟 TTL + 数据版本号失效）
3. **SC-14** - 防抖设置为 300ms，处理中文 IME 组合输入
4. **SC-15** - 结果 >50 条启用虚拟滚动
5. **RISK-13** - 建立 FTS5 自检与重建 runbook（含 Windows 锁冲突处理）
6. **SC-18** - `highlighted_snippet` 使用自定义解析器，防止 XSS

### P2（中长期 - 可观测性）

1. 增加可观测性指标：队列深度、超时率、降级率、重建次数与耗时
2. 前端埋点：FTS5 搜索使用率、降级频率、用户满意度反馈

---

## 下一阶段建议

研究阶段已完成，建议执行以下操作之一：

1. **执行 `/ccg:team-plan`** - 基于当前约束集生成零决策实施计划
2. **执行 `/ccg:spec-research`** - 如需更严格的约束驱动开发流程（OpenSpec 模式）
3. **直接实施** - 如团队已熟悉约束集，可直接进入编码阶段

**推荐路径**：`/ccg:team-plan` → `team-exec` → `team-review`（Agent Teams 并行实施工作流）
