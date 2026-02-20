# Team Plan: 记忆管理系统 P2 阶段实施

## 概述

为记忆管理系统（ji 工具）实施 P2 阶段功能：FTS5 检索集成、记忆摘要自动生成、Snapshot Diff 前端完整集成。

## Codex 分析摘要

**核心结论**：P2 的核心瓶颈不是"功能缺失"，而是"链路未打通"。`fts_index.rs` 已有能力，但 `manager.rs` CRUD 未形成稳定双写；`summary` 字段已存在但无触发；`SnapshotDiff.vue` 已实现但主工作流触发可能断链。

**架构决策**：
- **FTS5 双写策略**：主存储（JSON）先成功，再执行 `FtsIndex::sync_entry/delete_entry`；若索引失败，记录 `index_dirty` 并告警，搜索时自动降级到模糊匹配
- **搜索路由策略**：默认 FTS5 优先，FTS5 不可用或查询异常时走现有模糊匹配
- **摘要生成策略**：在 `add_memory` 内触发，条件 `content.len() > 500 && summary.is_none()`；采用"增强链路 + 规则引擎兜底"
- **Snapshot Diff 集成策略**：`MemoryWorkspace.vue` 作为编排层，负责触发 → 拉取快照数据 → 打开 `SnapshotDiff.vue` → 回滚后刷新列表

**关键风险**：
- 双写不一致风险：采用"主写成功 + 索引补偿重建"模型
- 摘要生成延迟风险：设置超时与分级降级
- 前端事件断链风险：先做事件/props 清单核对，再补齐触发点

## Gemini 分析摘要

**架构评估**：
- **数据流碎片化**：搜索逻辑与视图展现的解耦尚未完全通过统一的 Store 或 Composable 闭环
- **交互状态缺失**：渐进式披露目前仅有 Hook，但在组件层未定义明确的"摘要态"与"详情态"的视觉切换边界
- **Snapshot 链路空悬**：`SnapshotDiff.vue` 缺乏与 Tauri 后端 `get_memory_history` 等指令的粘合层

**关键设计决策**：
- **三态渐进式披露模型**：
  - Collapsed (28px)：仅显示标题 + 标签 + 活力值徽章
  - Expanded (80px-120px)：显示 `summary` 字段，限制 3 行
  - Detail (Auto)：显示完整 Markdown 内容及 `SnapshotDiff` 触发入口
- **Snapshot 数据流**：使用 `n-modal` 挂载 `SnapshotDiff`，避免在 850px 宽度下挤压列表空间
- **850px 布局策略**：左侧 `DomainTree` 220px，右侧 `MemoryWorkspace` 630px

## 技术方案

### 1. FTS5 检索集成

**后端改造**（Codex 权威）：
- 在 `manager.rs` 的 `add_memory`/`update_memory`/`delete_memory` 中调用 `FtsIndex::sync_entry`/`delete_entry`
- 搜索路径改造：`search_memories(query)` 先走 FTS5，失败/空结果按策略降级到模糊匹配
- 一致性保障：启动时执行 `reconcile_index()`，维护 `fts_available` 与 `index_dirty` 状态

**前端集成**（Gemini 权威）：
- 修改 `MemorySearch.vue`，封装 `useMemorySearch` composable 处理 FTS5 调用与降级
- 在 `MemoryWorkspace` 中增加对 `highlighted_snippet` 的支持
- 高亮渲染：直接信任后端返回的 HTML 安全片段（使用 `v-html` 配合严格的 CSS 样式）

### 2. 记忆摘要自动生成

**后端实现**（Codex 权威）：
- 新增摘要服务 `summary_service.rs`：`generate_summary(content) -> SummaryResult`
- Provider 链：Ollama（本地）→ 云端增强 → 规则引擎
- `add_memory` 集成：命中条件时调用生成逻辑；失败兜底规则摘要
- 降级规则引擎：首行提取 + 高频关键词（去停用词）+ 长度截断（80~120 字）

**前端显示**（Gemini 权威）：
- 更新 `MemoryWorkspace.vue` 的卡片模板，根据 `disclosureState` 切换容器高度和内容显示
- 实现 `summary` 字段的 `line-clamp` 展示

### 3. Snapshot Diff 完整集成

**前端实现**（Gemini 权威）：
- 在 `MemoryWorkspace` 中定义 `handleViewHistory(item)`
- 在 Modal 中调用 `get_memory_snapshots(id)` 并传递给 `SnapshotDiff`
- 实现 `rollback_memory(id, version)` 后的列表局部刷新

**后端支持**（Codex 权威）：
- 确认 `get_memory_snapshots` 和 `rollback_memory` 指令已暴露给前端
- 回滚后刷新当前条目、时间戳、活力值数据

## 子任务列表

### Task 1: FTS5 后端双写集成
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/manager.rs`
  - `src/rust/mcp/tools/memory/fts_index.rs`
- **依赖**: 无
- **实施步骤**:
  1. 在 `MemoryManager::add_memory()` 成功写入后调用 `FtsIndex::sync_entry(entry)`
  2. 在 `MemoryManager::update_memory()` 成功更新后调用 `FtsIndex::sync_entry(updated_entry)`
  3. 在 `MemoryManager::delete_memory()` 成功删除后调用 `FtsIndex::delete_entry(memory_id)`
  4. 在 `MemoryManager::new()` 或 `load_store()` 中初始化 `FtsIndex`，捕获异常并设置 `fts_available` 标志
  5. 添加 `reconcile_index()` 方法，启动时执行一次索引一致性校验
- **验收标准**:
  - CRUD 操作后 FTS5 索引自动同步
  - FTS5 初始化失败不阻塞主流程
  - 启动时自动修复索引不一致

### Task 2: FTS5 搜索路由与降级
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/commands.rs`（`search_memories` 函数）
  - `src/rust/mcp/tools/memory/manager.rs`
- **依赖**: Task 1
- **实施步骤**:
  1. 修改 `search_memories` 函数，优先调用 `FtsIndex::search(query, limit)`
  2. 若 FTS5 返回结果，按 ID 批量获取完整记忆条目
  3. 若 FTS5 失败或返回空，降级到现有模糊匹配逻辑
  4. 添加 `search_mode` 字段到返回结果，标注使用的搜索方式（`fts5` / `fuzzy`）
- **验收标准**:
  - FTS5 可用时搜索结果准确且快速
  - FTS5 不可用时自动降级到模糊匹配
  - 搜索结果包含搜索模式标识

### Task 3: 记忆摘要生成服务
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/summary_service.rs`（新建）
  - `src/rust/mcp/tools/memory/manager.rs`
  - `src/rust/mcp/tools/memory/mod.rs`
- **依赖**: 无
- **实施步骤**:
  1. 创建 `summary_service.rs`，定义 `SummaryService` 结构体
  2. 实现 `generate_summary(content: &str) -> Result<String>` 方法
  3. Provider 链：尝试调用 enhance 工具（Ollama → 云端），失败则使用规则引擎
  4. 规则引擎实现：提取首行 + 提取高频词（去停用词）+ 截断到 80-120 字
  5. 在 `MemoryManager::add_memory()` 中，当 `content.len() > 500 && summary.is_none()` 时调用摘要生成
  6. 设置超时（5 秒），超时则使用规则引擎兜底
- **验收标准**:
  - >500 字内容自动生成摘要
  - 增强工具失败时规则引擎兜底
  - 摘要生成不阻塞写入流程（超时保护）

### Task 4: FTS5 前端搜索集成
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemorySearch.vue`
  - `src/frontend/composables/useMemorySearch.ts`（新建）
- **依赖**: Task 2
- **实施步骤**:
  1. 创建 `useMemorySearch.ts` composable，封装搜索逻辑
  2. 实现 FTS5 搜索调用：`invoke('search_memories', { query, limit: 20 })`
  3. 实现降级逻辑：捕获异常后调用 `invoke('get_memories', { filter: query })`
  4. 在 `MemorySearch.vue` 中使用 `useMemorySearch`，替换现有搜索逻辑
  5. 添加搜索模式指示器（FTS5 / 模糊匹配）
- **验收标准**:
  - 搜索结果准确且快速
  - FTS5 失败时自动降级无感知
  - UI 显示当前搜索模式

### Task 5: 渐进式披露三态交互
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemoryWorkspace.vue`
  - `src/frontend/composables/useProgressiveDisclosure.ts`
- **依赖**: Task 3
- **实施步骤**:
  1. 在 `useProgressiveDisclosure.ts` 中定义三态枚举：`collapsed` / `expanded` / `detail`
  2. 在 `MemoryWorkspace.vue` 中为每个记忆卡片维护 `disclosureState`
  3. 实现三态视图切换：
     - Collapsed：标题 + 标签 + VitalityBadge（高度 28px）
     - Expanded：summary 字段 + 3 行 line-clamp（高度 80-120px）
     - Detail：完整 Markdown 内容 + 操作按钮（高度自适应）
  4. 添加点击/双击事件切换状态
- **验收标准**:
  - 三态切换流畅无卡顿
  - 摘要显示正确（有摘要显示摘要，无摘要显示前 100 字）
  - 850px 窗口下布局不溢出

### Task 6: Snapshot Diff 完整集成
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemoryWorkspace.vue`
  - `src/frontend/components/tools/SnapshotDiff.vue`
- **依赖**: 无
- **实施步骤**:
  1. 在 `MemoryWorkspace.vue` 的 Detail 态添加"查看历史"按钮（时钟图标）
  2. 实现 `handleViewHistory(item)` 方法：
     - 调用 `invoke('get_memory_snapshots', { memoryId: item.id })`
     - 打开 `n-modal`，传递 snapshots 数据给 `SnapshotDiff`
  3. 在 `SnapshotDiff.vue` 中添加版本选择器（n-select）
  4. 实现回滚确认对话框（n-modal + n-button）
  5. 回滚成功后：
     - 调用 `invoke('rollback_memory', { memoryId, targetVersion })`
     - 关闭 Modal
     - 刷新当前记忆条目（局部更新，不刷新整个列表）
- **验收标准**:
  - "查看历史"按钮可见且可点击
  - Snapshot Diff 正确显示版本对比
  - 回滚成功后列表数据正确更新
  - 850px 窗口下 Modal 布局合理

## 文件冲突检查

| 文件路径 | 归属任务 | 状态 |
|----------|----------|------|
| `src/rust/mcp/tools/memory/manager.rs` | Task 1, Task 2, Task 3 | ⚠️ 需协调 |
| `src/rust/mcp/tools/memory/fts_index.rs` | Task 1 | ✅ 唯一 |
| `src/rust/mcp/commands.rs` | Task 2 | ✅ 唯一 |
| `src/rust/mcp/tools/memory/summary_service.rs` | Task 3 | ✅ 唯一（新建） |
| `src/frontend/components/tools/MemorySearch.vue` | Task 4 | ✅ 唯一 |
| `src/frontend/composables/useMemorySearch.ts` | Task 4 | ✅ 唯一（新建） |
| `src/frontend/components/tools/MemoryWorkspace.vue` | Task 5, Task 6 | ⚠️ 需协调 |
| `src/frontend/composables/useProgressiveDisclosure.ts` | Task 5 | ✅ 唯一 |
| `src/frontend/components/tools/SnapshotDiff.vue` | Task 6 | ✅ 唯一 |

**冲突解决方案**：
- `manager.rs`：Task 1 → Task 2 → Task 3 串行执行（Task 1 添加 FTS5 双写，Task 2 修改搜索逻辑，Task 3 添加摘要生成）
- `MemoryWorkspace.vue`：Task 5 → Task 6 串行执行（Task 5 实现三态交互，Task 6 在 Detail 态添加历史按钮）

## 并行分组

### Layer 1 (并行执行)
- **Task 1**: FTS5 后端双写集成
- **Task 3**: 记忆摘要生成服务
- **Task 6**: Snapshot Diff 完整集成

### Layer 2 (依赖 Layer 1)
- **Task 2**: FTS5 搜索路由与降级（依赖 Task 1）
- **Task 5**: 渐进式披露三态交互（依赖 Task 3）

### Layer 3 (依赖 Layer 2)
- **Task 4**: FTS5 前端搜索集成（依赖 Task 2）

## 与 team-exec 的衔接

- 计划确认后运行：`/ccg:team-exec`
- team-exec 将按 Layer 顺序 spawn Builder
- Layer 1 的 3 个任务可并行执行（文件范围无冲突）
- Layer 2 的 2 个任务可并行执行（依赖 Layer 1 完成）
- Layer 3 的 1 个任务依赖 Layer 2 完成

## 验收标准（P2 Done Definition）

- [ ] CRUD 后 FTS 索引可用；FTS 异常时搜索可自动降级且用户无感失败
- [ ] >500 字内容新增时自动生成摘要，增强失败时有规则摘要兜底
- [ ] Snapshot Diff 在主界面可触发、可查看、可回滚，回滚后 UI 与数据一致刷新
- [ ] 三态渐进式披露流畅切换，850px 窗口下布局不溢出
- [ ] 所有功能通过单元测试和集成测试

## 双模型分析元数据

- **Codex SESSION_ID**: 019c77c0-27e1-7ac3-a248-aacee8888b4a
- **Gemini SESSION_ID**: 361444d8-a150-4fad-a051-a78e85c43aa2
- **分析状态**: SUCCESS（双模型均成功）
- **分析时间**: 2026-02-20
