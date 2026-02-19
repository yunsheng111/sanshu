# 上下文检索优化 -- OpenSpec 归档

**归档日期**: 2026-02-19
**任务名称**: context-retrieval
**OpenSpec 周期**: spec-research -> spec-plan -> spec-impl -> spec-review

---

## 归档元数据

| 字段 | 值 |
|------|-----|
| 约束集来源 | `.doc/agent-teams/research/20260218-context-retrieval-research.md` (v3) |
| 计划文件 | `.doc/spec/plans/20260219-context-retrieval-plan.md` |
| 实施报告 | `.doc/spec/reviews/20260219-context-retrieval-impl-report.md` |
| 审查报告 | `.doc/spec/reviews/20260219-context-retrieval-review.md` |
| 审查结论 | 有条件通过 (Conditional Pass) -> 报告修正后通过 |
| 归档操作者 | Claude (spec-review-agent) |

---

## 约束集摘要

### 硬约束 (HC-1 ~ HC-15)

| ID | 描述 | 最终状态 |
|----|------|----------|
| HC-1 | 存储架构（JSON） | 合规 |
| HC-2 | 记忆 CRUD | 合规 |
| HC-3 | 双模式检索 | 合规 (已有) |
| HC-4 | MCP 规范 | 合规 |
| HC-5 | 并发保护 | 合规 |
| HC-6 | 统一错误分类 | 合规 |
| HC-7 | 搜索缓存 | 合规 |
| HC-8 | 记忆 UI | 合规 |
| HC-9 | 密钥安全 | 合规 |
| HC-10 | 记忆大小限制 | 合规 |
| HC-11 | 后端测试 | 合规 |
| HC-12 | A11y 可访问性 | 部分合规 (MemoryList.vue 缺 ARIA) |
| HC-13 | i18n 国际化 | 合规 |
| HC-14 | 前端测试 | 部分合规 (框架已搭建，覆盖面窄) |
| HC-15 | 资源上限 | 合规 |

### 软约束 (SC-1 ~ SC-20)

| ID | 描述 | 最终状态 |
|----|------|----------|
| SC-1 | 上下文扩展（AST） | 延后 |
| SC-2 | Token 预算 | 延后 |
| SC-3 | 增量索引 | 合规 (已有) |
| SC-4 | 多语言 AST | 延后 |
| SC-5 | 记忆更新 | 部分合规 (MCP 层缺 "更新" action) |
| SC-6 | 记忆版本控制 | 合规 |
| SC-7 | Embedding 相似度 | 合规 |
| SC-8 | 磁盘缓存 | 合规 |
| SC-9 | 搜索 UI 预览 | 合规 |
| SC-10 | 搜索实时反馈 | 合规 |
| SC-11 | 响应式设计 | 合规 |
| SC-12 | 状态中心 | 延后 |
| SC-13 | 配置热更新 | 合规 |
| SC-14 | 可观测性 | 合规 |
| SC-15 | 数据迁移 | 合规 |
| SC-16 | 前端状态持久化 | 合规 |
| SC-17 | IPC 弹性 | 合规 |
| SC-18 | Worker 化 | 延后 |
| SC-19 | Skill 安全 | 合规 |
| SC-20 | 配置恢复 | 合规 |

---

## 计划执行摘要

- **计划步骤**: 32 步 (P0: 10, P1: 8, P2: 8, P3: 6)
- **完成步骤**: 32/32 (100%)
- **新增文件**: 17 个
- **修改文件**: 18 个
- **编译状态**: 通过 (4 个 dead_code 警告)
- **测试状态**: 170 个主库测试通过，0 失败

---

## 审查结论摘要

### Critical 问题（已修复）

| # | 描述 | 修复状态 |
|---|------|----------|
| C1 | 实施报告 HC 数量从 15 压缩为 6 | 已修正：恢复 15 个 HC 完整映射 |
| C2 | 步骤编号与计划不对应 | 已修正：按计划步骤编号重组 |

### Warning 问题（遗留）

| # | 描述 | 状态 | 建议迭代 |
|---|------|------|----------|
| W2 | MemoryList.vue 缺少 ARIA 属性 | 遗留 | 下一迭代 |
| W3 | 前端测试覆盖面窄 | 遗留 | 下一迭代 |
| W4 | MCP 层缺 "更新" action | 遗留 | 下一迭代 |

### Info 问题

| # | 描述 |
|---|------|
| I1 | 报告曾误标 SC-7/SC-16 为"延迟"（已修正） |
| I2 | 4 个 dead_code 编译警告 |
| I3 | SearchPreview.vue 使用 v-html 的轻微 XSS 风险 |

---

## 核心代码变更索引

### 后端 Rust

| 文件 | 关键变更 |
|------|----------|
| `src/rust/mcp/tools/memory/manager.rs` | SharedMemoryManager, 原子写入, 快照/回滚, 大小限制 |
| `src/rust/mcp/tools/memory/types.rs` | MemoryEntryStatus, version, snapshots, 大小限制配置 |
| `src/rust/mcp/tools/memory/similarity.rs` | cosine_similarity() 向量相似度 |
| `src/rust/mcp/tools/memory/migration.rs` | 版本迁移框架 "1.0"->"2.0"->"2.1" |
| `src/rust/mcp/utils/errors.rs` | McpToolError 6 种变体 + is_retryable + should_degrade |
| `src/rust/mcp/tools/acemcp/cache.rs` | 三层缓存 (内存->磁盘->API) |
| `src/rust/mcp/tools/acemcp/mcp.rs` | retry_request 适配 McpToolError |
| `src/rust/mcp/tools/acemcp/local_index.rs` | MAX_INDEX_SIZE_BYTES=500MB |
| `src/rust/mcp/tools/enhance/cache.rs` | LRU + TTL (10min) 缓存 |
| `src/rust/mcp/tools/icon/api.rs` | MAX_CACHE_ENTRIES=200 |
| `src/rust/mcp/tools/skills/mod.rs` | MAX_STDOUT_BYTES=1MB, 30s超时, 路径穿越检测 |
| `src/rust/mcp/hot_reload.rs` | 配置热更新缓存 (5秒刷新) |
| `src/rust/mcp/metrics.rs` | McpMetrics P50/P95/P99 (含边界保护) |
| `src/rust/config/keyring.rs` | SecureKeyStore 系统凭据管理器 |
| `src/rust/config/storage.rs` | 配置损坏恢复 (.json.corrupted.bak) |

### 前端 Vue/TypeScript

| 文件 | 关键变更 |
|------|----------|
| `src/frontend/components/tools/MemoryList.vue` | 记忆列表 (分页+过滤+编辑+删除) |
| `src/frontend/components/tools/MemorySearch.vue` | 记忆搜索 (ARIA+高亮) |
| `src/frontend/components/tools/SearchPreview.vue` | 搜索预览 (响应式+无障碍) |
| `src/frontend/composables/useSafeInvoke.ts` | IPC 超时保护 |
| `src/frontend/composables/useSearchFeedback.ts` | 搜索阶段状态 |
| `src/frontend/i18n/` | Vue i18n 中/英文 |
| `src/frontend/stores/searchStore.ts` | Pinia + localStorage 持久化 |
| `vitest.config.ts` | 前端测试基础设施 |

---

## 后续待办（从本次审查遗留）

1. **P0**: MCP `mcp.rs` 添加 "更新" action (SC-5 完整合规)
2. **P0**: `MemoryList.vue` 添加 ARIA 属性 (HC-12 完整合规)
3. **P1**: 前端核心组件单元测试扩展 (HC-14 完整合规)
4. **P2**: SearchPreview.vue v-html 安全加固
5. **P2**: dead_code 警告清理

---

**归档时间**: 2026-02-19
**归档操作者**: Claude (spec-review-agent)
**归档状态**: 完成
