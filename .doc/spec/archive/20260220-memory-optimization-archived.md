# OpenSpec 归档：记忆管理系统全面优化 (memory-optimization)

> 归档日期：2026-02-20
> 任务：记忆管理系统（ji 工具）全面优化
> 状态：审查通过，已归档
> OpenSpec 周期：spec-research -> spec-plan -> spec-impl -> spec-review

---

## 归档摘要

### 任务概述

对三术 (sanshu) 记忆管理系统进行全面优化，涵盖 P0-P3 四个优先级共 32 个实施步骤，新增 7 个 Rust 模块、8 个 Vue 前端组件、2 个 composables，以及配套的测试和基础设施。

### 关键成果

| 能力 | 优先级 | 状态 |
|------|--------|------|
| Write Guard 写入前拦截 | P0 | 已完成，三级判定（NOOP/UPDATE/ADD） |
| MemoryManagerRegistry 全局池 | P0 | 已完成，Weak<RwLock> + TTL + 池大小限制 |
| 数据模型 v2.2 | P0 | 已完成，6 个新字段 Option<T> + serde(default) |
| URI 路径分类体系 | P1 | 已完成，domain://path/segments 格式 |
| Vitality Decay 活力衰减 | P1 | 已完成，指数衰减 + Rule 豁免 + 用户确认清理 |
| FTS5 全文搜索索引 | P2 | 已完成，JSON 为 source of truth |
| 记忆摘要自动生成 | P2 | 已完成，规则引擎降级 |
| 前端树形浏览 + 渐进式披露 | P1-P2 | 已完成（C1 修复后可见） |
| 批量操作 + 快照对比 | P2 | 已完成（C1 修复后可见） |

### 约束合规

- 硬约束 HC-10~19：**全部合规**（10/10）
- 软约束 SC-4~26：**12/14 合规**（SC-17 前缀搜索、SC-24 意图识别未实现，Warning 级）
- 依赖约束 DEP-01~08：**全部合规**（8/8）
- 风险约束 RISK-01~10：**8 已缓解 / 2 部分缓解**（RISK-01 定时校验、RISK-04 前端审查视图未实现）

### 审查修复记录

| 问题 | 严重程度 | 修复内容 | 验证方式 |
|------|----------|----------|----------|
| C1: 8 个新组件未集成 | Critical | McpToolsTab.vue 导入 MemoryManager 替代 MemoryConfig + InjectionKey 提取到 memoryKeys.ts | `pnpm build` PASS |
| C2: 主界面 projectRootPath 为 null | Critical | 新增 get_current_dir Tauri 命令 + AppContent.vue fallbackProjectPath 降级 | `cargo check` PASS + `pnpm build` PASS |
| 额外: script setup export 错误 | Critical（构建阻断） | 从 MemoryManager.vue 提取 InjectionKey 到 memoryKeys.ts | `pnpm build` PASS |

### 遗留 Warning（建议后续迭代处理）

1. W1/W2: 前缀搜索语法 + 意图识别（SC-17, SC-24）
2. W9: fallback 路径语义改进（递归查找 .git）
3. W10: C1/C2 的直接自动化测试
4. W3/W4: RISK-01 定时校验 + RISK-04 被拒绝记忆审查视图
5. W5-W8: VitalityBadge max 值、summary.rs 字节长度、migration 版本号、文档更新

---

## 源文件索引

### 1. 约束集
- 路径：`.doc/spec/constraints/20260219-memory-optimization-constraints.md`
- 内容：HC-10~19 硬约束、SC-4~26 软约束、DEP-01~08 依赖约束、RISK-01~10 风险约束

### 2. 提案
- 路径：`.doc/spec/proposals/20260219-memory-optimization-proposal.md`
- 内容：问题陈述、4 个研究来源整合、P0-P3 实施方案、双模型分析结果

### 3. 计划
- 路径：`.doc/spec/plans/20260219-memory-optimization-plan.md`
- 内容：32 步零决策计划、前置条件检查清单、每步约束映射

### 4. 实施报告
- 路径：`.doc/spec/reviews/20260219-memory-optimization-impl-report.md`
- 内容：22 个后端步骤完成记录、38 个测试通过、10 个前端步骤状态

### 5. 审查报告（最终版）
- 路径：`.doc/spec/reviews/20260220-memory-optimization-review.md`
- 内容：双模型交叉审查（Codex + Gemini）、C1/C2 修复验证、约束合规矩阵、裁决通过

---

## 变更文件清单

### 新增文件（后端 Rust）
- `src/rust/mcp/tools/memory/write_guard.rs` -- Write Guard 写入拦截
- `src/rust/mcp/tools/memory/registry.rs` -- MemoryManager 全局池
- `src/rust/mcp/tools/memory/uri_path.rs` -- URI 路径解析/验证
- `src/rust/mcp/tools/memory/vitality.rs` -- Vitality Decay 活力衰减
- `src/rust/mcp/tools/memory/fts_index.rs` -- FTS5 全文搜索索引
- `src/rust/mcp/tools/memory/summary.rs` -- 记忆摘要自动生成
- `src/rust/mcp/tools/memory/observation_store.rs` -- 会话工具观察自动捕获

### 新增文件（前端 Vue）
- `src/frontend/components/tools/MemoryManager.vue` -- 记忆管理器主容器
- `src/frontend/components/tools/DomainTree.vue` -- 域树导航
- `src/frontend/components/tools/MemoryWorkspace.vue` -- 记忆工作区
- `src/frontend/components/tools/TagFilter.vue` -- 标签筛选
- `src/frontend/components/tools/VitalityBadge.vue` -- 活力值徽章
- `src/frontend/components/tools/SnapshotDiff.vue` -- 快照对比
- `src/frontend/components/tools/BatchActionBar.vue` -- 批量操作栏
- `src/frontend/components/tools/memoryKeys.ts` -- InjectionKey 定义（审查修复新增）
- `src/frontend/composables/useProgressiveDisclosure.ts` -- 渐进式披露
- `src/frontend/composables/useVitalityDecay.ts` -- 活力衰减计算

### 修改文件
- `src/rust/mcp/tools/memory/types.rs` -- v2.2 数据模型
- `src/rust/mcp/tools/memory/manager.rs` -- Write Guard + URI + 活力集成
- `src/rust/mcp/tools/memory/mcp.rs` -- 5 个新 MCP 操作
- `src/rust/mcp/tools/memory/migration.rs` -- v2.2 迁移链
- `src/rust/mcp/tools/memory/dedup.rs` -- 去重增强
- `src/rust/mcp/tools/memory/mod.rs` -- 模块声明
- `src/rust/mcp/commands.rs` -- 前端命令注册
- `src/rust/mcp/types.rs` -- MCP 类型扩展
- `src/rust/ui/commands.rs` -- 新增 get_current_dir（审查修复）
- `src/rust/app/builder.rs` -- 注册 get_current_dir（审查修复）
- `src/frontend/components/tabs/McpToolsTab.vue` -- MemoryManager 集成（审查修复）
- `src/frontend/components/AppContent.vue` -- fallbackProjectPath（审查修复）
- `src/frontend/components/tools/MemorySearch.vue` -- 搜索增强

---

## 双模型审查元数据

| 阶段 | Codex SESSION | Gemini SESSION | 状态 |
|------|---------------|----------------|------|
| 约束研究 (spec-research) | 019c75e4-93db-75d0-81da-2d80630282a8 | e60f4a6b-1c88-4691-aa55-1b340335ec11 | SUCCESS |
| 二次审查 (spec-review) | 019c76e1-600b-71a1-9d5b-c8cff3e77463 | 3aa20c75-365d-42d5-9e2b-67cc40dd17f1 | SUCCESS |

---

## 归档确认

- 归档操作人：Claude (spec-review-agent)
- 归档时间：2026-02-20
- 用户确认：通过 zhi 交互确认归档
- 审查裁决：**通过**（0 Critical, 10 Warning, 6 Info）
