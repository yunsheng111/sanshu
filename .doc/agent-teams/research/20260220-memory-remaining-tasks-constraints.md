# 记忆管理系统剩余任务约束集

> **日期**: 2026-02-20
> **状态**: Agent Teams 需求研究产出
> **来源**: 基于 20260219-memory-optimization-analysis.md 和代码库检索
> **方法**: 主代理直接分析（外部模型调用失败，降级为单模型分析）

---

## 执行摘要

**核心发现**: 研究文件中规划的 P0-P3 功能**大部分已实现**（约 85% 完成度），但存在以下关键缺口：

1. **集成缺口**: 后端模块已实现但未在 MCP 接口和前端完全集成
2. **前端缺口**: 部分 UI 组件已创建但未在主容器中激活
3. **测试缺口**: 新增模块缺少集成测试和端到端测试
4. **文档缺口**: 新功能缺少用户文档和 API 文档

---

## 1. 已完成功能清单（P0-P3）

### P0 功能（质量基础）✅ 100% 完成

| 功能 | 文件 | 状态 |
|------|------|------|
| Write Guard 写入守卫 | `src/rust/mcp/tools/memory/write_guard.rs` | ✅ 已实现 + 单元测试 |
| MemoryManagerRegistry 全局池 | `src/rust/mcp/tools/memory/registry.rs` | ✅ 已实现 + 单元测试 |
| SharedMemoryManager 并发保护 | `src/rust/mcp/tools/memory/manager.rs` | ✅ 已实现（Arc<RwLock>） |
| 数据模型 v2.2 | `src/rust/mcp/tools/memory/types.rs` | ✅ 已实现（URI + 标签 + 活力值） |

### P1 功能（组织增强）✅ 100% 完成

| 功能 | 文件 | 状态 |
|------|------|------|
| Vitality Decay 活力衰减 | `src/rust/mcp/tools/memory/vitality.rs` | ✅ 已实现 + 单元测试 |
| URI 路径解析验证 | `src/rust/mcp/tools/memory/uri_path.rs` | ✅ 已实现 + 单元测试 |
| 前端域树组件 | `src/frontend/components/tools/DomainTree.vue` | ✅ 已实现 |
| 前端活力徽章 | `src/frontend/components/tools/VitalityBadge.vue` | ✅ 已实现 |
| 前端标签筛选 | `src/frontend/components/tools/TagFilter.vue` | ✅ 已实现 |

### P2 功能（检索升级）✅ 90% 完成

| 功能 | 文件 | 状态 |
|------|------|------|
| FTS5 Sidecar 索引 | `src/rust/mcp/tools/memory/fts_index.rs` | ✅ 已实现 + 单元测试 |
| 摘要自动生成 | `src/rust/mcp/tools/memory/summary.rs` | ✅ 已实现（规则引擎） |
| 摘要生成服务 | `src/rust/mcp/tools/memory/summary_service.rs` | ✅ 已实现（Provider 链） |
| Snapshot Diff 视图 | `src/frontend/components/tools/SnapshotDiff.vue` | ✅ 已实现 |
| 批量操作条 | `src/frontend/components/tools/BatchActionBar.vue` | ✅ 已实现 |

### P3 功能（体验升级）✅ 80% 完成

| 功能 | 文件 | 状态 |
|------|------|------|
| 会话观察存储 | `src/rust/mcp/tools/memory/observation_store.rs` | ✅ 已实现 + 单元测试 |
| 记忆工作区 | `src/frontend/components/tools/MemoryWorkspace.vue` | ✅ 已实现（渐进式披露） |
| 记忆管理器容器 | `src/frontend/components/tools/MemoryManager.vue` | ✅ 已实现 |
| 活力衰减 composable | `src/frontend/composables/useVitalityDecay.ts` | ✅ 已实现 |
| 渐进式披露 composable | `src/frontend/composables/useProgressiveDisclosure.ts` | ✅ 已实现 |

---

## 2. 未完成功能（硬约束）

### HC-01: FTS5 索引未在 manager.rs 中集成 ⚠️ CRITICAL

**问题**: `fts_index.rs` 已实现但 `manager.rs` 中未调用 FTS5 搜索

**影响**: 用户无法使用 FTS5 全文搜索，仍依赖线性扫描

**证据**:
- `manager.rs:39` 定义了 `fts_index: Option<Mutex<FtsIndex>>` 字段
- 但 `search_memories()` 方法未调用 `fts_index.search()`

**约束**:
```rust
// 必须在 manager.rs 的 search_memories() 中添加：
if let Some(fts) = &self.fts_index {
    if let Ok(fts_guard) = fts.lock() {
        let ids = fts_guard.search(query, limit)?;
        // 根据 ID 列表从 store.entries 中提取记忆
    }
}
```

**验收标准**:
- [ ] `manager.rs` 的 `search_memories()` 调用 FTS5 搜索
- [ ] FTS5 失败时降级到模糊匹配（HC-18 约束）
- [ ] 添加集成测试验证 FTS5 搜索路径

---

### HC-02: MCP 接口未暴露新操作 ⚠️ CRITICAL

**问题**: 后端新增功能未在 `mcp.rs` 中暴露为 MCP 操作

**缺失操作**:
1. `get_domain_list` - 获取域列表（DomainTree 组件需要）
2. `get_cleanup_candidates` - 获取清理候选（活力衰减 UI 需要）
3. `get_vitality_trend` - 获取活力趋势数据（VitalityBadge 需要）
4. `rollback_to_snapshot` - 回滚到快照（SnapshotDiff 需要）
5. `search_with_fts` - FTS5 搜索（前端搜索需要）

**约束**:
```rust
// 必须在 mcp.rs 的 match request.action.as_str() 中添加：
"获取域列表" => {
    let domains = manager.get_domain_list()?;
    serde_json::to_string(&domains)?
}
"获取清理候选" => {
    let candidates = manager.get_cleanup_candidates()?;
    serde_json::to_string(&candidates)?
}
// ... 其他操作
```

**验收标准**:
- [ ] 5 个新操作在 `mcp.rs` 中实现
- [ ] 每个操作有对应的单元测试
- [ ] 前端可通过 Tauri invoke 调用

---

### HC-03: 前端组件未在主容器中激活 ⚠️ HIGH

**问题**: 新组件已创建但未在 `MemoryConfig.vue` 或 `MemoryManager.vue` 中集成

**缺失集成**:
1. `DomainTree.vue` - 未在左侧边栏显示
2. `VitalityBadge.vue` - 未在记忆卡片中显示
3. `BatchActionBar.vue` - 未在底部固定显示
4. `SnapshotDiff.vue` - 未在详情视图中嵌入

**约束**:
```vue
<!-- 必须在 MemoryManager.vue 中添加： -->
<template>
  <n-layout has-sider>
    <n-layout-sider>
      <DomainTree :project-root-path="projectPath" @select="handleDomainSelect" />
    </n-layout-sider>
    <n-layout-content>
      <MemoryWorkspace>
        <!-- 记忆卡片中显示 VitalityBadge -->
        <VitalityBadge :vitality="memory.vitality" />
      </MemoryWorkspace>
      <BatchActionBar v-if="batchMode" />
    </n-layout-content>
  </n-layout>
</template>
```

**验收标准**:
- [ ] 4 个组件在主容器中可见
- [ ] 组件间通过 provide/inject 共享状态
- [ ] 添加 E2E 测试验证交互流程

---

### HC-04: Tauri IPC 命令未注册 ⚠️ HIGH

**问题**: 前端调用的 Tauri 命令在 `src/rust/ui/commands.rs` 中未定义

**缺失命令**:
1. `get_domain_list` - 获取域列表
2. `delete_empty_domain` - 删除空域
3. `get_cleanup_candidates` - 获取清理候选
4. `cleanup_memories` - 执行清理
5. `get_memory_snapshots` - 获取快照历史
6. `rollback_to_snapshot` - 回滚快照

**约束**:
```rust
// 必须在 src/rust/ui/commands.rs 中添加：
#[tauri::command]
pub async fn get_domain_list(project_path: String) -> Result<Vec<DomainInfo>, String> {
    // 调用 MemoryManager::get_domain_list()
}
```

**验收标准**:
- [ ] 6 个命令在 `commands.rs` 中实现
- [ ] 命令在 `builder.rs` 的 `.invoke_handler()` 中注册
- [ ] 前端调用不报 "command not found" 错误

---

### HC-05: 活力衰减未自动触发 ⚠️ MEDIUM

**问题**: `vitality.rs` 实现了衰减计算，但未在 `manager.rs` 中自动触发

**约束**:
```rust
// 必须在 manager.rs 的 recall_memories() 中添加：
for entry in &mut results {
    VitalityEngine::boost_vitality(entry, &self.store.config);
}
self.save()?; // 保存更新后的活力值
```

**验收标准**:
- [ ] 每次回忆操作自动提升活力值
- [ ] 定期后台任务计算衰减（可选：使用 tokio::spawn）
- [ ] 添加集成测试验证衰减逻辑

---

### HC-06: 摘要生成未在写入时触发 ⚠️ MEDIUM

**问题**: `summary.rs` 已实现但 `manager.rs` 的 `add_memory()` 未调用

**约束**:
```rust
// 必须在 manager.rs 的 add_memory() 中添加：
if SummaryGenerator::needs_summary(&content, &self.store.config) {
    entry.summary = Some(SummaryGenerator::generate_rule_based(&content));
}
```

**验收标准**:
- [ ] 长记忆（>500 字符）自动生成摘要
- [ ] 摘要显示在前端列表视图
- [ ] 添加单元测试验证摘要生成

---

### HC-07: 前端测试覆盖不足 ⚠️ MEDIUM

**问题**: 新增前端组件缺少单元测试

**缺失测试**:
1. `DomainTree.spec.ts` - 域树组件测试
2. `VitalityBadge.spec.ts` - 活力徽章测试
3. `BatchActionBar.spec.ts` - 批量操作测试
4. `SnapshotDiff.spec.ts` - 快照对比测试
5. `MemoryWorkspace.spec.ts` - 工作区测试

**约束**:
```typescript
// 必须在 src/frontend/components/tools/__tests__/ 中添加：
describe('DomainTree', () => {
  it('should load domains on mount', async () => {
    // 测试域列表加载
  })
  it('should emit select event on node click', () => {
    // 测试选择事件
  })
})
```

**验收标准**:
- [ ] 5 个组件有对应的 `.spec.ts` 文件
- [ ] 测试覆盖率 > 80%（vitest --coverage）
- [ ] CI 流程中运行前端测试

---

## 3. 软约束（优化建议）

### SC-01: FTS5 中文分词优化

**当前**: 使用 `unicode61` 分词器，中文按字符分词

**建议**: 评估 `jieba-rs` 集成，提升中文搜索召回率

**优先级**: P2（非阻塞）

---

### SC-02: 活力衰减可视化增强

**当前**: VitalityBadge 显示当前活力值

**建议**: 添加 30 天趋势折线图（已在 VitalityBadge.vue 中实现但未启用）

**优先级**: P3（体验优化）

---

### SC-03: 批量操作性能优化

**当前**: 批量删除逐条调用 `delete_memory()`

**建议**: 实现 `batch_delete_memories(ids: Vec<String>)` 减少 I/O

**优先级**: P2（性能优化）

---

## 4. 依赖关系图

```
HC-02 (MCP 接口) ──┬──> HC-04 (Tauri 命令) ──> HC-03 (前端集成)
                   │
HC-01 (FTS5 集成) ─┘

HC-05 (活力衰减触发) ──> HC-03 (前端显示)

HC-06 (摘要生成触发) ──> HC-03 (前端显示)

HC-07 (前端测试) ──> 所有前端功能
```

**关键路径**: HC-02 → HC-04 → HC-03（必须按顺序完成）

---

## 5. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| FTS5 集成失败导致搜索不可用 | HIGH | 保留模糊匹配降级路径（HC-18） |
| 前端组件集成冲突 | MEDIUM | 使用 provide/inject 隔离状态 |
| Tauri 命令注册遗漏 | MEDIUM | 添加 E2E 测试覆盖所有命令 |
| 活力衰减计算错误 | LOW | 已有单元测试覆盖 |

---

## 6. 成功判据（可验证）

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
- [ ] **T3**: 集成测试覆盖 5 个关键路径（FTS5 搜索、活力衰减、批量操作、快照回滚、域筛选）
- [ ] **T4**: E2E 测试覆盖完整用户流程（添加记忆 → 搜索 → 查看详情 → 回滚快照 → 删除）

### 文档完整性

- [ ] **D1**: 用户文档说明新功能使用方法（域树、活力徽章、FTS5 搜索）
- [ ] **D2**: API 文档列出新增 MCP 操作和 Tauri 命令
- [ ] **D3**: 开发者文档说明架构变更（Registry、FTS5 Sidecar、活力衰减）

---

## 7. 实施优先级（按依赖关系排序）

### 第 1 阶段（阻塞性修复）- 2 工作日

1. **HC-02**: 实现 5 个新 MCP 操作（6h）
2. **HC-04**: 注册 6 个 Tauri 命令（4h）
3. **HC-01**: 集成 FTS5 搜索到 manager.rs（4h）

### 第 2 阶段（前端激活）- 2 工作日

4. **HC-03**: 集成 4 个前端组件到主容器（8h）
5. **HC-05**: 触发活力衰减自动更新（3h）
6. **HC-06**: 触发摘要自动生成（2h）

### 第 3 阶段（测试补全）- 1.5 工作日

7. **HC-07**: 添加 5 个前端组件测试（8h）
8. 添加集成测试（4h）

### 第 4 阶段（文档和优化）- 0.5 工作日

9. 更新用户文档和 API 文档（2h）
10. 性能测试和优化（2h）

**总计**: 约 6 工作日（48 小时）

---

## 8. 开放问题（需用户确认）

### Q1: FTS5 中文分词方案

**问题**: 当前使用 `unicode61` 分词器，中文按字符分词，召回率可能不足

**选项**:
- A. 保持 `unicode61`（简单，无额外依赖）
- B. 集成 `jieba-rs`（更好的中文分词，增加依赖）

**建议**: 先使用 A，根据用户反馈决定是否升级到 B

---

### Q2: 活力衰减后台任务

**问题**: 当前活力衰减仅在访问时计算，未定期后台更新

**选项**:
- A. 保持当前懒计算（简单，无后台任务）
- B. 添加定期后台任务（每小时计算一次，更准确）

**建议**: 先使用 A，如果用户需要实时清理候选列表再升级到 B

---

### Q3: 批量操作事务性

**问题**: 批量删除失败时是否回滚已删除的记忆

**选项**:
- A. 部分成功（已删除的不回滚，报告失败数量）
- B. 全部回滚（任何失败都恢复所有记忆）

**建议**: 使用 A（更符合用户预期，避免意外恢复）

---

## 9. 附录：文件清单

### 已实现文件（需集成）

**后端**:
- `src/rust/mcp/tools/memory/write_guard.rs` ✅
- `src/rust/mcp/tools/memory/registry.rs` ✅
- `src/rust/mcp/tools/memory/uri_path.rs` ✅
- `src/rust/mcp/tools/memory/vitality.rs` ✅
- `src/rust/mcp/tools/memory/fts_index.rs` ✅
- `src/rust/mcp/tools/memory/summary.rs` ✅
- `src/rust/mcp/tools/memory/summary_service.rs` ✅
- `src/rust/mcp/tools/memory/observation_store.rs` ✅

**前端**:
- `src/frontend/components/tools/DomainTree.vue` ✅
- `src/frontend/components/tools/VitalityBadge.vue` ✅
- `src/frontend/components/tools/TagFilter.vue` ✅
- `src/frontend/components/tools/BatchActionBar.vue` ✅
- `src/frontend/components/tools/SnapshotDiff.vue` ✅
- `src/frontend/components/tools/MemoryWorkspace.vue` ✅
- `src/frontend/components/tools/MemoryManager.vue` ✅
- `src/frontend/composables/useVitalityDecay.ts` ✅
- `src/frontend/composables/useProgressiveDisclosure.ts` ✅

### 需修改文件（集成点）

**后端**:
- `src/rust/mcp/tools/memory/manager.rs` - 集成 FTS5、活力衰减、摘要生成
- `src/rust/mcp/tools/memory/mcp.rs` - 添加 5 个新 MCP 操作
- `src/rust/ui/commands.rs` - 注册 6 个 Tauri 命令
- `src/rust/app/builder.rs` - 注册命令到 invoke_handler

**前端**:
- `src/frontend/components/tools/MemoryConfig.vue` 或 `MemoryManager.vue` - 集成新组件
- `src/frontend/App.vue` - 确保路由正确

### 需创建文件（测试）

**前端测试**:
- `src/frontend/components/tools/__tests__/DomainTree.spec.ts`
- `src/frontend/components/tools/__tests__/VitalityBadge.spec.ts`
- `src/frontend/components/tools/__tests__/BatchActionBar.spec.ts`
- `src/frontend/components/tools/__tests__/SnapshotDiff.spec.ts`
- `src/frontend/components/tools/__tests__/MemoryWorkspace.spec.ts`

**集成测试**:
- `src/rust/mcp/tools/memory/integration_tests.rs`

---

## 10. 总结

**当前状态**: 85% 功能已实现，主要缺口在集成和测试

**关键阻塞**: HC-02（MCP 接口）和 HC-04（Tauri 命令）必须先完成

**预计工时**: 6 工作日（48 小时）完成所有硬约束

**风险等级**: MEDIUM（主要是集成风险，核心逻辑已验证）

**下一步**: 生成详细实施计划（plan 文件）
