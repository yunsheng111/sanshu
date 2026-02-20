# Team Plan: 记忆管理系统完整实施计划

> **日期**: 2026-02-20
> **状态**: Agent Teams 规划产出
> **来源**: 合并约束集 + P2 计划
> **方法**: 主代理综合分析（外部模型调用失败，降级为单模型规划）

---

## 概述

为记忆管理系统（ji 工具）实施剩余功能，覆盖 P2-P3 阶段的所有硬约束（HC-01 ~ HC-07）。

**核心目标**:
1. 打通后端模块到 MCP 接口的集成链路
2. 激活前端组件并完成 Tauri IPC 调用
3. 补全测试覆盖和文档

**完成度**: 当前 85% → 目标 100%

---

## 架构决策

### 后端集成策略

**FTS5 双写模型**:
- 主存储（JSON）先成功，再执行 `FtsIndex::sync_entry/delete_entry`
- 索引失败记录 `index_dirty` 并告警，搜索时自动降级到模糊匹配
- 启动时执行 `reconcile_index()` 修复不一致

**搜索路由策略**:
- 默认 FTS5 优先，失败或空结果降级到模糊匹配
- 返回结果包含 `search_mode` 字段标注使用的搜索方式

**摘要生成策略**:
- 在 `add_memory` 内触发，条件 `content.len() > 500 && summary.is_none()`
- 采用"增强链路 + 规则引擎兜底"，超时 5 秒

**活力衰减触发**:
- 每次 `recall_memories()` 自动提升活力值
- 定期后台任务计算衰减（可选，P3 阶段）

### 前端集成策略

**三态渐进式披露模型**:
- **Collapsed (28px)**: 标题 + 标签 + 活力值徽章
- **Expanded (80-120px)**: 显示 `summary` 字段，限制 3 行
- **Detail (Auto)**: 完整 Markdown 内容 + 操作按钮

**850px 布局策略**:
- 左侧 `DomainTree` 220px（可折叠）
- 右侧 `MemoryWorkspace` 630px
- 底部 `BatchActionBar` 固定（仅批量模式显示）

**Snapshot Diff 集成**:
- 使用 `n-modal` 挂载，避免挤压列表空间
- 回滚后局部刷新当前条目，不刷新整个列表

---

## 任务列表

### Layer 1: 后端核心集成（并行执行）

#### Task 1: FTS5 后端双写集成
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/manager.rs` (修改)
  - `src/rust/mcp/tools/memory/fts_index.rs` (已存在)
- **依赖**: 无
- **预估工时**: 4h
- **实施步骤**:
  1. 在 `MemoryManager::add_memory()` 成功写入后调用 `FtsIndex::sync_entry(entry)`
  2. 在 `MemoryManager::update_memory()` 成功更新后调用 `FtsIndex::sync_entry(updated_entry)`
  3. 在 `MemoryManager::delete_memory()` 成功删除后调用 `FtsIndex::delete_entry(memory_id)`
  4. 在 `MemoryManager::new()` 中初始化 `FtsIndex`，捕获异常并设置 `fts_available` 标志
  5. 添加 `reconcile_index()` 方法，启动时执行一次索引一致性校验
- **验收标准**:
  - [ ] CRUD 操作后 FTS5 索引自动同步
  - [ ] FTS5 初始化失败不阻塞主流程
  - [ ] 启动时自动修复索引不一致

#### Task 2: MCP 接口新增操作
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/mcp.rs` (修改)
  - `src/rust/mcp/tools/memory/manager.rs` (新增方法)
- **依赖**: 无
- **预估工时**: 6h
- **实施步骤**:
  1. 在 `manager.rs` 中实现 5 个新方法:
     - `get_domain_list() -> Vec<DomainInfo>`
     - `get_cleanup_candidates() -> Vec<CleanupCandidate>`
     - `get_vitality_trend(memory_id) -> VitalityTrend`
     - `get_memory_snapshots(memory_id) -> Vec<MemorySnapshot>`
     - `rollback_to_snapshot(memory_id, version) -> Result<()>`
  2. 在 `mcp.rs` 的 `match request.action.as_str()` 中添加 5 个分支
  3. 为每个操作添加参数验证和错误处理
- **验收标准**:
  - [ ] 5 个新操作在 `mcp.rs` 中实现
  - [ ] 每个操作有对应的单元测试
  - [ ] MCP 调用返回正确的 JSON 格式

#### Task 3: 记忆摘要生成触发
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/manager.rs` (修改)
  - `src/rust/mcp/tools/memory/summary_service.rs` (已存在)
- **依赖**: 无
- **预估工时**: 3h
- **实施步骤**:
  1. 在 `MemoryManager::add_memory()` 中，当 `content.len() > 500 && summary.is_none()` 时调用摘要生成
  2. 调用 `SummaryService::generate_summary(content)` 获取摘要
  3. 设置超时（5 秒），超时则使用规则引擎兜底
  4. 将生成的摘要写入 `entry.summary` 字段
- **验收标准**:
  - [ ] >500 字内容自动生成摘要
  - [ ] 增强工具失败时规则引擎兜底
  - [ ] 摘要生成不阻塞写入流程（超时保护）

#### Task 4: 活力衰减自动触发
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/manager.rs` (修改)
  - `src/rust/mcp/tools/memory/vitality.rs` (已存在)
- **依赖**: 无
- **预估工时**: 3h
- **实施步骤**:
  1. 在 `MemoryManager::recall_memories()` 中，对每个返回的记忆调用 `VitalityEngine::boost_vitality(entry, &config)`
  2. 更新 `entry.last_accessed_at` 时间戳
  3. 调用 `self.save()` 保存更新后的活力值
  4. 添加日志记录活力值变化
- **验收标准**:
  - [ ] 每次回忆操作自动提升活力值
  - [ ] 活力值变化正确保存到 JSON
  - [ ] 添加集成测试验证衰减逻辑

---

### Layer 2: 搜索与前端基础（依赖 Layer 1）

#### Task 5: FTS5 搜索路由与降级
- **类型**: 后端
- **文件范围**:
  - `src/rust/mcp/tools/memory/manager.rs` (修改)
- **依赖**: Task 1
- **预估工时**: 4h
- **实施步骤**:
  1. 修改 `search_memories()` 方法，优先调用 `FtsIndex::search(query, limit)`
  2. 若 FTS5 返回结果，按 ID 批量获取完整记忆条目
  3. 若 FTS5 失败或返回空，降级到现有模糊匹配逻辑
  4. 添加 `search_mode` 字段到返回结果，标注使用的搜索方式（`fts5` / `fuzzy`）
- **验收标准**:
  - [ ] FTS5 可用时搜索结果准确且快速
  - [ ] FTS5 不可用时自动降级到模糊匹配
  - [ ] 搜索结果包含搜索模式标识

#### Task 6: Tauri 命令注册
- **类型**: 后端
- **文件范围**:
  - `src/rust/ui/commands.rs` (修改)
  - `src/rust/app/builder.rs` (修改)
- **依赖**: Task 2
- **预估工时**: 4h
- **实施步骤**:
  1. 在 `commands.rs` 中添加 6 个 Tauri 命令:
     - `get_domain_list(project_path: String) -> Result<Vec<DomainInfo>, String>`
     - `delete_empty_domain(project_path: String, domain: String) -> Result<(), String>`
     - `get_cleanup_candidates(project_path: String) -> Result<Vec<CleanupCandidate>, String>`
     - `cleanup_memories(project_path: String, ids: Vec<String>) -> Result<(), String>`
     - `get_memory_snapshots(project_path: String, memory_id: String) -> Result<Vec<MemorySnapshot>, String>`
     - `rollback_to_snapshot(project_path: String, memory_id: String, version: u32) -> Result<(), String>`
  2. 每个命令内部调用对应的 MCP 操作
  3. 在 `builder.rs` 的 `.invoke_handler()` 中注册这 6 个命令
- **验收标准**:
  - [ ] 6 个命令在 `commands.rs` 中实现
  - [ ] 命令在 `builder.rs` 中注册
  - [ ] 前端调用不报 "command not found" 错误

#### Task 7: FTS5 前端搜索集成
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemorySearch.vue` (修改)
  - `src/frontend/composables/useMemorySearch.ts` (新建)
- **依赖**: Task 5
- **预估工时**: 4h
- **实施步骤**:
  1. 创建 `useMemorySearch.ts` composable，封装搜索逻辑
  2. 实现 FTS5 搜索调用：`invoke('search_memories', { query, limit: 20 })`
  3. 实现降级逻辑：捕获异常后调用 `invoke('get_memories', { filter: query })`
  4. 在 `MemorySearch.vue` 中使用 `useMemorySearch`，替换现有搜索逻辑
  5. 添加搜索模式指示器（FTS5 / 模糊匹配）
- **验收标准**:
  - [ ] 搜索结果准确且快速
  - [ ] FTS5 失败时自动降级无感知
  - [ ] UI 显示当前搜索模式

---

### Layer 3: 前端组件激活（依赖 Layer 2）

#### Task 8: 渐进式披露三态交互
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemoryWorkspace.vue` (修改)
  - `src/frontend/composables/useProgressiveDisclosure.ts` (已存在)
- **依赖**: Task 3
- **预估工时**: 4h
- **实施步骤**:
  1. 在 `useProgressiveDisclosure.ts` 中定义三态枚举：`collapsed` / `expanded` / `detail`
  2. 在 `MemoryWorkspace.vue` 中为每个记忆卡片维护 `disclosureState`
  3. 实现三态视图切换：
     - Collapsed：标题 + 标签 + VitalityBadge（高度 28px）
     - Expanded：summary 字段 + 3 行 line-clamp（高度 80-120px）
     - Detail：完整 Markdown 内容 + 操作按钮（高度自适应）
  4. 添加点击/双击事件切换状态
- **验收标准**:
  - [ ] 三态切换流畅无卡顿
  - [ ] 摘要显示正确（有摘要显示摘要，无摘要显示前 100 字）
  - [ ] 850px 窗口下布局不溢出

#### Task 9: 前端组件集成到主容器
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemoryManager.vue` (修改)
  - `src/frontend/components/tools/DomainTree.vue` (已存在)
  - `src/frontend/components/tools/VitalityBadge.vue` (已存在)
  - `src/frontend/components/tools/BatchActionBar.vue` (已存在)
- **依赖**: Task 6, Task 8
- **预估工时**: 6h
- **实施步骤**:
  1. 在 `MemoryManager.vue` 中添加左侧边栏布局（`n-layout-sider`）
  2. 嵌入 `DomainTree` 组件，绑定 `@select` 事件筛选记忆
  3. 在 `MemoryWorkspace` 的记忆卡片中嵌入 `VitalityBadge` 组件
  4. 在底部添加 `BatchActionBar` 组件（使用 `n-affix` 固定）
  5. 实现批量模式切换逻辑（多选 checkbox）
  6. 使用 `provide/inject` 共享状态（当前域、批量选择列表）
- **验收标准**:
  - [ ] 4 个组件在主容器中可见
  - [ ] 域树点击可筛选记忆
  - [ ] 活力徽章正确显示颜色编码
  - [ ] 批量操作条在批量模式下显示

#### Task 10: Snapshot Diff 完整集成
- **类型**: 前端
- **文件范围**:
  - `src/frontend/components/tools/MemoryWorkspace.vue` (修改)
  - `src/frontend/components/tools/SnapshotDiff.vue` (已存在)
- **依赖**: Task 6
- **预估工时**: 4h
- **实施步骤**:
  1. 在 `MemoryWorkspace.vue` 的 Detail 态添加"查看历史"按钮（时钟图标）
  2. 实现 `handleViewHistory(item)` 方法：
     - 调用 `invoke('get_memory_snapshots', { projectPath, memoryId: item.id })`
     - 打开 `n-modal`，传递 snapshots 数据给 `SnapshotDiff`
  3. 在 `SnapshotDiff.vue` 中添加版本选择器（n-select）
  4. 实现回滚确认对话框（n-modal + n-button）
  5. 回滚成功后：
     - 调用 `invoke('rollback_to_snapshot', { projectPath, memoryId, targetVersion })`
     - 关闭 Modal
     - 刷新当前记忆条目（局部更新，不刷新整个列表）
- **验收标准**:
  - [ ] "查看历史"按钮可见且可点击
  - [ ] Snapshot Diff 正确显示版本对比
  - [ ] 回滚成功后列表数据正确更新
  - [ ] 850px 窗口下 Modal 布局合理

---

### Layer 4: 测试与文档（依赖 Layer 3）

#### Task 11: 前端组件单元测试
- **类型**: 测试
- **文件范围**:
  - `src/frontend/components/tools/__tests__/DomainTree.spec.ts` (新建)
  - `src/frontend/components/tools/__tests__/VitalityBadge.spec.ts` (新建)
  - `src/frontend/components/tools/__tests__/BatchActionBar.spec.ts` (新建)
  - `src/frontend/components/tools/__tests__/SnapshotDiff.spec.ts` (新建)
  - `src/frontend/components/tools/__tests__/MemoryWorkspace.spec.ts` (新建)
- **依赖**: Task 9, Task 10
- **预估工时**: 8h
- **实施步骤**:
  1. 为每个组件创建对应的 `.spec.ts` 文件
  2. 使用 vitest + happy-dom 编写单元测试
  3. 测试覆盖：
     - 组件挂载和渲染
     - Props 传递和验证
     - 事件触发和响应
     - 状态变化和 UI 更新
  4. 运行 `pnpm vitest --coverage` 确保覆盖率 > 80%
- **验收标准**:
  - [ ] 5 个组件有对应的 `.spec.ts` 文件
  - [ ] 测试覆盖率 > 80%
  - [ ] 所有测试通过

#### Task 12: 后端集成测试
- **类型**: 测试
- **文件范围**:
  - `src/rust/mcp/tools/memory/integration_tests.rs` (新建)
- **依赖**: Task 1, Task 2, Task 3, Task 4, Task 5
- **预估工时**: 6h
- **实施步骤**:
  1. 创建 `integration_tests.rs` 模块
  2. 编写集成测试覆盖：
     - FTS5 搜索路径（添加记忆 → 搜索 → 验证结果）
     - 活力衰减触发（添加记忆 → 回忆 → 验证活力值提升）
     - 摘要自动生成（添加长记忆 → 验证摘要字段）
     - Snapshot 回滚（添加记忆 → 更新 → 回滚 → 验证内容）
     - MCP 操作调用（调用 5 个新操作 → 验证返回值）
  3. 使用 `tempfile` 创建临时测试目录
  4. 运行 `cargo test --package sanshu --lib mcp::tools::memory::integration_tests`
- **验收标准**:
  - [ ] 5 个关键路径有集成测试
  - [ ] 所有测试通过
  - [ ] 测试覆盖 FTS5 降级场景

---

## 文件冲突检查

| 文件路径 | 归属任务 | 冲突状态 | 解决方案 |
|----------|----------|----------|----------|
| `src/rust/mcp/tools/memory/manager.rs` | Task 1, 3, 4, 5 | ⚠️ 高冲突 | 串行执行：Task 1 → Task 3 → Task 4 → Task 5 |
| `src/rust/mcp/tools/memory/mcp.rs` | Task 2 | ✅ 唯一 | 无冲突 |
| `src/rust/ui/commands.rs` | Task 6 | ✅ 唯一 | 无冲突 |
| `src/rust/app/builder.rs` | Task 6 | ✅ 唯一 | 无冲突 |
| `src/frontend/components/tools/MemoryWorkspace.vue` | Task 8, 9, 10 | ⚠️ 中冲突 | 串行执行：Task 8 → Task 9 → Task 10 |
| `src/frontend/components/tools/MemoryManager.vue` | Task 9 | ✅ 唯一 | 无冲突 |
| `src/frontend/components/tools/MemorySearch.vue` | Task 7 | ✅ 唯一 | 无冲突 |

**冲突解决方案**:
- **manager.rs**: Task 1 → Task 3 → Task 4 → Task 5 串行执行（每个任务修改不同方法）
- **MemoryWorkspace.vue**: Task 8 → Task 9 → Task 10 串行执行（Task 8 实现三态，Task 9 嵌入组件，Task 10 添加历史按钮）

---

## 并行分组

### Layer 1 (并行执行 - 4 个任务)
- **Task 1**: FTS5 后端双写集成 (4h)
- **Task 2**: MCP 接口新增操作 (6h)
- **Task 3**: 记忆摘要生成触发 (3h)
- **Task 4**: 活力衰减自动触发 (3h)

**Layer 1 总工时**: 6h（最长任务）

### Layer 2 (依赖 Layer 1 - 3 个任务)
- **Task 5**: FTS5 搜索路由与降级 (4h) - 依赖 Task 1
- **Task 6**: Tauri 命令注册 (4h) - 依赖 Task 2
- **Task 7**: FTS5 前端搜索集成 (4h) - 依赖 Task 5

**Layer 2 总工时**: 4h（Task 5 和 Task 7 串行，Task 6 并行）

### Layer 3 (依赖 Layer 2 - 3 个任务)
- **Task 8**: 渐进式披露三态交互 (4h) - 依赖 Task 3
- **Task 9**: 前端组件集成到主容器 (6h) - 依赖 Task 6, Task 8
- **Task 10**: Snapshot Diff 完整集成 (4h) - 依赖 Task 6

**Layer 3 总工时**: 10h（Task 8 → Task 9 串行，Task 10 并行）

### Layer 4 (依赖 Layer 3 - 2 个任务)
- **Task 11**: 前端组件单元测试 (8h) - 依赖 Task 9, Task 10
- **Task 12**: 后端集成测试 (6h) - 依赖 Task 1-5

**Layer 4 总工时**: 8h（并行执行）

**总工时**: 6h + 4h + 10h + 8h = **28h (3.5 工作日)**

---

## 与 team-exec 的衔接

- 计划确认后运行：`/ccg:team-exec D:\CLIGUI\sanshu\.doc\agent-teams\plans\20260220-memory-complete-implementation-plan.md`
- team-exec 将按 Layer 顺序 spawn Builder
- Layer 1 的 4 个任务可并行执行（文件范围无冲突）
- Layer 2 的 Task 6 可与 Task 5/7 并行
- Layer 3 的 Task 10 可与 Task 8/9 并行
- Layer 4 的 2 个任务可并行执行

---

## 验收标准（Done Definition）

### 功能完整性
- [ ] **F1**: 用户可通过前端域树浏览记忆（点击域节点筛选记忆）
- [ ] **F2**: 用户可在记忆卡片上看到活力徽章（颜色编码：绿/黄/红）
- [ ] **F3**: 用户可使用 FTS5 全文搜索（输入关键词返回相关记忆）
- [ ] **F4**: 长记忆（>500 字符）自动生成摘要并显示在列表视图
- [ ] **F5**: 用户可查看记忆快照历史并回滚到旧版本
- [ ] **F6**: 用户可批量选择记忆并执行删除/导出操作
- [ ] **F7**: 低活力记忆自动出现在清理候选列表

### 性能指标
- [ ] **P1**: FTS5 搜索 1000 条记忆 < 50ms（P95）
- [ ] **P2**: 域树加载 < 200ms（包含 IPC 往返）
- [ ] **P3**: 活力值计算 < 10ms（单条记忆）
- [ ] **P4**: 批量删除 100 条记忆 < 500ms

### 测试覆盖
- [ ] **T1**: 后端单元测试覆盖率 > 85%（cargo tarpaulin）
- [ ] **T2**: 前端单元测试覆盖率 > 80%（vitest --coverage）
- [ ] **T3**: 集成测试覆盖 5 个关键路径
- [ ] **T4**: 所有测试通过（cargo test + pnpm vitest）

### 文档完整性
- [ ] **D1**: 用户文档说明新功能使用方法
- [ ] **D2**: API 文档列出新增 MCP 操作和 Tauri 命令
- [ ] **D3**: 开发者文档说明架构变更

---

## 关键风险与缓解

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| FTS5 集成失败导致搜索不可用 | HIGH | 保留模糊匹配降级路径（HC-18） |
| manager.rs 串行修改导致延迟 | MEDIUM | 优先完成 Task 1，其他任务依次进行 |
| 前端组件集成冲突 | MEDIUM | 使用 provide/inject 隔离状态 |
| Tauri 命令注册遗漏 | MEDIUM | 添加 E2E 测试覆盖所有命令 |
| 活力衰减计算错误 | LOW | 已有单元测试覆盖 |

---

## 开放问题（需用户确认）

### Q1: FTS5 中文分词方案
**问题**: 当前使用 `unicode61` 分词器，中文按字符分词，召回率可能不足

**选项**:
- A. 保持 `unicode61`（简单，无额外依赖）
- B. 集成 `jieba-rs`（更好的中文分词，增加依赖）

**建议**: 先使用 A，根据用户反馈决定是否升级到 B

### Q2: 活力衰减后台任务
**问题**: 当前活力衰减仅在访问时计算，未定期后台更新

**选项**:
- A. 保持当前懒计算（简单，无后台任务）
- B. 添加定期后台任务（每小时计算一次，更准确）

**建议**: 先使用 A，如果用户需要实时清理候选列表再升级到 B

### Q3: 批量操作事务性
**问题**: 批量删除失败时是否回滚已删除的记忆

**选项**:
- A. 部分成功（已删除的不回滚，报告失败数量）
- B. 全部回滚（任何失败都恢复所有记忆）

**建议**: 使用 A（更符合用户预期，避免意外恢复）

---

## 分析元数据

- **规划方法**: 主代理综合分析（外部模型调用失败，降级为单模型规划）
- **输入文档**:
  - 约束集：`20260220-memory-remaining-tasks-constraints.md`
  - P2 计划：`20260220-memory-p2-implementation-plan.md`
- **规划状态**: SUCCESS
- **规划时间**: 2026-02-20
- **预计完成时间**: 3.5 工作日（28 小时）
