## OpenSpec 合规审查报告（修复后二次审查）

> 日期：2026-02-20
> 任务：记忆管理系统全面优化（memory-optimization）
> 审查模式：双模型交叉审查（Codex + Gemini）
> 审查维度：约束合规 + 修复验证 + 代码质量 + 测试覆盖

---

### 审查范围

- 计划文件：`.doc/spec/plans/20260219-memory-optimization-plan.md`
- 约束集文件：`.doc/spec/constraints/20260219-memory-optimization-constraints.md`
- 实施报告：`.doc/spec/reviews/20260219-memory-optimization-impl-report.md`
- 约束数量：硬约束 10 (HC-10~19) + 软约束 14 (SC-4~26) + 依赖约束 8 (DEP-01~08) + 风险约束 10 (RISK-01~10)
- 变更文件：后端新增 7 个 + 修改 8 个，前端新增 9 组件 + 2 composables + 1 类型文件
- 修复文件：7 个（McpToolsTab.vue, AppContent.vue, MemoryManager.vue, MemoryWorkspace.vue, TagFilter.vue, memoryKeys.ts, commands.rs, builder.rs）
- 编译验证：`cargo check` PASS + `pnpm build` PASS

---

### 审查结果

| 级别 | 数量 | Codex | Gemini | 共识 |
|------|------|-------|--------|------|
| **Critical** | **0** | 0 | 0 | 0 |
| **Warning** | **10** | 5 | 3 | 8+2 |
| **Info** | **6** | 1 | 2 | 5+1 |

---

### Critical 问题修复验证

#### C1 修复验证：8 个新前端组件已集成到 UI

**修复内容**：

1. `src/frontend/components/tabs/McpToolsTab.vue` -- 将 import 和渲染从 `MemoryConfig` 改为 `MemoryManager`
2. `src/frontend/components/tools/memoryKeys.ts` -- 新建独立文件存放 5 个 InjectionKey 定义
3. `src/frontend/components/tools/MemoryManager.vue` -- 移除非法 `export const`（`<script setup>` 禁止 ES module 导出），改为从 `memoryKeys.ts` 导入
4. `src/frontend/components/tools/MemoryWorkspace.vue` -- import 路径从 `./MemoryManager.vue` 改为 `./memoryKeys`
5. `src/frontend/components/tools/TagFilter.vue` -- import 路径从 `./MemoryManager.vue` 改为 `./memoryKeys`

**Codex 验证**：PASS -- 挂载链 McpToolsTab -> MemoryManager -> (provide) -> DomainTree/MemoryWorkspace/MemorySearch/MemoryConfig 完整。Codex 额外发现并验证了 `<script setup>` 中 `export const` 导致的构建错误已修复（`pnpm build` 通过）。

**Gemini 验证**：PASS -- 组件挂载链完整，provide/inject 状态（5 个 InjectionKey）可被子组件正确注入。

**共识**：**C1 已完全修复**。

---

#### C2 修复验证：主界面模式下 projectRootPath 降级机制

**修复内容**：

1. `src/rust/ui/commands.rs` -- 新增 `get_current_dir` Tauri 命令（只读 `current_dir()` + 路径规范化）
2. `src/rust/app/builder.rs` -- 注册 `get_current_dir` 命令
3. `src/frontend/components/AppContent.vue` -- 新增 `fallbackProjectPath` ref，`onMounted` 非弹窗模式时调用 `invoke('get_current_dir')`，模板绑定 `fallbackProjectPath` 替代硬编码 `null`

**Codex 验证**：PASS（有改进空间）-- `get_current_dir` 仅只读查询 + 路径规范化，无安全风险。`fallbackProjectPath` 逻辑健壮：失败时置 `null`，Vue 响应式系统处理初始 `null` 状态。

**Gemini 验证**：PASS（CONCERN：路径语义风险）-- `current_dir()` 对 Tauri 应用可能返回安装目录而非项目目录。建议增加 UI 提示标注"当前工作目录"。竞态条件已被 Vue 响应式 + 组件内 `v-if="!projectPath"` 空状态安全处理。

**共识**：**C2 已修复**（降级行为正确，Warning 级改进建议见 W9）。

---

#### 额外修复：`<script setup>` 构建错误

**问题**：`MemoryManager.vue` 在 `<script setup>` 中使用 `export const` 定义 InjectionKey，导致 Vite 构建失败（`@vue/compiler-sfc` 禁止 `<script setup>` 中的 ES module 导出）。

**发现来源**：Codex 在审查 C1 修复时执行 `pnpm build` 发现此构建错误。

**修复**：将 5 个 InjectionKey 提取到独立的 `memoryKeys.ts` 文件，所有消费组件的 import 路径同步更新。

**验证**：`pnpm build` 成功（42.22s），`MemoryManager-BJ3dOlFd.js` chunk 正确生成（53.08 kB）。

---

### Warning 问题（建议修复）

| # | 描述 | 违反约束 | 来源 | 修复建议 |
|---|------|----------|------|----------|
| W1 | SC-17 前缀搜索语法未实现 | SC-17 | 首次审查 | 添加正则解析 `@domain` 和 `#tag` 前缀 |
| W2 | SC-24 意图识别检索未实现 | SC-24 | 首次审查 | 添加关键词评分法识别查询意图 |
| W3 | RISK-01 定时校验未实现 | RISK-01 | 首次审查 | 添加定时任务调用 `verify_consistency()` |
| W4 | RISK-04 "被拒绝记忆"无前端审查视图 | RISK-04 | 首次审查 | 在 MemoryWorkspace 添加被拒绝记忆面板 |
| W5 | VitalityBadge max 与后端不一致 | 显示准确性 | 首次审查 | 统一为 3.0 或从后端配置读取 |
| W6 | summary.rs 使用字节长度判断中文 | SC-19 | 首次审查 | 改用 `content.chars().count()` |
| W7 | migration.rs 创建 v2.0 而非 v2.2 | HC-19 | 首次审查 | 直接写入 v2.2 更清晰 |
| W8 | memory CLAUDE.md 文档描述 v2.1 | 文档准确性 | 首次审查 | 更新文档反映 v2.2 实际结构 |
| W9 | fallback 路径语义风险 | UX 一致性 | Codex+Gemini | `get_current_dir` 返回进程工作目录，可能非项目目录。建议：UI 标注"当前工作目录"或递归查找 `.git`/`package.json` |
| W10 | C1/C2 缺少直接自动化测试 | 测试覆盖 | Codex | 建议补充 McpToolsTab 挂载测试和 AppContent fallback 路径测试 |

---

### Info 问题（可选改进）

| # | 描述 | 来源 | 建议 |
|---|------|------|------|
| I1 | SnapshotDiff 使用简单逐行 diff 算法 | 首次审查 | 未来可引入 diff-match-patch |
| I2 | BatchActionBar 逐条调用 Tauri 命令 | 首次审查 | 大批量操作可考虑批量 API |
| I3 | ObservationStore 跳过列表硬编码 | 首次审查 | 可通过 MemoryConfig 配置化 |
| I4 | FTS5 unicode61 分词器对中文效果未验证 | 首次审查 | A/B 测试当前为合理起步方案 |
| I5 | useProgressiveDisclosure 三态循环 | 首次审查 | 可添加直接跳转 detail 的快捷键 |
| I6 | MemoryManager 850px 模态框宽度 | Gemini | 在较窄窗口下可能操作不便，考虑响应式适配 |

---

### 约束合规矩阵

#### 硬约束 (HC-10 ~ HC-19) -- 全部合规

| 约束编号 | 类型 | 合规状态 | 验证文件 | 审查备注 |
|----------|------|----------|----------|----------|
| HC-10 | 硬 | **合规** | `manager.rs` L173/L182 | `max_entry_bytes=10240`、`max_entries=1000` 限制在 `add_memory()` 中检查 |
| HC-11 | 硬 | **合规** | `write_guard.rs` 全文 | 三级判定：>=0.80 NOOP、0.60-0.80 UPDATE、<0.60 ADD |
| HC-12 | 硬 | **合规** | `types.rs:30-57` | 6 个新字段全部 `Option<T>` + `#[serde(default)]` |
| HC-13 | 硬 | **合规** | `registry.rs` 全文 | `Weak<RwLock<MemoryManager>>`，TTL=30min，POOL_SIZE=16 |
| HC-14 | 硬 | **合规** | `uri_path.rs:12-15` | 正则 `^([a-z][a-z0-9_-]*)://(.+)$`，支持中文路径段 |
| HC-15 | 硬 | **合规** | `vitality.rs:67-95` | 指数衰减 V(t)=V0*2^(-t/half_life)，Rule 类别永不清理 |
| HC-16 | 硬 | **合规** | `fts_index.rs` 全文 | JSON 为 source of truth，FTS5 仅加速搜索 |
| HC-17 | 硬 | **合规** | `MemoryManager.vue:185-204` | CSS `min-width: 850px`，sider `width=200`/`collapsed-width=0`，main `min-width: 500px`。**C1 修复后可见** |
| HC-18 | 硬 | **合规** | `manager.rs` save_store() | 原子写入 tmp+rename，fts_index sync 不传播错误 |
| HC-19 | 硬 | **合规** | `types.rs:181-209` | upgrade_to_current() 链式升级 v1.0->v2.1->v2.2 |

#### 软约束 (SC-4 ~ SC-26)

| 约束编号 | 合规状态 | 审查备注 |
|----------|----------|----------|
| SC-4 | **合规** | Patch 模式（完全替换）支持 |
| SC-5 | **合规** | 版本兼容 + 链式升级 |
| SC-6 | **合规** | 自动创建快照，最多保留 5 个 |
| SC-7 | **合规** | 文本相似度，Embedding 为未来方向 |
| SC-15 | **合规** | 阈值可通过 MCP 配置操作修改 |
| SC-16 | **合规** | 活力衰减 5 个参数全部可配置 |
| SC-17 | **不合规** | 前端搜索仅用 includes()，未实现前缀语法 [W1] |
| SC-18 | **合规** | useProgressiveDisclosure 三态管理 |
| SC-19 | **合规** | summary.rs 规则引擎实现（字节长度问题 [W6]） |
| SC-20 | **合规** | verbose 参数 + 分页支持 |
| SC-21 | **合规** | BatchActionBar 批量操作。**C1 修复后可见** |
| SC-22 | **合规** | 懒加载 + canonical_path() |
| SC-23 | **合规** | unicode61 tokenizer |
| SC-24 | **不合规** | 意图识别完全未实现 [W2] |
| SC-25 | **合规** | SQLite + tokio mpsc + FIFO 淘汰 |
| SC-26 | **合规** | SnapshotDiff 逐行 diff。**C1 修复后可见** |

#### 依赖约束 (DEP-01 ~ DEP-08) -- 全部合规

#### 风险约束 (RISK-01 ~ RISK-10)

| 约束编号 | 缓解状态 | 审查备注 |
|----------|----------|----------|
| RISK-01 | 部分缓解 | verify_consistency() 存在但无定时调度 [W3] |
| RISK-02 | 已缓解 | Option<T> + serde(default) + 升级失败不覆盖 |
| RISK-03 | 已缓解 | Weak 引用 + TTL + 池大小限制 |
| RISK-04 | 部分缓解 | NOOP 记录日志但无前端审查视图 [W4] |
| RISK-05 | 已缓解 | 低于 update 阈值正常 ADD + 整理兜底 |
| RISK-06 | 已缓解 | Rule 类别永不清理 |
| RISK-07 | 已标记 | unicode61 合理起步，A/B 未执行 |
| RISK-08 | 已缓解 | 旧记忆默认 legacy://uncategorized |
| RISK-09 | 已缓解 | ARIA 属性完备 |
| RISK-10 | 已缓解 | 跳过列表 + FIFO 5000 条 |

---

### 裁决

- **结论**：**通过**
- **可归档**：**是**
- **前提条件**：C1 + C2 + `<script setup>` 构建错误 均已修复并通过编译验证

### 建议后续迭代优化

1. W1/W2（搜索增强）：实现前缀搜索语法和意图识别
2. W9（路径语义）：改进 `get_current_dir` 为 `detect_project_root`（递归查找 .git/package.json）
3. W10（测试覆盖）：补充 McpToolsTab/AppContent 的集成测试

---

### 双模型审查元数据

| 字段 | 值 |
|------|-----|
| review_mode | 双模型交叉审查（二次审查，针对 C1+C2 修复验证） |
| dual_model_status | SUCCESS（两个模型均返回有效结论） |
| codex_session | `019c76e1-600b-71a1-9d5b-c8cff3e77463` |
| gemini_session | `3aa20c75-365d-42d5-9e2b-67cc40dd17f1` |
| codex_findings | C1 PASS, C2 PASS(有改进空间), 回归 FAIL(缺测试), HC PASS |
| gemini_findings | C1 PASS, C2 CONCERN(路径语义), 竞态 PASS, A11y PASS |
| compilation_check | `cargo check` PASS (4 warnings, 0 errors) + `pnpm build` PASS (42.22s) |
| files_modified | 8 (McpToolsTab.vue, AppContent.vue, MemoryManager.vue, MemoryWorkspace.vue, TagFilter.vue, memoryKeys.ts, commands.rs, builder.rs) |

---

### 归档信息

- 归档路径：`.doc/spec/archive/20260220-memory-optimization-archived.md`
- 包含文件：约束集、提案、计划、实施报告、审查报告
