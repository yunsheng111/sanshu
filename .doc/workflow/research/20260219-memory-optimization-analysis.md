# 技术分析：记忆管理系统全面优化方案

> 日期：2026-02-19
> 状态：SUCCESS（双模型均成功）+ Memory-Palace 参考研究
> Codex SESSION_ID: 019c75e4-93db-75d0-81da-2d80630282a8
> Gemini SESSION_ID: e60f4a6b-1c88-4691-aa55-1b340335ec11

---

## 需求摘要

记忆管理系统（ji 工具）全面优化，涵盖 5 个核心维度：

1. **MCP 工具项目路径指定优化**：消除每次调用重复创建 MemoryManager 的开销
2. **分类体系升级**：从 4 种固定分类扩展为 URI 路径 + 面向切面标签的混合方案
3. **图形化可视化**：前端 UI 从扁平列表升级为树形/图谱/时间线等多视图
4. **前后端交互增强**：搜索筛选、批量操作、高级搜索语法
5. **记忆生命周期管理**：Write Guard 写入守卫 + Vitality Decay 活力衰减

---

## 现有架构分析

### 技术栈
- **后端**：Rust + Tauri 2.0 + rmcp 0.12.0
- **前端**：Vue 3 + Naive UI + UnoCSS + Tauri invoke
- **存储**：JSON 文件 (`.sanshu-memory/memories.json`)
- **并发**：`SharedMemoryManager` (`Arc<RwLock<MemoryManager>>`)

### 相关模块
| 模块 | 文件 | 职责 |
|------|------|------|
| 数据模型 | `types.rs` | MemoryStore v2.1, MemoryEntry, MemoryCategory |
| 管理器 | `manager.rs` | CRUD + 去重 + 版本快照 + 路径规范化 |
| MCP 入口 | `mcp.rs` | 8 种操作（记忆/回忆/整理/列表/预览相似/配置/删除/更新） |
| 相似度 | `similarity.rs` | Levenshtein(40%) + Phrase(40%) + Jaccard(20%) |
| 去重 | `dedup.rs` | 阈值去重 + 统计报告 |
| 迁移 | `migration.rs` | MD -> JSON 格式迁移 |
| 前端主容器 | `MemoryConfig.vue` | 4 Tab (配置/列表/搜索/相似度预览) |
| 前端列表 | `MemoryList.vue` | 分页 + 分类筛选 + 删除 + 编辑 |
| 前端搜索 | `MemorySearch.vue` | 关键词搜索 + 相关度 + 高亮 |

### 已知约束
- 存储上限：1000 条记忆（`max_entries` 配置）
- 单条上限：10KB（`max_entry_bytes` 配置）
- 搜索方式：仅模糊匹配（线性扫描）
- 分类固定：4 种枚举值（Rule/Preference/Pattern/Context）
- 实例生命周期：每次 MCP 调用创建新 MemoryManager 实例
- 窗口约束：Tauri 弹窗宽度约 850px

---

## 并行分析结果

### Codex 后端视角

**核心矛盾识别**：当前架构在 <=1000 条时可用，但搜索、分类、并发复用都建立在线性扫描和单文件之上，扩展到 1w+ 会出现明显性能与维护压力。

**三大风险**：
1. **实例重复创建**：每次调用创建 MemoryManager，有冷启动和重复加载成本
2. **语义维度不足**：4 类固定分类难表达"项目/功能/事件"交叉关系
3. **检索能力天花板**：模糊匹配和相似度重排依赖全量遍历，复杂度线性上升

**方案 A（渐进式演进 -- 推荐优先落地）**：
- 新增 `MemoryManagerRegistry`（全局池），按 canonical project_path 缓存，配 TTL/LRU + Weak 引用回收
- 引入 `StorageEngine` 抽象层，保留 JSON 为主存储
- 增加 Sidecar 索引（SQLite FTS5）用于搜索，不立即替换主数据源
- 数据模型新增 `dimensions: { project?:[], function?:[], event?:[] }` 可选字段
- schema_version 升至 2.2，启动时懒迁移
- 性能预估：1k=20-80ms, 1w=40-150ms

**方案 B（全面重构 -- 按容量阈值触发）**：
- SQLite 作为唯一真源（WAL 模式），JSON 改为导出/备份
- 关系模型建表：`memory_entries` / `memory_snapshots` / `memory_dimensions` / `memory_fts`
- 统一检索管线：FTS 召回 + 规则过滤 + 相似度重排
- 性能预估：1k=10-40ms, 1w=15-80ms, 10w=30-200ms

**检索能力评估**：
- FTS5：短中期性价比最高，应优先
- 向量检索：1k-1w 规模非刚需，适合语义差异大的场景增量引入
- 混合检索：推荐终态（FTS 召回 + 向量补召回 + 规则重排），分阶段实施

### Gemini 前端视角

**UX 核心问题**：
- 扁平化列表在数据量增加时导致"信息过载"和"检索迷失"
- 三维度逻辑关系在扁平列表中难以直观体现
- 缺乏批量操作导致维护成本随数据量指数级增长
- 850px 窗口宽度属于"紧凑型"桌面布局

**方案 A（结构化效能型 -- 主框架）**：
- 左侧收纳栏（Collapsible Sider）：展示「项目」树形目录
- 顶部动态标签（Dynamic Tags）：显示当前「功能」分类
- 中心区域（Main Content）：多栏布局按「事件时间线」排列记忆卡片
- 搜索支持 `@项目`、`#功能` 前缀快速过滤
- Naive UI 映射：NLayoutSider + NTree / NTabs(segment) / NCard + NCheckbox / NDataTable

**方案 B（关系探索型 -- 增强组件）**：
- 主视图为力导向图（Force-directed Graph），节点颜色代表项目，形状代表功能
- 右侧抽屉（Detail Drawer）：点击节点弹出详情
- 底部时间滑块（Time Slider）：查看记忆随时间增长分布
- Naive UI 映射：v-network-graph / NDrawer / NSlider / NStats

**可视化方案对比**：

| 方案 | 优点 | 缺点 | 适用场景 |
|------|------|------|----------|
| 树形面板 | 逻辑严谨，符合文件系统直觉 | 视觉较枯燥 | 生产力工具，大量数据维护 |
| 标签云 | 直观反映高频关键词 | 无法表现层级嵌套 | 灵感记录，非线性思维 |
| 时间线 | 完美还原事件维度 | 横向空间占用大，850px 下易拥挤 | 个人日志，进展追踪 |

**实施工时估算**：

| 模块 | 工时 |
|------|------|
| 三维面板重构 | 12h |
| 图形化组件 | 16h |
| 批量操作引擎 | 8h |
| 高级搜索与筛选 | 10h |
| 响应式与 A11y | 6h |
| **前端总计** | **52h (~6.5 工作日)** |

---

## Memory-Palace 参考研究

> 项目地址：https://github.com/AGI-is-going-to-arrive/Memory-Palace
> 技术栈：Python + FastAPI + SQLite + React + Vite + TailwindCSS
> 定位：AI Agent 长期记忆操作系统，支持 MCP 协议

### Memory-Palace 核心架构

| 层 | 技术 | 关键能力 |
|---|---|---|
| 后端 | FastAPI + SQLAlchemy + SQLite | 记忆 CRUD、检索、审查、维护 |
| MCP | mcp.server.fastmcp | 9 个标准化工具，支持 stdio/SSE |
| 前端 | React + Vite + TailwindCSS + Framer Motion | 4 大功能视图仪表盘 |
| 运行时 | 内置队列与 worker | 写入串行化、索引重建、活力衰减 |

### 值得借鉴的 6 大机制

#### 1. Write Guard（写入守卫） -- P0 优先

Memory-Palace 的核心创新之一。每次写入前经过三级判定链：
- **语义匹配** -- 检测是否存在语义重复的记忆
- **关键词匹配** -- 快速过滤明显重复
- **LLM 决策（可选）** -- 疑难情况由 AI 辅助判断

判定结果：`ADD`（新增） | `UPDATE`（合并更新） | `NOOP`（忽略重复） | `DELETE`（标记删除）

**对三术的启发**：当前 ji 工具的去重只在写入后批量检测（`整理`操作），而非写入前拦截。引入 Write Guard 可以从源头防止垃圾记忆进入系统。

**适配方案**：在 `MemoryManager::add_memory()` 写入前新增 `write_guard()` 检查步骤，复用现有 `TextSimilarity::calculate()` 能力。阈值高于 0.8 自动 NOOP，0.6-0.8 自动 UPDATE（合并），低于 0.6 正常 ADD。

#### 2. Vitality Decay（活力衰减） -- P1 优先

记忆是有生命力的实体，核心参数：
- **vitality_score**：最大 3.0，每次访问可提升
- **指数衰减**：半衰期 30 天
- **清理阈值**：活力低于 0.35 且超过 14 天未访问自动标记清理候选
- **清理审批**：需要确认才能执行

**对三术的启发**：当前只有硬上限（`max_entries=1000`），没有"记忆过期"概念。引入活力衰减可以自动淘汰不再有价值的旧记忆，保持记忆库的信噪比。

#### 3. Intent Recognition（意图识别检索） -- P2

搜索先识别用户意图再匹配策略：
- `factual` -- 事实型（高精度匹配）
- `exploratory` -- 探索型（高召回）
- `temporal` -- 时间型（时间过滤）
- `causal` -- 因果型（宽候选池）

#### 4. URI 路径体系（domain://path） -- P1

Memory-Palace 用 `domain://path` 格式组织记忆，天然支持树形浏览，且与文件系统类比直观。支持别名（alias）跨域引用。

**与三维标签的对比**：

| 维度 | URI 路径 | 三维标签 |
|---|---|---|
| 组织方式 | 层级树形 `domain://a/b/c` | 扁平标签 `[project:x, func:y]` |
| 浏览性 | 天然支持树形浏览 | 需要多维交叉筛选 |
| 灵活性 | 路径嵌套无限层级 | 三个固定维度 |
| 交叉引用 | 别名（alias）机制 | 同一条目多标签 |
| 适合场景 | 大量记忆、深层结构 | 中等记忆、快速筛选 |

**建议**：采用 URI 路径 + 自由标签的混合方案（见下方修订数据模型）。

#### 5. Snapshot + Diff + Rollback -- P2

写入前自动创建 snapshot，前端提供 Diff 可视化，支持一键回滚。当前 ji 工具已有 snapshots 和 rollback 能力，但前端未展示。

#### 6. 分级部署档位 -- 参考

A/B/C/D 四档覆盖从纯关键词到完整云端检索，参考其分级思路明确每个阶段的能力边界。

### 不建议直接照搬的部分

| 功能 | 原因 |
|---|---|
| C/S 架构（FastAPI HTTP 服务） | 三术是嵌入式 MCP 服务器（stdio），无需独立 HTTP 后端 |
| React 前端 | 三术已有 Vue 3 + Naive UI 成熟前端 |
| SQLAlchemy ORM | Rust 生态用 rusqlite 更合适 |
| API Key 鉴权 | 三术走 Tauri IPC，无需 HTTP 鉴权 |
| Docker 部署 | 三术是桌面应用，不需要容器化 |

---

## 交叉验证

### 一致观点（强信号）

1. **当前架构的天花板共识**：双方均认为现有扁平存储 + 线性搜索在 1000+ 条时会遇到性能和体验瓶颈
2. **渐进式优先策略**：双方都推荐先渐进式优化再按需重构，避免一步到位的高风险改造
3. **分类体系升级必要性**：双方均认为 4 种固定分类不足，Memory-Palace 的 URI 路径进一步验证了层级组织的价值
4. **FTS5 是短中期最佳选择**：后端确认 FTS5 性价比最高，Memory-Palace 的混合检索管线进一步佐证
5. **写入质量控制缺失**：Memory-Palace 的 Write Guard 机制揭示了"源头控制"的重要性，与 Codex 识别的"去重后置"问题一致

### 分歧点（需权衡）

| 议题 | Codex 观点 | Gemini 观点 | Memory-Palace 参考 | 最终建议 |
|------|------------|-------------|---------------------|----------|
| 分类方式 | dimensions 扁平标签 | 左侧树形面板 | URI 路径体系 | URI 路径 + 自由标签混合 |
| 知识图谱组件 | 未提及 | 推荐力导向图 | 无（专注树形浏览） | P3 功能，非优先 |
| 记忆生命周期 | 未提及 | 未提及 | Vitality Decay | P1 引入活力衰减 |
| 写入质量控制 | 去重后置 | 未提及 | Write Guard 前置 | P0 引入写入守卫 |

### 互补见解

- **Codex 独特洞察**：MemoryManagerRegistry 全局池设计、双写一致性校验方案、中文分词 tokenizer 选型
- **Gemini 独特洞察**：850px 紧凑布局策略、NDataTable 虚拟滚动性能保障、搜索前缀语法、A11y 无障碍方案
- **Memory-Palace 独特洞察**：Write Guard 三级判定链、Vitality Decay 指数衰减模型、意图识别检索策略、URI 路径组织体系

---

## 可行性评估

| 维度 | 评估 | 说明 |
|------|------|------|
| 技术可行 | 可行 | 现有 Rust 生态支持 SQLite(rusqlite) + FTS5；Vue 3 + Naive UI 支持所有 UI 方案；Write Guard 可复用现有相似度算法 |
| 实施成本 | 中等 | 修订后总计约 88h（11 工作日），详见下方优先级表 |
| 风险等级 | 中低 | 渐进式方案风险低，核心风险在双写一致性、FTS 中文分词效果、活力衰减参数调优 |

---

## 修订后的推荐方案

### 数据模型 v2.2（修订版）

```rust
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub content_normalized: String,
    pub category: MemoryCategory,               // 保留原有分类（向后兼容）
    pub uri_path: Option<String>,               // 新增：如 "core://architecture/backend"
    pub domain: Option<String>,                 // 新增：如 "core", "project", "session"
    pub tags: Option<Vec<String>>,              // 新增：自由标签（替代固定三维）
    pub vitality_score: Option<f64>,            // 新增：活力值 (0.0-3.0)
    pub last_accessed_at: Option<DateTime<Utc>>,// 新增：最后访问时间
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub snapshots: Vec<MemorySnapshot>,
}
```

### 前端组件结构（修订版）

```
MemoryManager.vue（主容器 -- 替代 MemoryConfig.vue）
  +-- DomainTree.vue（左侧域/路径树）
  |   +-- NTree + NLayoutSider
  +-- MemoryWorkspace.vue（中间工作区）
  |   +-- TagFilter.vue（标签筛选条 -- NTag + NSpace）
  |   +-- MemoryCardList.vue（记忆卡片列表 -- NDataTable virtual）
  |   +-- VitalityBadge.vue（活力值徽章 -- NProgress + NTooltip）
  |   +-- BatchActionBar.vue（批量操作条 -- NAffix 底部固定）
  +-- MemorySearch.vue（搜索面板 -- 增强版）
  +-- MemoryConfig.vue（配置面板 -- 精简版）
```

### 优先级实施表

| 优先级 | 功能 | 来源 | 工时预估 |
|--------|------|------|----------|
| P0 | Write Guard（写入守卫） | Memory-Palace | 8h |
| P0 | MemoryManagerRegistry（全局池） | Codex 分析 | 12h |
| P1 | Vitality Decay（活力衰减） | Memory-Palace | 10h |
| P1 | URI 路径体系 + 自由标签 | Memory-Palace + 面向切面分类 | 16h |
| P1 | 前端树形浏览 + 标签筛选 | Gemini 分析 | 12h |
| P2 | FTS5 检索 + 意图识别 | Codex + Memory-Palace | 16h |
| P2 | Snapshot Diff 前端视图 | Memory-Palace | 8h |
| P3 | 活力衰减前端可视化 | Memory-Palace | 6h |
| | **总计** | | **约 88h (11 工作日)** |

---

## 分阶段路线图（修订版）

### 第 1 阶段（第 1-2 周）：质量基础 -- P0

- [ ] 后端：实现 Write Guard 写入守卫（复用 TextSimilarity）
- [ ] 后端：实现 `MemoryManagerRegistry` 全局管理器池
- [ ] 后端：发布 schema_version=2.2，新增 URI 路径 + 标签 + 活力值字段
- [ ] 前端：实现 300ms debounce 实时搜索

### 第 2 阶段（第 3-4 周）：组织增强 -- P1

- [ ] 后端：实现 Vitality Decay 活力衰减引擎（指数衰减 + 访问提升）
- [ ] 后端：实现 URI 路径解析和域管理逻辑
- [ ] 前端：重构为左侧域树 + 中间列表布局（DomainTree + MemoryWorkspace）
- [ ] 前端：实现标签筛选和搜索前缀语法（`@域` `#标签`）

### 第 3 阶段（第 5-6 周）：检索升级 -- P2

- [ ] 后端：接入 SQLite FTS5 Sidecar 索引
- [ ] 后端：实现简单意图识别（关键词评分法）
- [ ] 前端：Snapshot Diff 视图（NDiffViewer 组件）
- [ ] 前端：批量操作条（多选/删除/导出/重新分类）

### 第 4 阶段（第 7-8 周）：体验升级 -- P3

- [ ] 前端：活力值可视化（VitalityBadge + 衰减趋势图）
- [ ] 前端：虚拟滚动 + 骨架屏
- [ ] 前端：ARIA 标注完善 + 键盘导航
- [ ] 后端：压测 1k/1w/5w 规模，定义全面重构触发条件

### 第 5 阶段（按需）：全面重构

- [ ] 后端：SQLite 主存储迁移 + 影子双写
- [ ] 前端：知识图谱组件集成
- [ ] 前端：统计仪表盘

---

## 关键风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 双写不一致 | 定时校验任务（条数 + hash）+ 告警 |
| 迁移失败 | 保留只读 JSON 回退通道 |
| 中文分词效果 | 评估 unicode61/自定义分词，做召回 A/B 测试 |
| 管理器池内存 | Weak 引用 + TTL 回收 + 池大小上限 |
| Write Guard 误判 | 可配置阈值 + NOOP 条目记录到日志供审查 |
| 活力衰减过激 | 半衰期/阈值可配置 + 清理前需确认 + 保留恢复通道 |
| URI 路径迁移 | 旧记忆默认归入 `legacy://uncategorized`，不强制迁移 |

---

## 触发全面重构的条件

- 记忆条目稳定超过 2w 条
- P95 查询延迟 > 200ms
- 并发写入场景显著增加
- JSON 文件体积 > 50MB

---

## 附录

### 分类方案演变历程

1. **初始方案**：4 种固定枚举（Rule/Preference/Pattern/Context）
2. **Codex 提议**：三维标签（project/function/event 数组）
3. **5 种备选方案研究**：多维标签、类型对象、双向链接、面向切面分类、AI 混合分类
4. **Memory-Palace 启发**：URI 路径体系（domain://path）
5. **最终采纳**：URI 路径 + 自由标签混合方案（兼顾层级浏览和横向筛选）

### Memory-Palace 关键参数参考

```
# Write Guard
WRITE_GUARD_SEMANTIC_THRESHOLD = 0.85
WRITE_GUARD_KEYWORD_THRESHOLD = 0.70
WRITE_GUARD_LLM_ENABLED = false  # 可选

# Vitality Decay
VITALITY_MAX_SCORE = 3.0
VITALITY_DECAY_HALF_LIFE_DAYS = 30
VITALITY_CLEANUP_THRESHOLD = 0.35
VITALITY_CLEANUP_INACTIVE_DAYS = 14
VITALITY_ACCESS_BOOST = 0.5

# Intent Recognition
INTENT_TYPES = ["factual", "exploratory", "temporal", "causal"]
DEFAULT_INTENT = "factual"
FALLBACK_INTENT = "unknown"
```

---

## Claude-Mem 参考研究（第 4 参考源）

> 项目地址：https://github.com/thedotmack/claude-mem
> 文档站点：https://docs.claude-mem.ai
> 技术栈：Node.js + SQLite + Claude Agent SDK
> 定位：Claude Code 插件，提供持久化记忆压缩系统
> 版本：v10.x（截至 2026-02-19）

### Claude-Mem 核心架构

| 层 | 技术 | 关键能力 |
|---|---|---|
| 存储 | SQLite | sessions + observations + summaries + FTS5 搜索 |
| 钩子 | Claude Code Hooks API | 5 阶段生命周期（SessionStart/UserPrompt/PostToolUse/SubagentStop/Stop） |
| MCP | 5 个工具 | search/timeline/get_observations/get_session/list_sessions |
| 压缩 | Claude Agent SDK | 工具输出 AI 压缩（10x-20x 压缩比） |

### SQLite 数据库 Schema

| 表名 | 核心字段 | 用途 |
|------|----------|------|
| `sdk_sessions` | id, started_at, ended_at, project_path | 会话跟踪 |
| `user_prompts` | id, session_id(FK), content, timestamp | 用户输入记录 |
| `observations` | id, session_id(FK), tool_name, input, output, compressed_summary, tags | 工具执行观察 |
| `session_summaries` | id, session_id(FK), summary, key_decisions, files_changed | 会话摘要 |

FTS5 全文搜索索引覆盖 observations 和 session_summaries 表。

### 5 阶段钩子生命周期

1. **SessionStart** --> 从 SQLite 查询相关历史记忆，注入为初始上下文
2. **UserPromptSubmit** --> 记录用户输入到 user_prompts 表
3. **PostToolUse** --> 核心捕获点：记录工具输入/输出，异步 AI 压缩为语义观察
4. **SubagentStop** --> 子代理完成时汇总其观察结果
5. **Stop** --> 生成整个会话的结构化摘要（key_decisions + files_changed）

### 3 层渐进式披露（Progressive Disclosure）

Claude-Mem 的核心创新之一。通过 MCP 工具分层揭示信息，按需加载以节省 token：

| 层 | MCP 工具 | 返回内容 | Token 成本 | 类比 |
|----|----------|----------|------------|------|
| L1 | `search` | 紧凑索引（ID + 标题 + 标签 + 时间） | ~50 tokens/项 | 搜索引擎结果页 |
| L2 | `timeline` | 时间线上下文（摘要 + 相邻会话信息） | ~200 tokens/项 | Google 知识面板 |
| L3 | `get_observations` | 完整观察细节（按 ID 批量获取） | ~500-1000 tokens/项 | 点击进入详情页 |

**工作流**：AI 先调用 search 获取索引 → 根据相关性选择 ID → 调用 get_observations 获取详情 → 仅加载真正需要的内容。

### 压缩机制

- PostToolUse 捕获的原始工具输出通常 1000-10000 tokens
- 通过 Claude Agent SDK 异步压缩为 ~500 tokens 的语义观察
- 压缩比约 2x-20x
- 可配置跳过低价值工具（如 Read、Glob、Grep 的输出）
- 压缩策略：保留关键决策、文件变更、错误信息，丢弃冗余细节

### 值得借鉴的 4 大机制

#### 1. 渐进式披露 UI 模式（P1 -- 融入前端重构）

搜索结果分层展示，先紧凑索引后按需展开。对三术的启发：

- 记忆列表默认展示卡片视图（标题 + 分类 + 活力值徽章 + 创建时间）
- 点击卡片展开完整内容 + 标签 + 版本信息
- 再次点击可查看历史快照和 Diff

**适配方案**：在 MemoryCardList.vue 中实现 `collapsed → expanded → detail` 三态交互，使用 Naive UI 的 NCollapse 或自定义动画。

#### 2. 记忆摘要自动生成（P2 -- 新增功能）

长记忆（>500 字符）自动生成简短摘要，用于：
- 列表视图快速浏览
- MCP 回忆操作返回的压缩格式
- FTS5 索引的补充字段

**适配方案**：
- 后端新增 `summary: Option<String>` 字段到 MemoryEntry
- 写入时如果 content 长度 > 500 字符，通过 enhance 工具的降级链（Ollama → 云端 → 规则引擎）生成摘要
- 规则引擎降级：提取首行 + 关键词作为摘要

#### 3. 会话工具观察自动捕获（P3 -- 远期但技术可行）

三术的 MCP 服务器 ZhiServer 的 `call_tool` 方法已经是工具调用的统一入口，可在此插入后置钩子。

**Tauri 2.0 评估**：
- Plugin `on_event` 可监听事件循环
- Events `emit/listen` 可实现 MCP 工具调用后通知前端
- IPC 隔离层可拦截命令实现 middleware
- **结论：技术基础完全具备，但需要新增 SQLite 观察存储层和异步压缩队列**

**适配方案**（P3 阶段）：
1. 在 `server.rs` 的 `call_tool` 返回前，异步 emit 工具调用记录
2. 新增 `observation_store.rs` 模块，使用 rusqlite 存储观察
3. 异步压缩队列：tokio channel + worker 线程
4. 前端新增"会话时间线"视图

#### 4. Token 效率意识（P3 -- 软约束）

MCP 返回结果需关注 token 成本：
- `回忆` 操作已有压缩实现（`get_project_info()`），但可进一步优化
- 列表操作支持分页 + 摘要模式（仅返回 summary 而非全文）
- 搜索结果默认返回摘要，可选返回完整内容

### 与 Memory-Palace 的对比

| 维度 | claude-mem | Memory-Palace | 三术当前 |
|------|-----------|---------------|----------|
| 定位 | 会话级自动捕获 | 手动 + AI 辅助管理 | 手动全局知识库 |
| 存储 | SQLite | SQLite | JSON 文件 |
| 检索 | FTS5 + 渐进披露 | 混合检索（语义 + 关键词） | 模糊匹配（线性扫描） |
| 写入控制 | 自动（钩子驱动） | Write Guard（三级判定） | 去重后置（批量整理） |
| 生命周期 | 会话级（自动管理） | Vitality Decay | 无（硬上限 1000 条） |
| 压缩 | AI 压缩（Claude SDK） | 无 | 无 |
| Token 效率 | 3 层渐进披露 | 无特殊处理 | 回忆压缩格式 |

### 不建议直接照搬的部分

| 功能 | 原因 |
|---|---|
| Claude Code Hooks API | 三术不是 Claude Code 插件，需自建钩子机制 |
| Claude Agent SDK 压缩 | 三术已有 enhance 工具降级链（Ollama → 云端 → 规则引擎），可复用 |
| Node.js 实现 | 三术后端是 Rust，需用 rusqlite + tokio 重写 |
| 纯自动捕获模式 | 三术是"全局知识库"定位，需保留手动添加能力 |

---

## 更新后的交叉验证（4 源对比）

### 一致观点（强信号 -- 4 源共识）

1. **当前线性搜索的天花板**：4 源均认为模糊匹配 + 全量遍历在 1000+ 条时遇到瓶颈
2. **SQLite 是正确方向**：Codex（FTS5 Sidecar）、Memory-Palace（SQLAlchemy）、claude-mem（SQLite 主存储）均采用 SQLite
3. **渐进式优先策略**：4 源均推荐先增量优化再按需重构

### 新增共识（claude-mem 加入后）

4. **Token 效率是实际痛点**：claude-mem 的 3 层披露专门解决此问题，说明 MCP 返回格式的效率值得关注
5. **自动捕获是趋势**：claude-mem 和 Memory-Palace 都在"减少手动操作"方向发力
6. **压缩/摘要是长记忆的标配**：claude-mem 的 AI 压缩和 Memory-Palace 的 Write Guard 都在处理"信息过载"

### 分歧点更新

| 议题 | Codex | Gemini | Memory-Palace | claude-mem | 最终建议 |
|------|-------|--------|---------------|-----------|----------|
| 分类方式 | dimensions 标签 | 树形面板 | URI 路径 | 无（按会话自动归类） | URI 路径 + 自由标签 |
| 写入控制 | 去重后置 | 无 | Write Guard 前置 | 自动（钩子驱动） | P0 Write Guard + P3 自动捕获 |
| 检索策略 | FTS5 | 搜索语法 | 意图识别 | 渐进式披露 | FTS5 + 渐进披露 UI |
| 记忆生命周期 | 无 | 无 | Vitality Decay | 会话级自动管理 | Vitality Decay + 摘要压缩 |

---

## 更新后的优先级实施表

| 优先级 | 功能 | 来源 | 工时预估 |
|--------|------|------|----------|
| P0 | Write Guard（写入守卫） | Memory-Palace | 8h |
| P0 | MemoryManagerRegistry（全局池） | Codex 分析 | 12h |
| P1 | URI 路径体系 + 自由标签 | Memory-Palace + 面向切面分类 | 16h |
| P1 | Vitality Decay（活力衰减） | Memory-Palace | 10h |
| P1 | 前端树形浏览 + 标签筛选 + 渐进式披露 | Gemini + claude-mem | 14h |
| P2 | FTS5 检索 + 意图识别 | Codex + Memory-Palace | 16h |
| P2 | 记忆摘要自动生成 | claude-mem | 6h |
| P2 | Snapshot Diff 前端视图 | Memory-Palace | 8h |
| P3 | 会话工具观察自动捕获 | claude-mem + Tauri | 16h |
| P3 | 活力衰减前端可视化 | Memory-Palace | 6h |
| P3 | Token 效率优化（回忆返回格式） | claude-mem | 4h |
| P4 | 知识图谱可视化 | Gemini | TBD |
| P4 | SQLite 全面迁移（替换 JSON） | Codex | TBD |
| | **P0-P3 总计** | | **约 116h (14.5 工作日)** |
