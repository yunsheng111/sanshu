# OpenSpec 约束集：记忆管理系统全面优化

> 日期：2026-02-19
> 任务：记忆管理系统（ji 工具）全面优化
> 状态：研究完成，约束已分类
> 研究文档：`.doc/workflow/research/20260219-memory-optimization-analysis.md`

---

## 硬约束（不可违反）

| # | 约束描述 | 来源 | 影响范围 |
|---|----------|------|----------|
| HC-10 | 记忆内容大小限制：单条 <= max_entry_bytes（默认 10KB），总数 <= max_entries（默认 1000）。已有实现，不可放宽默认值 | 项目配置 | `manager.rs` L173/L182 |
| HC-11 | Write Guard 写入前必须执行相似度检查：阈值 >= 0.8 自动 NOOP（静默拒绝），0.6-0.8 自动 UPDATE（合并更新），< 0.6 正常 ADD。检查步骤在 `add_memory()` 内，save_store() 之前 | Memory-Palace 启发 | `manager.rs` `add_memory()` |
| HC-12 | 数据模型 v2.2 必须向后兼容 v2.1：所有新增字段（uri_path, domain, tags, vitality_score, last_accessed_at, summary）必须为 `Option<T>` 且配置 `#[serde(default)]`。启动时懒迁移，不修改未升级字段 | 架构原则 | `types.rs` `MemoryEntry` |
| HC-13 | MemoryManagerRegistry 全局池必须使用 `Weak<RwLock<MemoryManager>>` 引用 + TTL 回收（默认 30 分钟）+ 池大小上限（默认 16）。防止内存泄漏且确保不活跃项目的管理器被回收 | Codex 分析 | 新增 `registry.rs` |
| HC-14 | URI 路径格式必须遵循 `domain://path/segments` 规范。domain 限定为 `[a-z][a-z0-9_-]*`，path segments 不限字符集（支持中文）。无效格式写入时返回验证错误 | Memory-Palace | `types.rs` 新增字段 |
| HC-15 | Vitality Decay 清理操作（删除低活力记忆）必须经用户确认。自动标记清理候选（vitality_score < 0.35 且 last_accessed_at 超过 14 天），但执行删除前必须通过 MCP 返回候选列表并等待确认 | Memory-Palace | `manager.rs` 新增方法 |
| HC-16 | P0-P2 阶段 FTS5 索引作为 Sidecar，不替换 JSON 主存储。JSON 仍为真实数据源（source of truth），FTS5 仅用于加速搜索。写入操作必须先写 JSON 再更新 FTS5（双写一致性） | Codex 分析 | 新增 `fts_index.rs` |
| HC-17 | 前端组件必须适配 850px 最小窗口宽度。左侧域树面板可折叠（默认 200px，最小 0px），中间工作区最小 500px。使用 Naive UI NLayoutSider 的 collapse 特性 | Gemini 分析 + Tauri 窗口约束 | 前端组件 |
| HC-18 | SharedMemoryManager 的原子写入机制不可变更：所有写操作必须经过「写临时文件 → rename」流程。新增的 FTS5 索引写入失败不可阻塞 JSON 主存储写入 | 项目架构 | `manager.rs` `save_store()` |
| HC-19 | 旧记忆迁移策略：升级到 v2.2 时，已有记忆的新字段自动填充默认值（uri_path=None, domain=None, tags=None, vitality_score=Some(1.5), last_accessed_at=Some(updated_at), summary=None）。不强制迁移到新 URI 路径 | 架构原则 | `migration.rs` |

---

## 软约束（尽量满足）

| # | 约束描述 | 来源 | 优先级 |
|---|----------|------|--------|
| SC-4 | 更新操作（已实现）：支持 Patch（完全替换）和 Append（追加）两种模式 | 项目配置 | 已完成 |
| SC-5 | 版本兼容性检查（已实现）：启动时检查 schema_version 并自动升级 | 项目配置 | 已完成 |
| SC-6 | 版本控制与快照（已实现）：每次更新自动创建快照，最多保留 5 个 | 项目配置 | 已完成 |
| SC-7 | Embedding 语义相似度（已占位）：当 Ollama 可用时使用向量增强去重 | 项目配置 | 低 |
| SC-15 | Write Guard 阈值可配置：通过 MemoryConfig 暴露 write_guard_semantic_threshold（默认 0.8）和 write_guard_update_threshold（默认 0.6）。NOOP 条目记录到日志供审查 | Memory-Palace 参数参考 | 高 |
| SC-16 | Vitality Decay 参数可配置：半衰期（默认 30 天）、清理阈值（默认 0.35）、不活跃天数（默认 14 天）、访问提升值（默认 0.5）、最大活力值（默认 3.0）。通过 MemoryConfig 暴露 | Memory-Palace 参数参考 | 高 |
| SC-17 | 前端搜索支持前缀语法：`@domain`（按域过滤）、`#tag`（按标签过滤）、自然语言（全文搜索）。使用正则解析，失败时降级为全文搜索 | Gemini 分析 | 中 |
| SC-18 | 渐进式披露 UI：记忆卡片默认折叠（标题 + 分类 + 活力值），点击展开完整内容 + 标签 + 版本信息，再点击查看历史快照。使用 NCollapse 或自定义三态组件 | claude-mem 启发 | 中 |
| SC-19 | 记忆摘要自动生成：长记忆（>500 字符）写入时通过 enhance 降级链生成摘要。规则引擎降级：提取首行 + 关键词截断为 100 字符。摘要存入 summary 字段 | claude-mem 启发 | 中 |
| SC-20 | Token 效率优化：`回忆` 操作默认返回压缩格式（分类汇总 + 摘要），可选参数 `verbose=true` 返回完整内容。`列表` 操作支持 `page` + `page_size` 分页参数 | claude-mem 启发 | 低 |
| SC-21 | 批量操作支持：前端提供多选 + 批量删除 / 批量重新分类 / 批量导出功能。使用 NAffix 底部固定操作条 | Gemini 分析 | 中 |
| SC-22 | MemoryManagerRegistry 懒加载：首次请求某 project_path 时创建管理器并缓存，后续请求复用。支持 `project_path` 参数的 canonical 规范化（复用现有 `normalize_project_path`） | Codex 分析 | 高 |
| SC-23 | FTS5 中文分词：评估 unicode61 tokenizer 效果，如不满足则考虑 jieba-rs 自定义分词器。需 A/B 测试对比召回率 | Codex 分析 | 中 |
| SC-24 | 意图识别检索：搜索前通过关键词评分法识别查询意图（factual/exploratory/temporal/causal），不同意图采用不同检索策略（精确匹配/高召回/时间过滤/宽候选池） | Memory-Palace 启发 | 低 |
| SC-25 | 会话工具观察自动捕获：在 `server.rs` 的 `call_tool` 返回后异步记录工具名、输入参数、输出摘要。使用 tokio channel 异步写入，不阻塞主流程 | claude-mem + Tauri 评估 | 低 |
| SC-26 | Snapshot Diff 前端视图：展示记忆版本变更对比，使用文本 diff 算法高亮差异。可选集成 diff2html 或自定义组件 | Memory-Palace 启发 | 低 |

---

## 依赖约束（前置条件）

| # | 约束描述 | 依赖对象 | 执行顺序 |
|---|----------|----------|----------|
| DEP-01 | Write Guard（HC-11）依赖现有 TextSimilarity 算法。实现时复用 `similarity.rs` 的 `calculate_enhanced()` 方法，不引入新依赖 | `similarity.rs` | P0 第一步 |
| DEP-02 | MemoryManagerRegistry（HC-13）依赖现有 SharedMemoryManager。Registry 管理 SharedMemoryManager 实例池，不修改 SharedMemoryManager 内部逻辑 | `manager.rs` SharedMemoryManager | P0 第二步 |
| DEP-03 | 数据模型 v2.2（HC-12）必须在 P0 阶段完成 schema 升级，P1 的 URI 路径、标签、活力值字段依赖此升级 | `types.rs` MemoryEntry | P0 完成后才能开始 P1 |
| DEP-04 | 前端树形浏览（P1 前端）依赖 URI 路径体系（P1 后端）。后端 API 返回域/路径树结构后，前端才能渲染 DomainTree 组件 | P1 后端 URI 路径实现 | P1 后端先于 P1 前端 |
| DEP-05 | FTS5 检索（P2）依赖 SQLite 引入项目。需在 Cargo.toml 添加 `rusqlite` 依赖，并在 P0 阶段预留 SQLite 初始化代码路径 | `Cargo.toml` rusqlite | P2 开始前完成依赖引入 |
| DEP-06 | 记忆摘要自动生成（SC-19）依赖 enhance 工具的降级链。摘要生成复用 `enhance/chat_client.rs` 的统一 Chat 接口 | `enhance/` 模块 | P2 开始前确认 enhance 可用 |
| DEP-07 | 会话自动捕获（SC-25）依赖 FTS5（P2）和 MemoryManagerRegistry（P0）。观察记录存入 SQLite，检索依赖 FTS5 索引 | P0 Registry + P2 FTS5 | P3 开始前 P0+P2 完成 |
| DEP-08 | Snapshot Diff 前端视图（SC-26）依赖已有的版本控制机制（SC-6）。需在 MCP 接口新增 `获取快照` 操作，前端才能渲染 Diff | SC-6 快照机制 | P2 开始时新增 MCP 操作 |

---

## 风险约束（需要防护）

| # | 风险描述 | 概率 | 影响 | 缓解措施 |
|---|----------|------|------|----------|
| RISK-01 | FTS5 索引与 JSON 主存储双写不一致：写入 JSON 成功但 FTS5 更新失败，导致搜索结果遗漏 | 中 | 高（搜索不准确） | 定时校验任务（每小时对比 JSON 条目数与 FTS5 行数 + hash），不一致时触发 FTS5 全量重建。HC-18 保证 FTS5 失败不阻塞 JSON 写入 |
| RISK-02 | 数据模型 v2.2 迁移失败：旧 JSON 文件格式异常导致反序列化崩溃 | 低 | 高（数据丢失） | HC-19 规定所有新字段 `Option<T>` + `serde(default)`，`upgrade_to_current()` 失败时保留只读 JSON 回退通道，不覆盖原文件 |
| RISK-03 | MemoryManagerRegistry 内存泄漏：Weak 引用未正确回收或 TTL 配置过长 | 中 | 中（内存增长） | HC-13 规定池大小上限 16 + TTL 30 分钟。添加 `McpMetrics` 计数器监控池大小，超过阈值触发强制回收 |
| RISK-04 | Write Guard 误判（假阳性）：语义不同但文本相似的记忆被错误拒绝 | 中 | 中（记忆丢失） | SC-15 规定阈值可配置 + NOOP 记录到日志。前端展示"最近被拒绝的记忆"供用户审查。误判时可手动降低阈值 |
| RISK-05 | Write Guard 误判（假阴性）：语义重复但文本差异大的记忆未被拦截 | 低 | 低（冗余数据） | 保留现有去重后置机制（`整理` 操作）作为兜底。SC-7 的 Embedding 语义相似度可在未来补强 |
| RISK-06 | Vitality Decay 活力衰减过激：重要但低频访问的记忆被错误标记为清理候选 | 中 | 高（关键知识丢失） | HC-15 规定清理必须经用户确认。SC-16 规定参数可配置。Rule 分类记忆的活力值下限设为 1.0（永不自动清理） |
| RISK-07 | 中文分词效果不佳：FTS5 默认 unicode61 tokenizer 对中文词组切分不准确 | 高 | 中（搜索召回率低） | SC-23 规定需 A/B 测试。备选方案：jieba-rs 自定义分词器、字符 N-gram 索引、或混合检索（FTS 召回 + 向量补召回） |
| RISK-08 | URI 路径迁移用户认知成本：用户需要理解和使用 `domain://path` 格式 | 中 | 中（使用门槛） | HC-19 规定旧记忆默认归入 `legacy://uncategorized`，不强制迁移。前端域树面板提供"快速分类"拖拽交互。MCP 工具支持自动推断 domain |
| RISK-09 | 前端渐进式披露增加交互复杂度：三态折叠（collapsed/expanded/detail）学习成本 | 低 | 低（用户体验） | 默认展示 expanded 状态（标题 + 前 100 字符预览），collapse/detail 为可选交互。添加键盘快捷键（Space 展开/收起） |
| RISK-10 | 会话自动捕获产生大量低价值观察：频繁工具调用（如 Read、Grep）生成过多噪音记录 | 高 | 中（存储膨胀 + 搜索噪音） | SC-25 规定可配置跳过列表（skip_tools: ["Read", "Glob", "Grep"]）。观察存储设置独立上限（默认 5000 条），超出按 FIFO 淘汰。活力衰减机制同样适用于观察记录 |

---

## 约束来源统计

- 用户需求：0 条（用户需求已通过研究文档转化）
- 项目配置/现有代码：6 条（HC-10, SC-4, SC-5, SC-6, SC-7, HC-18）
- Codex 分析：5 条（HC-13, HC-16, SC-22, SC-23, DEP-05）
- Gemini 分析：4 条（HC-17, SC-17, SC-21, RISK-09）
- Memory-Palace 参考：10 条（HC-11, HC-14, HC-15, HC-19, SC-15, SC-16, SC-24, SC-26, RISK-06, RISK-08）
- claude-mem 参考：6 条（SC-18, SC-19, SC-20, SC-25, RISK-10, DEP-06/07）
- 架构原则/交叉验证：8 条（HC-12, DEP-01~04, DEP-08, RISK-01~05, RISK-07）

---

## 双模型执行元数据

| 字段 | 值 |
|------|-----|
| dual_model_status | SUCCESS |
| degraded_level | null |
| missing_dimensions | [] |
| codex_session | 019c75e4-93db-75d0-81da-2d80630282a8 |
| gemini_session | e60f4a6b-1c88-4691-aa55-1b340335ec11 |

> 注：Codex 和 Gemini 分析在原始研究阶段完成，本约束集基于研究文档 + claude-mem 新增研究整合。

---

## SESSION_ID（供后续使用）

- CODEX_SESSION: 019c75e4-93db-75d0-81da-2d80630282a8
- GEMINI_SESSION: e60f4a6b-1c88-4691-aa55-1b340335ec11

---

## 下一步

运行 `/ccg:spec-plan` 将约束集转化为零决策计划
