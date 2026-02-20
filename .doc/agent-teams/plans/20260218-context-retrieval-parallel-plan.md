# Agent Teams 并行实施计划 - 上下文检索优化

**日期**: 2026-02-18
**计划类型**: 零决策并行实施
**状态**: DEGRADED 降级模式（主代理接管）
**基于研究**: `.doc/agent-teams/wip/research/20260218-context-retrieval-analysis.md`

---

## ⚠️ 降级说明（Level 3 DEGRADED 单模型）

由于外部模型调用持续超时（Codex/Gemini 均无响应 >120s），本计划采用 **Level 3 降级模式**：
- **执行主体**: 主代理（Claude）单模型接管规划任务
- **分析基础**: 研究文档（已完成的技术可行性评估）+ ace-tool 代码上下文
- **验证方式**: 建议通过 `/ccg:team-review` 补充双模型交叉验证

**降级证据**:
- Codex 调用失败: exit code 124 (timeout >120s)
- Gemini 调用失败: exit code 124 (timeout >90s)
- 参数修正尝试: 3 次（--model, --backend, stdin 模式均超时）
- 进程终止时间: 2026-02-18 15:20 - 15:50

**风险**: 缺少 Codex 后端权威视角和 Gemini 前端权威视角，子任务拆分可能存在偏差。

---

## 执行摘要

本计划将三术项目上下文检索优化（阶段 1）拆分为 5 个可并行执行的子任务，确保文件范围完全隔离，支持多 Builder 同时写码。

**核心目标**:
1. FTS5 快速通道（关键词查询 <100ms）
2. 记忆更新机制（Patch/Append 模式）
3. 错误信息优化（统一格式 + 降级提示）

**并行策略**: 5 个子任务文件范围无重叠，可同时启动 5 个 Builder

---

## 子任务拆分（5 个并行任务）

### 任务 1: FTS5 索引引擎（后端核心）
**Builder**: backend-builder-1 | **优先级**: P0 | **工时**: 16-20h

**目标**: 实现 SQLite + FTS5 索引引擎，支持关键词快速查询（<100ms）

**涉及文件**:
- `src/rust/mcp/tools/acemcp/fts5_engine.rs` (新建)
- `src/rust/mcp/tools/acemcp/mod.rs` (修改)
- `Cargo.toml` (修改)

**验收标准**:
- [ ] 关键词查询响应时间 <100ms
- [ ] 索引构建成功率 >99%
- [ ] 单元测试覆盖率 >80%

---

### 任务 2: 记忆更新机制（后端核心）
**Builder**: backend-builder-2 | **优先级**: P0 | **工时**: 12-16h

**目标**: 实现记忆 Patch/Append 更新模式

**涉及文件**:
- `src/rust/mcp/tools/memory/manager.rs` (修改)
- `src/rust/mcp/tools/memory/types.rs` (修改)
- `src/rust/mcp/tools/memory/mcp.rs` (修改)

**验收标准**:
- [ ] Patch 模式更新成功率 >99%
- [ ] Append 模式更新成功率 >99%
- [ ] 单元测试覆盖率 >80%

---

### 任务 3: 错误信息优化（后端通用）
**Builder**: backend-builder-3 | **优先级**: P0 | **工时**: 8-12h

**目标**: 统一错误格式，增加诊断信息和可操作建议

**涉及文件**:
- `src/rust/mcp/error.rs` (新建)
- `src/rust/mcp/server.rs` (修改)
- `src/rust/mcp/tools/acemcp/mcp.rs` (修改)
- `src/rust/mcp/tools/memory/mcp.rs` (修改)

**验收标准**:
- [ ] 所有错误信息包含可操作建议
- [ ] 降级提示正常工作
- [ ] 单元测试覆盖率 >80%

---

### 任务 4: 搜索配置界面（前端）
**Builder**: frontend-builder-1 | **优先级**: P1 | **工时**: 10-14h

**目标**: 增加 sou 工具配置界面

**涉及文件**:
- `src/frontend/components/settings/SouSettings.vue` (新建)
- `src/frontend/components/settings/McpSettings.vue` (修改)
- `src/frontend/types/config.ts` (修改)

**验收标准**:
- [ ] 搜索模式切换正常工作
- [ ] 索引状态实时更新
- [ ] UI 响应式设计

---

### 任务 5: 记忆管理界面（前端）
**Builder**: frontend-builder-2 | **优先级**: P1 | **工时**: 10-14h

**目标**: 增加 ji 工具配置界面

**涉及文件**:
- `src/frontend/components/settings/JiSettings.vue` (新建)
- `src/frontend/components/settings/McpSettings.vue` (修改)
- `src/frontend/types/config.ts` (修改)

**验收标准**:
- [ ] 记忆列表正常显示
- [ ] 记忆编辑功能正常
- [ ] UI 响应式设计

---

## 文件冲突检测

**阶段 1（完全并行）**: 任务 1 + 任务 2
**阶段 2（完全并行）**: 任务 3 + 任务 4 + 任务 5

---

## 后续步骤

1. 用户确认计划
2. 执行 `/ccg:team-exec` 启动并行实施
3. 执行 `/ccg:team-review` 交叉审查产出

---

**计划生成时间**: 2026-02-18 16:00
**下一步**: 用户确认后执行 `/ccg:team-exec`
