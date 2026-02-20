# Team Review: 记忆管理系统 P2 阶段实施

## 审查概况

| 指标 | 值 |
|------|-----|
| 审查模式 | 降级模式（单模型审查） |
| 审查文件数 | 23 |
| 变更行数 | +4456 / -1841 |
| 关键提交数 | 2 |
| 最终发现数 | 12 |
| 审查时间 | 2026-02-20 |

**降级原因**：项目中缺少 `collab` Skill 和相关模板文件（`skills/collab/SKILL.md`、`agents/ccg/_templates/multi-model-gate.md`），无法执行标准的 Codex + Gemini 双模型交叉审查流程。

## 发现详情

### Critical (1 issue) - 必须修复

| # | 维度 | 文件:行 | 描述 | 来源 | 修复建议 |
|---|------|---------|------|------|----------|
| C1 | 架构限制 | `src/rust/mcp/tools/memory/manager.rs` + `fts_index.rs` | **Task 1 技术阻塞**：FTS5 双写集成遇到线程安全问题。`rusqlite::Connection` 不是 `Send`，无法在 `Arc<RwLock<MemoryManager>>` 中使用。尝试集成时编译失败：`RefCell<rusqlite::Connection>` cannot be shared between threads safely | 主代理 | **方案 A**（推荐）：将 FTS5 集成延迟到 P2.1，使用异步消息队列架构。**方案 B**：使用 `Arc<Mutex<FtsIndex>>` 并重构 `SharedMemoryManager`。**方案 C**：使用 `r2d2` 连接池管理 SQLite 连接 |

### Warning (7 issues) - 建议修复

| # | 维度 | 文件:行 | 描述 | 来源 | 修复建议 |
|---|------|---------|------|------|----------|
| W1 | 功能缺失 | `src/frontend/composables/` | **Task 4 未完成**：前端缺少 `useMemorySearch.ts` composable。计划要求封装 FTS5 搜索逻辑和降级处理，但文件不存在 | 主代理 | 创建 `src/frontend/composables/useMemorySearch.ts`，当前可先实现模糊搜索封装，为未来 FTS5 集成预留接口 |
| W2 | 测试覆盖 | `src/rust/mcp/tools/memory/manager.rs:1313` | 测试代码存在未使用变量 `id3`，可能导致测试逻辑不完整 | 主代理 | 检查 `test_get_domain_list_with_entries` 测试，确认 `id3` 是否应参与断言 |
| W3 | 代码质量 | 多个文件 | 编译产生 9 个警告（未使用导入、未使用变量、未读取字段），影响代码可维护性 | 主代理 | 运行 `cargo fix --lib -p sanshu --tests` 修复 5 个可自动修复的警告，手动处理其余 4 个 |
| W4 | 文件组织 | `.doc/` 目录 | 32 个未跟踪文件（包括测试文件、新组件、文档），可能导致版本控制混乱 | 主代理 | 将必要文件添加到 Git（如 `SnapshotDiff.vue`、`MemoryWorkspace.vue`、测试文件），删除临时文件（如 `nul`） |
| W5 | API 兼容性 | `src/rust/mcp/commands.rs:395` | `search_memories` 函数签名变更（新增 `domain` 和 `tags` 参数），可能破坏现有调用方 | 主代理 | 确认前端调用是否已更新。若未更新，考虑使用 `Option` 参数保持向后兼容 |
| W6 | 功能完整性 | `src/rust/mcp/tools/memory/summary_service.rs:116` | 摘要生成的 enhance 工具集成未完成（`try_enhance_summary` 直接返回错误），强制降级到规则引擎 | 主代理 | 集成 enhance 工具调用（Ollama → 云端 API），或在文档中明确标注"当前版本仅支持规则引擎" |
| W7 | 用户体验 | `src/frontend/components/tools/MemoryWorkspace.vue` | 渐进式披露三态交互已实现，但缺少 850px 窗口下的布局测试验证 | 主代理 | 在浏览器中测试 850px 窗口宽度，确认布局不溢出（参考计划 Task 5 验收标准） |

### Info (3 issues) - 可选

| # | 维度 | 文件:行 | 描述 | 来源 |
|---|------|---------|------|------|
| I1 | 文档完整性 | `.doc/agent-teams/reviews/` | 缺少 P2 实施过程文档（如 `wip/execution/` 中的执行日志） | 主代理 |
| I2 | 性能优化 | `src/rust/mcp/commands.rs:459` | 意图识别加权使用硬编码关键词列表，可考虑使用配置文件或 NLP 库提升准确性 | 主代理 |
| I3 | 代码注释 | `src/rust/mcp/tools/memory/manager.rs:232` | `spawn_summary_backfill_task` 调用缺少失败处理说明（如 Tokio runtime 不存在时的行为） | 主代理 |

## 已通过检查

- ✅ **Task 3 部分完成**：摘要生成服务已实现（`summary_service.rs`），包含 Provider 链和超时保护
- ✅ **Task 3 触发集成**：`add_memory()` 和 `update_memory()` 中已调用 `should_auto_generate_summary()` 和 `spawn_summary_backfill_task()`
- ✅ **Task 5 完成**：渐进式披露三态交互已实现（`useProgressiveDisclosure.ts` + `MemoryWorkspace.vue`）
- ✅ **Task 6 完成**：Snapshot Diff 完整集成（`SnapshotDiff.vue` + 版本历史 Modal + 回滚功能）
- ✅ **搜索增强**：`search_memories` 新增域过滤、标签过滤和意图识别加权（SC-17、SC-24）
- ✅ **测试编译**：所有测试可成功编译（仅有 9 个警告，无错误）
- ✅ **FTS5 索引模块**：`fts_index.rs` 已实现（`sync_entry`、`sync_all`、`delete_entry`、`search` 方法）

## 约束合规检查

| 约束编号 | 约束描述 | 合规状态 | 备注 |
|----------|----------|----------|------|
| HC-16 | Sidecar 索引，不替换 JSON 主存储 | ✅ 合规 | `fts_index.rs` 使用独立 SQLite 文件 |
| HC-18 | FTS5 失败不阻塞 JSON 主流程 | ⚠️ 部分合规 | `fts_index.rs` 有错误处理，但 `manager.rs` 未集成双写逻辑 |
| SC-17 | 域和标签过滤 | ✅ 合规 | `search_memories` 已实现域和标签过滤 |
| SC-23 | unicode61 分词器 | ✅ 合规 | `fts_index.rs:34` 使用 `tokenize='unicode61'` |
| SC-24 | 意图识别加权 | ✅ 合规 | `commands.rs:501` 实现 `calculate_intent_boost()` |

## 成功判据验证

| 判据编号 | 判据描述 | 验证状态 | 验证方式 |
|----------|----------|----------|----------|
| P2-1 | CRUD 后 FTS 索引可用 | ❌ 未通过 | 代码审查：`manager.rs` 未调用 `FtsIndex::sync_entry()` |
| P2-2 | FTS 异常时搜索可自动降级 | ⚠️ 部分通过 | `search_memories` 有降级逻辑，但前端缺少 `useMemorySearch` 封装 |
| P2-3 | >500 字内容自动生成摘要 | ✅ 通过 | `manager.rs:232` 调用 `spawn_summary_backfill_task()` |
| P2-4 | 增强失败时规则摘要兜底 | ✅ 通过 | `summary_service.rs:86` 降级到 `SummaryGenerator::generate_rule_based()` |
| P2-5 | Snapshot Diff 可触发、可查看、可回滚 | ✅ 通过 | `MemoryWorkspace.vue:247` + `SnapshotDiff.vue:170` |
| P2-6 | 回滚后 UI 与数据一致刷新 | ✅ 通过 | `MemoryWorkspace.vue:263` 局部更新记忆条目 |
| P2-7 | 三态渐进式披露流畅切换 | ✅ 通过 | `useProgressiveDisclosure.ts` + `MemoryWorkspace.vue` |
| P2-8 | 850px 窗口下布局不溢出 | ⚠️ 待验证 | 需手动测试验证 |
| P2-9 | 所有功能通过单元测试和集成测试 | ⚠️ 部分通过 | 测试可编译，但 FTS5 集成测试缺失 |

## 提交历史分析

| 提交 SHA | 提交信息 | 关联任务 | 状态 |
|----------|----------|----------|------|
| `521b643` | feat(memory): Task 3 - 记忆摘要生成触发 | Task 3 | ✅ 完成 |
| `ca08466` | feat(memory): FTS5 后端双写集成 | Task 1 | ⚠️ 标题与实际不符（双写未集成） |
| `fbb0562` | docs(spec): 上下文检索优化审查报告与归档 | 文档 | ✅ 完成 |
| `fc5dc9d` | feat(memory): 上下文检索优化实施（P0-P3 全部完成） | P0-P3 | ✅ 完成 |

**关键发现**：提交 `ca08466` 的标题为"FTS5 后端双写集成"，但实际代码中未找到双写逻辑，存在标题与内容不一致的问题。

## 未完成任务清单

### Task 1: FTS5 后端双写集成 - ⚠️ 技术阻塞

**已完成**：
- ✅ `fts_index.rs` 模块实现（`sync_entry`、`sync_all`、`delete_entry`、`search`、`verify_consistency`）
- ✅ FTS5 表结构设计（unicode61 分词器）

**技术阻塞**：
- ❌ `rusqlite::Connection` 不是 `Send`，无法在 `Arc<RwLock<MemoryManager>>` 中使用
- ❌ 尝试集成时编译失败：`RefCell<rusqlite::Connection>` cannot be shared between threads safely

**根本原因**：
- `MemoryManager` 被 `SharedMemoryManager` 包装为 `Arc<RwLock<>>`，用于并发访问
- `FtsIndex` 包含 `rusqlite::Connection`，它内部使用 `RefCell`，不是线程安全的
- Rust 编译器拒绝将非 `Send` 类型放入 `Arc<RwLock<>>`

**解决方案**（需要架构重构）：
- **方案 A**（推荐）：异步消息队列架构，将 FTS5 操作移到独立线程
- **方案 B**：使用 `r2d2` 连接池管理 SQLite 连接
- **方案 C**：使用 `Arc<Mutex<FtsIndex>>` 并接受性能开销

**影响**：FTS5 搜索加速功能延迟到 P2.1，当前仍使用模糊匹配搜索。

### Task 2: FTS5 搜索路由与降级 - ⚠️ 部分完成

**已完成**：
- ✅ `search_memories` 函数新增域和标签过滤
- ✅ 意图识别加权逻辑

**缺失内容**：
- ❌ 未调用 `FtsIndex::search(query, limit)`
- ❌ 未实现 FTS5 失败时的降级逻辑
- ❌ 未添加 `search_mode` 字段到返回结果

**影响**：搜索仍使用模糊匹配，无法利用 FTS5 的性能优势。

### Task 4: FTS5 前端搜索集成 - ❌ 未完成

**缺失内容**：
1. 缺少 `src/frontend/composables/useMemorySearch.ts` 文件
2. `MemorySearch.vue` 未使用 `useMemorySearch` composable
3. 缺少搜索模式指示器（FTS5 / 模糊匹配）

**影响**：前端无法调用 FTS5 搜索，降级逻辑未封装。

## 风险评估

| 风险等级 | 风险描述 | 影响范围 | 缓解措施 |
|----------|----------|----------|----------|
| 🔴 高 | FTS5 双写遇到线程安全技术阻塞，无法在当前架构下集成 | 搜索性能优化 | 将 FTS5 集成延迟到 P2.1，重新设计架构（异步消息队列或连接池） |
| 🟡 中 | 前端缺少 `useMemorySearch` composable | 前端搜索封装 | 补充 Task 4，先实现模糊搜索封装，为未来 FTS5 集成预留接口 |
| 🟡 中 | 32 个未跟踪文件可能导致版本控制混乱 | 项目管理 | 清理临时文件，添加必要文件到 Git |
| 🟢 低 | 编译警告影响代码质量 | 可维护性 | 运行 `cargo fix` 修复警告 |

## 建议与后续行动

### 立即行动（Critical 修复）

1. **重新规划 Task 1（FTS5 双写）**：
   - **问题**：`rusqlite::Connection` 不是 `Send`，无法在 `Arc<RwLock<MemoryManager>>` 中使用
   - **短期方案**：将 FTS5 集成标记为"技术阻塞"，延迟到 P2.1
   - **长期方案**：
     - **方案 A**（推荐）：使用异步消息队列（如 `tokio::sync::mpsc`），将 FTS5 操作移到独立线程
     - **方案 B**：使用 `r2d2` 连接池管理 SQLite 连接，确保线程安全
     - **方案 C**：使用 `Arc<Mutex<FtsIndex>>` 并接受性能开销
   - **时间估算**：方案 A 需 2-3 天，方案 B 需 1-2 天，方案 C 需 4-6 小时

### 短期改进（Warning 修复）

2. **创建 `useMemorySearch.ts` composable**（Task 4 部分完成）
   - 封装当前的模糊搜索逻辑
   - 预留 FTS5 搜索接口（`useFtsSearch` 标志）
   - 在 `MemorySearch.vue` 中使用该 composable

3. 清理未跟踪文件，将必要文件添加到 Git
4. 运行 `cargo fix --lib -p sanshu --tests` 修复编译警告
5. 测试 850px 窗口下的布局（验证 Task 5 验收标准）
6. 更新前端调用以适配 `search_memories` 的新签名

### 长期优化（Info 改进）

7. 补充 P2 实施过程文档（执行日志、决策记录）
8. 优化意图识别加权（使用配置文件或 NLP 库）
9. 补充 `spawn_summary_backfill_task` 的失败处理文档
10. 设计 FTS5 异步集成架构（P2.1 规划）

## 审查结论

**总体评估**：P2 阶段实施 **部分完成**，核心功能（摘要生成、Snapshot Diff、渐进式披露）已实现，但 FTS5 双写集成遇到技术阻塞（线程安全问题），需要重新设计架构。

**完成度**：
- Task 1（FTS5 后端双写）：20% ⚠️（模块已实现，但集成受阻）
- Task 2（FTS5 搜索路由）：40% ⚠️（搜索增强已完成，FTS5 路由未实现）
- Task 3（记忆摘要生成）：100% ✅
- Task 4（FTS5 前端搜索）：0% ❌
- Task 5（渐进式披露）：100% ✅
- Task 6（Snapshot Diff）：100% ✅

**整体完成度**：60% (3.6/6 任务)

**技术债务**：
- FTS5 集成需要架构重构（异步消息队列或连接池）
- 估算工作量：2-3 天（方案 A）或 1-2 天（方案 B）

**建议**：
1. **可以合并到主分支**，但需标注"FTS5 集成延迟到 P2.1"
2. Task 3/5/6 已完成，可立即使用（摘要生成、渐进式披露、版本回滚）
3. 创建 P2.1 计划，专门处理 FTS5 异步集成架构
4. 补充 `useMemorySearch` composable，封装当前搜索逻辑

## 审查元数据

- **审查模式**: 降级模式（单模型审查）
- **审查代理**: team-review-agent (主代理)
- **降级原因**: 缺少 collab Skill 和多模型调用模板
- **审查时间**: 2026-02-20
- **审查范围**: `.doc/agent-teams/plans/20260220-memory-p2-implementation-plan.md` 对应的变更
- **变更基线**: HEAD (最新提交 `521b643`)
- **审查工具**: Git diff + 代码静态分析 + 计划对照
