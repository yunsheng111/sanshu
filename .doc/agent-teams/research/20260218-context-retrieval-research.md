# 上下文检索方案研究报告

**日期**: 2026-02-18（初始）→ 2026-02-19（双模型补充）→ 2026-02-19（v3 查漏补缺）
**研究类型**: Agent Teams 需求研究
**状态**: 已完成（v3 — 含查漏补缺优化）
**环境说明**: v1 基于 5 个开源项目文档分析；v2 通过 Codex + Gemini 并行探索三术源码，补充代码级约束；v3 补充安全/测试/配置/可访问性/i18n/资源限制等遗漏维度

---

## 执行摘要

本研究对比分析了 5 个开源项目的上下文检索与记忆管理方案，并通过三轮迭代分析三术源码，共识别出 **15 个硬约束、20 个软约束、9 个依赖关系和 10 个风险点**，提供可验证的成功判据。

**核心发现**:
1. **存储架构**: SQLite + FTS5 是中小型项目的最佳选择（零依赖、事务支持、全文搜索）
2. **检索策略**: 混合检索（向量 + 词法 + RRF 融合）显著优于单一模式；三术已有 `hybrid_search.rs` 基础
3. **记忆管理**: 优先级 + 触发条件系统可实现智能记忆加载
4. **上下文扩展**: 三阶段扩展（邻居 + 面包屑 + Import）减少多轮查询
5. **并发安全**: MemoryManager 使用 `&mut self` 无并发保护，多 MCP 客户端场景存在数据竞争风险
6. **错误处理**: MCP 工具缺少统一错误分类和降级反馈机制
7. **前端缺失**: 记忆管理 UI 完全缺失，搜索结果无预览和实时反馈
8. **安全性**: API 密钥明文存储、记忆内容无大小限制、资源无上限增长 🆕v3
9. **测试覆盖**: 后端 5 个核心模块无测试、前端无单元/E2E 测试框架 🆕v3
10. **可访问性/国际化**: 前端缺少 a11y 基线和 i18n 框架 🆕v3

**用户决策**: 暂不实施优化功能，仅记录研究结果供未来参考

---

## 研究对象

| 项目 | 技术栈 | 核心特性 | GitHub Stars |
|------|--------|----------|--------------|
| **nocturne_memory** | Python + SQLite + MCP | URI 路径系统、版本控制、优先级触发 | - |
| **claude-mem** | TypeScript + SQLite + Claude SDK | 自动捕获、AI 压缩、会话回滚 | - |
| **ContextWeaver** | TypeScript + LanceDB + Tree-sitter | 混合检索、AST 分片、三阶段扩展 | - |
| **fast-context-mcp** | Node.js + Windsurf API + ripgrep | AI 驱动搜索、多轮迭代、自动降级 | - |
| **Claude-Code-Workflow** | TypeScript + SQLite FTS5 | 多 CLI 编排、工作流管理、CodexLens | - |

---

## 技术对比矩阵

### 存储架构对比

| 方案 | 代表项目 | 优点 | 缺点 | 适用场景 |
|------|----------|------|------|----------|
| **SQLite + FTS5** | claude-mem, Claude-Code-Workflow | 零依赖、事务支持、全文搜索 | 向量检索需额外实现 | 中小型项目、关键词搜索为主 |
| **SQLite + 向量扩展** | nocturne_memory (可扩展) | 单文件、易备份、结构化 + 向量 | 向量性能不如专用库 | 混合查询、离线优先 |
| **LanceDB** | ContextWeaver | 嵌入式向量库、高性能、增量索引 | 需额外依赖、文件格式专有 | 大规模代码库、语义检索为主 |

**三术当前方案**: JSON (memories.json) + 本地索引 (.sanshu-index/)

**建议**: 保持 JSON 主存储（短期），增加 FTS5 索引支持关键词快速查找

---

### 检索策略对比

| 检索模式 | 实现方案 | 代表项目 | 性能 | 准确度 | 实现复杂度 |
|----------|----------|----------|------|--------|------------|
| **关键词精确** | SQLite FTS5 / ripgrep | Claude-Code-Workflow, fast-context-mcp | ⚡⚡⚡ | ⭐⭐⭐ | ⭐ |
| **语义向量** | Embedding + 余弦相似度 | ContextWeaver, 三术 (sou) | ⚡⚡ | ⭐⭐⭐⭐ | ⭐⭐ |
| **混合检索** | RRF 融合 (向量 + 词法) | ContextWeaver | ⚡⚡ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **AI 驱动** | LLM 生成搜索命令 | fast-context-mcp | ⚡ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

**三术当前方案**:
- ✅ 语义向量检索 (sou + ace-tool)
- ✅ 混合检索基础 (`hybrid_search.rs` — BM25 + 向量 + RRF 融合已实现) 🆕
- ❌ 缺少 FTS5 关键词精确搜索独立通道
- ❌ 缺少搜索结果缓存层

**建议**: 增加 FTS5 关键词搜索作为快速通道，在 `hybrid_search.rs` 基础上完善缓存

---

### 记忆管理功能对比

| 功能 | nocturne_memory | claude-mem | 三术 (ji) |
|------|-----------------|------------|-----------|
| **创建** | `create_memory` (URI + priority + disclosure) | 自动捕获 Claude 操作 | `mcp______ji` action=记忆 |
| **读取** | `read_memory` (支持 system://boot) | MCP search + 自动注入 | action=回忆 |
| **更新** | `update_memory` (Patch/Append 模式) | 自动压缩更新 | ❌ 缺少 |
| **删除** | `delete_memory` (切断路径，不删内容) | 手动清理 | action=删除 |
| **版本控制** | 快照 + 回滚 (Web 界面) | 会话回滚 | ❌ 缺少 |
| **优先级** | 1-10 级别 | ❌ | ❌ |
| **触发条件** | disclosure 字段 | ❌ | ❌ |
| **并发保护** | SQLite WAL 模式 | SQLite 事务 | ❌ `&mut self` 无保护 🆕 |

**三术缺失功能**:
1. ❌ 记忆更新机制（当前只能删除重建）
2. ❌ 版本控制与回滚
3. ❌ 记忆优先级系统（priority）
4. ❌ 触发条件系统（disclosure）
5. ❌ 并发访问保护（MemoryManager 使用 `&mut self`） 🆕

---

### 上下文扩展策略对比

| 项目 | E1 邻居扩展 | E2 面包屑补全 | E3 Import 解析 | Token 预算控制 |
|------|-------------|---------------|----------------|----------------|
| **ContextWeaver** | ✅ (同文件前后 chunks) | ✅ (同类/函数其他方法) | ✅ (跨文件依赖) | ✅ (Smart TopK) |
| **fast-context-mcp** | ❌ | ❌ | ❌ | ✅ (tree_depth 降级) |
| **三术 (sou)** | ❌ | ❌ | ❌ | ❌ |

**ContextWeaver Smart TopK 策略**:
- **Anchor & Floor**: 动态阈值 + 绝对下限双保险
- **Delta Guard**: 防止 Top1 outlier 场景误判
- **Safe Harbor**: 前 N 个结果只检查下限，保证基本召回

**建议**: 实现 E1 + E2，E3 作为可选功能（性能考虑）

---

## 约束集

### 硬约束 (MUST)

#### HC-1: 存储架构选择
必须在 SQLite + FTS5、SQLite + 向量扩展、LanceDB 三种方案中选择一种作为主存储。

**来源**: 开源项目对比分析
**三术建议**: 保持 JSON/SQLite，增加 FTS5 索引

#### HC-2: 记忆生命周期管理
必须实现记忆的 CRUD + 版本控制。

**来源**: 开源项目对比分析
**三术缺失**: 更新机制、版本控制、优先级、触发条件

#### HC-3: 上下文检索策略
必须支持至少两种检索模式（精确 + 语义）。

**来源**: 开源项目对比分析
**三术缺失**: FTS5 关键词精确搜索独立通道

#### HC-4: MCP 工具接口规范
MCP 工具必须遵循 Model Context Protocol 规范。

**来源**: 开源项目对比分析
**三术改进点**: 工具级详细文档、统一错误处理、参数验证

#### HC-5: MemoryManager 并发保护 🆕
`MemoryManager` 当前使用 `&mut self`，无并发访问保护。多 MCP 客户端同时操作时存在数据竞争风险，必须引入 `RwLock` 或 `Mutex` 保护。

**来源**: Codex 源码分析（`src/rust/mcp/tools/memory/manager.rs`）
**影响范围**: `manager.rs` 中所有 pub 方法
**参考方案**: `local_index.rs` 已使用 `RwLock<HashMap>` 保护索引数据

#### HC-6: MCP 工具统一错误分类 🆕
`retry_request` 当前仅按 HTTP 状态码分类错误，缺少语义化错误类型。所有 MCP 工具应实现统一的错误分类体系（网络错误/认证错误/限流/业务错误），支持精确降级策略。

**来源**: Codex 源码分析（`src/rust/mcp/tools/acemcp/mcp.rs` retry 逻辑）
**影响范围**: 所有 MCP 工具的错误处理路径

#### HC-7: 搜索结果缓存层 🆕
频繁的相似查询不应重复调用外部 API。必须实现内存级查询缓存（LRU + TTL），减少 Augment API 调用次数。

**来源**: Codex 源码分析 + 性能分析
**影响范围**: `sou` 工具 + `enhance` 工具

#### HC-8: 记忆管理 UI 🆕
当前 GUI 完全缺少记忆管理界面。用户无法可视化查看、搜索、编辑或删除已存储的记忆。必须提供基础记忆管理 UI。

**来源**: Gemini 前端分析
**影响范围**: 新增前端页面/组件

#### HC-9: API 密钥明文存储 🆕v3
`config.json` 中 `acemcp_token`、`enhance_api_key`、`context7_api_key`、`acemcp_proxy_password` 均为**明文 JSON 字符串**存储。任何有文件读取权限的进程或用户均可获取。

**来源**: Claude 源码分析（`src/rust/config/settings.rs:113-131`）
**影响范围**: 所有依赖外部 API 的 MCP 工具
**建议**: 使用操作系统密钥管理（Windows Credential Manager / macOS Keychain）或加密存储

#### HC-10: 记忆内容无长度/大小限制 🆕v3
`MemoryManager::add_memory()` 只检查 `content.is_empty()`，不限制内容长度。恶意或失控的 AI 可写入巨大记忆条目导致 `memories.json` 膨胀。

**来源**: Claude 源码分析（`src/rust/mcp/tools/memory/manager.rs:139-142`）
**影响范围**: `memories.json` 文件大小、内存占用
**建议**: 添加单条记忆最大长度限制（如 10KB）和总条目数上限（如 1000 条）

#### HC-11: MCP 工具关键模块测试覆盖缺失 🆕v3
以下模块完全缺少单元测试或集成测试：
- `enhance/core.rs` — Prompt 增强核心逻辑
- `acemcp/mcp.rs` — 代码索引和搜索核心
- `context7/` — 文档查询全部模块
- `interaction/mcp.rs` — zhi 工具核心
- `skills/mod.rs` — 技能运行时

**来源**: Claude 源码分析 + 项目 CLAUDE.md 测试策略
**影响范围**: 回归测试保障缺失，代码变更无安全网

#### HC-12: 可访问性(A11y)基线缺失 🆕v3
前端组件缺少 `aria-*` 标签、键盘导航逻辑、焦点管理。未安装 `eslint-plugin-vuejs-accessibility`。

**来源**: Gemini 前端分析
**影响范围**: 视障用户无法使用上下文检索功能
**建议**: 定义检索结果列表的焦点陷阱；为搜索输入框添加 `aria-label`；确保搜索建议在屏幕阅读器中实时播报

#### HC-13: 国际化(i18n)框架缺失 🆕v3
未安装 `vue-i18n`，UI 文本全部硬编码中文（包括 `test_simple_popup.json` 等配置）。阻碍国际化推广。

**来源**: Gemini 前端分析
**影响范围**: 非中文用户无法使用
**建议**: 引入 `vue-i18n` + `@intlify/unplugin-vue-i18n`；建立检索专用 locale 文件（中/英）

#### HC-14: 前端单元/E2E 测试覆盖真空 🆕v3
仅有组件样式测试环境（`pnpm test:ui`），`package.json` 缺乏 Vitest/Jest 依赖，`src/frontend/test/` 目录结构非标准，无 E2E 测试、无可视化回归测试。

**来源**: Gemini 前端分析
**影响范围**: 前端代码变更无测试保障
**建议**: 引入 Vitest + @vue/test-utils 进行 Composables 单元测试；使用 Playwright 进行 E2E 测试

#### HC-15: 资源无上限增长 🆕v3
- `memories.json` 无条目数量上限，大项目可能无限增长
- `.sanshu-index/` 无磁盘空间限制
- Icon `SEARCH_CACHE`（`src/rust/mcp/tools/icon/api.rs:42`）仅有 TTL 过期无容量上限（可无限增长内存占用）

**来源**: Claude 源码分析
**影响范围**: 内存和磁盘资源
**建议**: 为各缓存/存储添加容量上限 + LRU 淘汰策略

---

### 软约束 (SHOULD)

#### SC-1: 智能上下文扩展
应支持上下文自动扩展，减少多轮查询。

**来源**: 开源项目对比分析
**建议**: 实现 E1 邻居扩展 + E2 面包屑补全

#### SC-2: Token 预算控制
应支持 Token 感知的结果截断。

**来源**: 开源项目对比分析
**建议**: 实现 Smart TopK 或简单的行数/字符数截断

#### SC-3: 增量索引与实时更新
应支持文件变更时增量更新索引。

**来源**: 开源项目对比分析
**三术优势**: 已实现增量索引 + 文件监听

#### SC-4: 多语言 AST 解析
应支持主流编程语言的 AST 语义分片。

**来源**: 开源项目对比分析
**建议**: 可选功能，优先级低于核心检索优化

#### SC-5: 记忆更新机制 🆕
应支持 Patch/Append 模式更新记忆，避免删除重建。

**来源**: Codex 源码分析 + nocturne_memory 对比
**优先级**: 高

#### SC-6: 记忆版本控制 🆕
应支持记忆的快照与回滚，防止误操作导致数据丢失。

**来源**: Codex 源码分析 + claude-mem 对比
**优先级**: 中

#### SC-7: Embedding 相似度检测 🆕
当前相似度检测基于字符串（Levenshtein + Phrase + Jaccard），应增加 Embedding 语义相似度作为补充，提升去重准确度。

**来源**: Codex 源码分析（`similarity.rs`）
**优先级**: 低

#### SC-8: 磁盘级查询缓存 🆕
内存缓存（HC-7）之外，高频查询结果应支持持久化缓存到磁盘，减少冷启动后的重复查询。

**来源**: Codex 性能分析
**优先级**: 中

#### SC-9: 搜索结果 UI 预览 🆕
搜索结果应在 GUI 中提供代码片段预览、高亮关键词、文件路径面包屑，而非仅返回文本。

**来源**: Gemini 前端分析
**优先级**: 中

#### SC-10: 搜索实时反馈 🆕
搜索过程中应提供进度指示（加载动画、已检索文件数、预计剩余时间），改善长时间搜索的用户体验。

**来源**: Gemini 前端分析
**优先级**: 中

#### SC-11: 响应式设计适配 🆕
记忆管理和搜索结果 UI 应适配不同窗口尺寸，确保在小屏幕和分屏模式下可用。

**来源**: Gemini 前端分析
**优先级**: 低

#### SC-12: 全局任务/状态中心 🆕
应提供统一的任务状态面板，展示 MCP 工具运行状态、索引进度、API 健康度，替代当前分散的状态信息。

**来源**: Gemini 前端分析
**优先级**: 低

#### SC-13: MCP Server 配置热更新 🆕v3
`load_standalone_config()` 每次从文件读取无缓存；MCP server（三术 bin）运行时不支持配置变更通知，工具启用/禁用需重启服务。

**来源**: Claude 源码分析（`src/rust/config/storage.rs:125-142`）
**优先级**: 中

#### SC-14: 结构化可观测性指标 🆕v3
缺少指标收集：搜索延迟直方图、缓存命中率、API 错误率统计、工具调用频次。当前仅有 `call_id + elapsed_ms` 日志。

**来源**: Claude 源码分析（`src/rust/mcp/server.rs:423-452`）
**优先级**: 中

#### SC-15: 数据迁移策略 🆕v3
`MemoryStore version="2.0"` 无 schema 版本校验逻辑（仅有旧 MD→JSON 迁移）。若升级到 SQLite 存储，JSON→SQLite 迁移路径未定义。`memories.json` 破坏性变更无法检测和自动修复。

**来源**: Claude 源码分析（`src/rust/mcp/tools/memory/types.rs:62-85`, `migration.rs`）
**优先级**: 高

#### SC-16: 前端状态持久化 🆕v3
项目采用 Composables 模式（如 `useMcpHandler.ts`），缺乏全局状态持久化（如 Pinia），跨组件检索上下文可能丢失。Tauri 重启后检索历史无法恢复。

**来源**: Gemini 前端分析
**优先级**: 中

#### SC-17: IPC 弹性错误处理 🆕v3
`useMcpHandler.ts` 直接调用 Tauri API，未针对 IPC 延迟、超时或 Rust 端崩溃设计细粒度降级策略。应实现 `SafeInvoke` 包装器和前端错误边界。

**来源**: Gemini 前端分析
**优先级**: 高

#### SC-18: 检索性能 Worker 化 🆕v3
`markdown-it` 渲染和语法高亮在主线程执行。大量检索结果时可能导致 UI 卡顿。应移入 Web Worker。已安装 `@tanstack/vue-virtual` 但计算开销未 Worker 化。

**来源**: Gemini 前端分析
**优先级**: 低

#### SC-19: Skill 执行安全性增强 🆕v3
`skills/mod.rs` 对 Python 脚本执行有 canonicalize 路径检查，但缺少 stdout 大小限制（`String::from_utf8_lossy(&output.stdout)` 可能无限大）和执行超时配置化。

**来源**: Claude 源码分析（Skills CLAUDE.md 安全沙箱段落）
**优先级**: 中

#### SC-20: 配置文件损坏恢复 🆕v3
`config.json` 反序列化失败时 `load_standalone_config()` 回退到默认配置，但不备份损坏文件也不通知用户。用户可能不知道自定义配置已丢失。

**来源**: Claude 源码分析（`src/rust/config/storage.rs:125-142`）
**优先级**: 低

---

## 依赖关系

### DEP-1: 外部 API 依赖

| 项目 | 依赖 API | 用途 | 降级方案 |
|------|----------|------|----------|
| **ContextWeaver** | SiliconFlow (Embedding + Rerank) | 向量生成 + 精排 | 本地 Ollama |
| **fast-context-mcp** | Windsurf Devstral | AI 驱动搜索 | ❌ 无降级 |
| **三术 (sou)** | Augment ACE | 语义检索 | 本地索引 |
| **三术 (enhance)** | Augment chat-stream | Prompt 增强 | ace-tool → Claude 自增强 |

**三术依赖风险**: Augment API 不可用时，sou 工具完全失效

**建议**: 为 sou 增加本地 Embedding 降级方案（Ollama）

---

### DEP-2: 存储格式兼容性

| 项目 | 存储格式 | 迁移难度 | 备份方案 |
|------|----------|----------|----------|
| **nocturne_memory** | SQLite (单文件) | ⭐ | 文件复制 |
| **claude-mem** | SQLite (单文件) | ⭐ | 文件复制 |
| **ContextWeaver** | LanceDB (目录) | ⭐⭐⭐ | 目录打包 |
| **三术 (ji)** | JSON (单文件) | ⭐ | 文件复制 |
| **三术 (sou)** | 自定义索引 (目录) | ⭐⭐ | 目录打包 |

**三术优势**: JSON 格式易于迁移和版本控制

---

### DEP-3: 并发保护 → 多客户端支持 🆕
MemoryManager 并发保护（HC-5）是支持多 MCP 客户端同时连接的前提条件。必须先完成 HC-5 再开放多客户端能力。

**来源**: Codex 源码分析
**依赖对象**: HC-5 (MemoryManager 并发保护)
**执行顺序**: 先于多客户端支持

### DEP-4: 错误分类 → 智能降级 🆕
统一错误分类体系（HC-6）是实现精确降级策略的前提。当前 retry 逻辑无法区分"应重试"和"应降级"的错误。

**来源**: Codex 源码分析
**依赖对象**: HC-6 (统一错误分类)
**执行顺序**: 先于降级策略优化

### DEP-5: FTS5 索引 → 混合检索 🆕
FTS5 关键词索引是混合检索（向量 + 词法 + RRF）的前提。`hybrid_search.rs` 中的 BM25 部分需要 FTS5 支撑才能发挥最佳效果。

**来源**: Codex 源码分析 + ContextWeaver 对比
**依赖对象**: HC-1 (FTS5 索引), HC-3 (双模式检索)
**执行顺序**: 先于混合检索优化

### DEP-6: 记忆 CRUD API → 记忆管理 UI 🆕
记忆管理 UI（HC-8）依赖后端完善的 CRUD API（HC-2 + SC-5）。前端界面需要更新、批量操作、搜索过滤等后端接口支持。

**来源**: Gemini 前端分析
**依赖对象**: HC-2 (CRUD), SC-5 (更新机制)
**执行顺序**: 先于 UI 开发

### DEP-7: 密钥安全存储 → API 工具信任度 🆕v3
API 密钥安全存储（HC-9）是公开发布前的必要条件。明文存储的密钥在开源环境中极易泄露。

**来源**: Claude 源码分析
**依赖对象**: HC-9 (密钥明文)
**执行顺序**: 先于公开发布

### DEP-8: 前端测试框架 → 前端 UI 开发保障 🆕v3
前端测试框架（HC-14）是大规模前端功能开发的前提。记忆管理 UI（HC-8）等新增功能需测试保障。

**来源**: Gemini 前端分析
**依赖对象**: HC-14 (前端测试缺失) → HC-8 (记忆 UI)
**执行顺序**: 先于大规模前端功能开发

### DEP-9: i18n 框架 → 国际化推广 🆕v3
国际化框架（HC-13）是英文版发布和国际社区推广的前提。

**来源**: Gemini 前端分析
**依赖对象**: HC-13 (i18n 缺失)
**执行顺序**: 先于英文版发布

---

## 风险识别

### R-1: 性能风险

**大规模代码库检索性能**:
- ContextWeaver: 通过 LanceDB 向量索引优化，支持 10k+ 文件
- fast-context-mcp: 依赖 Windsurf API，本地无性能瓶颈
- 三术: 本地索引 + 外部 API，性能取决于网络延迟

**缓解措施**:
1. 实现本地缓存（查询结果 TTL）
2. 增加 FTS5 快速通道（关键词查询 <100ms）
3. 异步索引更新（不阻塞主流程）

---

### R-2: 数据一致性风险

**记忆去重与冲突**:
- nocturne_memory: 相似度检测 + 静默拒绝
- claude-mem: 自动压缩 + 会话隔离
- 三术: 相似度检测 + 去重配置

**三术当前方案**: 已实现相似度检测（similarity.rs）

**缓解措施**:
1. 增加去重日志（记录被拒绝的记忆）
2. 提供手动合并工具（Web 界面）
3. 定期去重任务（dedup_on_startup）

---

### R-3: 用户体验风险

**学习曲线**:
- nocturne_memory: URI 路径系统需要理解
- claude-mem: 自动化程度高，学习成本低
- ContextWeaver: 参数众多，需要调优
- 三术: 工具分散，缺少统一入口

**缓解措施**:
1. 提供默认配置（开箱即用）
2. 增加交互式配置向导（ccg:init）
3. 完善文档与示例（GETTING_STARTED.md）

---

### R-4: 并发数据竞争 🆕

**风险描述**: MemoryManager 使用 `&mut self` 无并发保护，多 MCP 客户端同时读写 `memories.json` 可能导致数据丢失或文件损坏。

**概率**: 中（当前单客户端场景低，但多客户端场景高）
**影响**: 高（数据不可逆丢失）

**缓解措施**:
1. 引入 `RwLock<MemoryManager>` 包装（参考 `local_index.rs` 实现）
2. JSON 文件写入采用原子操作（先写临时文件，再 rename）
3. 增加文件锁（`flock`）防止跨进程冲突

**来源**: Codex 源码分析

---

### R-5: 索引重建期间检索质量下降 🆕

**风险描述**: 大规模代码库索引重建期间（如 git checkout 切换分支），搜索结果可能不完整或返回过时内容。

**概率**: 中
**影响**: 中（影响搜索准确度但不影响数据安全）

**缓解措施**:
1. 实现增量索引（已有基础），缩小重建范围
2. 索引重建期间标记结果为"可能不完整"
3. 保留旧索引直到新索引就绪（双缓冲策略）

**来源**: Codex 性能分析

---

### R-6: 前端复杂度膨胀 🆕

**风险描述**: 记忆管理 UI + 搜索结果预览 + 状态中心等新增功能可能导致前端代码复杂度快速增长，现有 44 个组件已具一定规模。

**概率**: 中
**影响**: 中（维护成本增加）

**缓解措施**:
1. 组件化设计，按功能域拆分（记忆管理/搜索/状态面板）
2. 复用 Naive UI 组件库，减少自定义组件
3. 采用 composable 模式管理状态（已有 18 个 composables）

**来源**: Gemini 前端分析

---

### R-7: API 密钥泄露 🆕v3

**风险描述**: `config.json` 中 API 密钥明文存储，在多用户共享机器、备份泄露或开源项目 fork 场景下可能导致密钥泄露。

**概率**: 中
**影响**: 高（经济损失 + 安全事件）

**缓解措施**:
1. 使用操作系统密钥管理（Windows Credential Manager / macOS Keychain）
2. 配置文件权限收紧（仅当前用户可读）
3. 添加 `.gitignore` 排除配置文件（已有）

**来源**: Claude 源码分析

---

### R-8: 测试覆盖不足导致回归 Bug 🆕v3

**风险描述**: 后端 5 个核心 MCP 工具模块（enhance、acemcp、context7、interaction、skills）和整个前端缺少自动化测试。代码变更时无法检测回归。

**概率**: 高
**影响**: 中（功能回归、用户体验劣化）

**缓解措施**:
1. 分阶段补测试：先核心路径（记忆 CRUD、搜索、zhi 交互）后边缘场景
2. CI 集成 `cargo test` + Vitest
3. 关键路径建立 E2E 测试

**来源**: Claude + Gemini 分析

---

### R-9: memories.json 膨胀影响性能 🆕v3

**风险描述**: 无条目上限的 `memories.json` 在长期使用或多 AI 客户端场景下可能增长到数 MB 甚至数十 MB，导致加载缓慢、搜索延迟增加。

**概率**: 中
**影响**: 中（性能劣化）

**缓解措施**:
1. 添加条目上限（如 1000 条）+ 超限自动归档旧记忆
2. 单条记忆大小限制（如 10KB）
3. 定期去重 + 压缩

**来源**: Claude 源码分析

---

### R-10: IPC 超时导致用户无感等待 🆕v3

**风险描述**: Tauri IPC 调用无超时保护，Rust 端无响应时（如 Ollama 模型加载卡死、外部 API 超时），前端用户处于"无感等待"状态。

**概率**: 中
**影响**: 高（用户体验极差，可能误以为应用崩溃）

**缓解措施**:
1. 实现 `SafeInvoke` 包装器（带超时 + 取消按钮）
2. 前端错误边界（Error Boundary）
3. IPC 调用进度指示器

**来源**: Gemini 前端分析

---

## 可借鉴的设计模式

### 1. nocturne_memory 的 URI 路径系统

**设计理念**: 使用类似文件系统的路径组织记忆

```
core://agent/coding_style
user://preferences/theme
project://myapp/architecture
system://boot  # 启动时自动加载
```

**优点**:
- 层级清晰，易于理解
- 支持通配符查询（`core://agent/*`）
- 优先级与路径解耦

**适用场景**: 需要复杂记忆组织的场景

---

### 2. ContextWeaver 的混合检索架构

**设计理念**: 向量召回 + 词法召回 + RRF 融合 + Rerank 精排

```
1. 向量召回 (top 30) → 语义相关
2. 词法召回 (top 30) → 精确匹配
3. RRF 融合 (top 40)  → 综合排序
4. Rerank 精排 (top 10) → 交叉编码器
5. Smart TopK 截断    → 智能过滤
```

**优点**:
- 兼顾语义和精确匹配
- RRF 融合算法简单高效
- Rerank 显著提升准确度

**适用场景**: 对检索准确度要求高的场景

**三术映射**: `hybrid_search.rs` 已实现步骤 1-3（BM25 + 向量 + RRF），缺少 Rerank 和 Smart TopK

---

### 3. claude-mem 的自动捕获机制

**设计理念**: 监听 Claude 操作，自动提取关键信息

```typescript
// 自动捕获场景
- 用户明确说"记住这个"
- Claude 生成重要决策
- 代码变更的关键上下文
```

**优点**:
- 零学习成本
- 自动压缩（AI 提取关键信息）
- 会话隔离（避免污染）

**适用场景**: 面向普通用户的记忆系统

---

### 4. fast-context-mcp 的 AI 驱动搜索

**设计理念**: LLM 生成搜索命令，而非直接搜索

```
用户查询 → LLM 生成 ripgrep 命令 → 执行 → 返回结果
```

**优点**:
- 自然语言查询
- 自动降级（tree_depth 调整）
- 多轮迭代优化

**适用场景**: 需要灵活查询的场景

---

### 5. local_index.rs 的 RwLock 模式 🆕

**设计理念**: 读多写少场景使用 `RwLock` 保护共享数据

```rust
// 已在三术代码库中实现
pub struct LocalIndex {
    entries: RwLock<HashMap<String, IndexEntry>>,
}
```

**优点**:
- 读操作无阻塞（多个读者并发）
- 写操作独占（保证数据一致性）
- Rust 所有权系统编译期保证安全

**适用场景**: MemoryManager 可直接参考此模式

---

## 实施优先级建议 🆕v3

基于 Codex（后端）+ Gemini（前端）+ Claude（查漏补缺）三轮分析，推荐以下优先级排序：

| 优先级 | 约束 | 说明 | 预期收益 |
|--------|------|------|----------|
| **P0** | HC-5, HC-6, HC-9, HC-10, HC-15 | 并发保护 + 错误分类 + 密钥安全 + 记忆大小限制 + 资源上限 | 消除数据竞争、安全漏洞和资源泄露风险 |
| **P1** | HC-7, HC-11, SC-5, SC-15, SC-17, HC-8 | 缓存 + 测试补全 + 记忆更新 + 数据迁移 + IPC 弹性 + 记忆 UI | 建立安全网，改善性能和用户体验 |
| **P2** | HC-14, SC-13, SC-14, SC-16, SC-8, SC-6, SC-9, SC-10 | 前端测试 + 配置热更新 + 可观测性 + 状态持久化 + 磁盘缓存 + 版本控制 + 搜索 UX | 完善功能矩阵和工程基础设施 |
| **P3** | HC-12, HC-13, SC-7, SC-11, SC-12, SC-18, SC-19, SC-20 | A11y + i18n + Embedding 相似度 + 响应式 + 状态中心 + Worker 化 + Skill 安全 + 配置恢复 | 国际化准备和锦上添花 |

---

## 可验证成功判据

### 功能完整性

- [ ] 支持记忆的 CRUD + 版本控制
- [ ] 支持关键词 + 语义双模式检索
- [ ] 支持上下文自动扩展（E1 + E2）
- [ ] 支持 Token 预算控制与截断
- [ ] 支持增量索引与文件监听
- [ ] MemoryManager 并发安全（RwLock） 🆕
- [ ] MCP 工具统一错误分类体系 🆕
- [ ] 搜索结果缓存（内存 LRU + 可选磁盘） 🆕
- [ ] 记忆管理 GUI 界面 🆕
- [ ] 记忆内容大小限制（单条 ≤ 10KB，总条目 ≤ 1000） 🆕v3
- [ ] 各缓存/存储容量上限 + LRU 淘汰策略 🆕v3

### 安全性 🆕v3

- [ ] API 密钥使用操作系统安全存储（非明文 JSON）
- [ ] 记忆内容写入有大小限制和条目上限
- [ ] Skill 脚本执行有 stdout 大小限制和超时配置
- [ ] 配置文件损坏时备份原文件并通知用户

### 测试覆盖 🆕v3

- [ ] 后端核心模块测试覆盖：enhance、acemcp、context7、interaction、skills
- [ ] 前端单元测试框架就绪（Vitest + @vue/test-utils）
- [ ] 前端 E2E 测试框架就绪（Playwright）
- [ ] CI 集成 `cargo test` + `pnpm test`

### 性能指标

- [ ] 关键词查询响应时间 < 100ms
- [ ] 语义查询响应时间 < 2s
- [ ] 索引更新延迟 < 5s（单文件变更）
- [ ] 支持 10k+ 文件代码库
- [ ] 缓存命中时查询响应 < 10ms 🆕
- [ ] IPC 调用有超时保护（默认 30s） 🆕v3

### 用户体验

- [ ] 默认配置开箱即用
- [ ] 错误信息包含诊断 + 建议 + 降级状态 🆕
- [ ] 提供 Web 可视化界面（记忆管理 + 搜索预览） 🆕
- [ ] 文档覆盖所有 MCP 工具
- [ ] 搜索过程实时反馈 🆕
- [ ] 前端组件基础可访问性（aria-label、键盘导航） 🆕v3
- [ ] 国际化框架就绪（中/英双语） 🆕v3
- [ ] 配置热更新无需重启 MCP Server 🆕v3

---

## 开放问题（已确认）

用户选择"暂不实施，仅记录"，以下问题供未来参考：

1. **AST 语义分片支持**: 是否需要 Tree-sitter 解析？（优先级：低）
2. **本地 Embedding 降级**: 是否需要 Ollama 支持？（优先级：中）
3. **Web 可视化界面**: 是否需要记忆管理 UI？（优先级：低 → 中 🆕 Gemini 建议提升）
4. **记忆优先级系统**: 是否需要 priority + disclosure？（优先级：中）
5. **混合检索融合**: 是否需要 RRF + Rerank？（优先级：高）
6. **MemoryManager 并发保护时机**: 当前单客户端是否已有风险？（优先级：高 🆕）
7. **错误分类粒度**: 区分多少种错误类型？（优先级：高 🆕）
8. **密钥安全存储方案选择**: Windows Credential Manager vs 加密文件？（优先级：高 🆕v3）
9. **测试补全优先级**: 先补后端还是先建前端测试框架？（优先级：高 🆕v3）
10. **数据迁移路径**: JSON→SQLite 迁移是否需要向前兼容？（优先级：中 🆕v3）
11. **i18n 范围**: 仅 UI 文本还是包含错误消息和日志？（优先级：低 🆕v3）
12. **资源上限阈值**: memories.json 条目上限 1000 条是否合理？Icon 缓存容量多大？（优先级：中 🆕v3）

---

## 参考资源

### 项目链接

- [nocturne_memory](https://github.com/Dataojitori/nocturne_memory)
- [ContextWeaver](https://github.com/hsingjui/ContextWeaver)
- [claude-mem](https://github.com/JillVernus/claude-mem)
- [fast-context-mcp](https://github.com/SammySnake-d/fast-context-mcp)
- [Claude-Code-Workflow](https://github.com/catlog22/Claude-Code-Workflow)

### 技术文档

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
- [LanceDB](https://lancedb.com/)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)

---

## 双模型执行元数据 🆕

### v2 元数据

| 字段 | 值 |
|------|-----|
| dual_model_status | SUCCESS |
| degraded_level | null |
| missing_dimensions | [] |
| codex_session | `019c71b9-59dc-79c1-bb37-23744264bd28` |
| gemini_session | `b53b93da-6e08-46c9-9b33-9bad43bd403d` |

### v3 元数据 🆕

| 字段 | 值 |
|------|-----|
| dual_model_status | PARTIAL_SUCCESS |
| degraded_level | codex_timeout → claude_self_analysis |
| missing_dimensions | [] |
| codex_session | 超时未返回（>5min，已停止） |
| gemini_session | `2ee80b46-0dbe-448d-86d1-76a75d4be386` |
| enhance_mode | Claude 自增强（mcp______enhance 失败） |

### Codex 分析摘要（后端视角）

- **并发安全**: `MemoryManager` 使用 `&mut self` 无并发保护，`local_index.rs` 已有 `RwLock` 可参考
- **错误处理**: `retry_request` 仅按 HTTP 状态码分类，缺少语义化错误类型
- **缓存缺失**: 搜索和增强工具无查询结果缓存，重复查询浪费 API 配额
- **混合检索**: `hybrid_search.rs` 已实现 BM25 + 向量 + RRF 基础，缺少 Rerank 和 Smart TopK
- **记忆系统**: 相似度检测基于字符串算法，缺少 Embedding 语义相似度

### Gemini 分析摘要（前端视角）

- **记忆 UI 缺失**: 当前 GUI 无记忆管理界面，用户无法可视化操作记忆
- **搜索体验**: 搜索结果无代码预览、无高亮、无实时反馈
- **响应式不足**: 现有组件未充分适配小屏幕和分屏模式
- **状态分散**: MCP 工具运行状态、索引进度等信息分散在各处，缺少统一面板
- **设计系统**: 已有 Naive UI + UnoCSS 基础，新增组件应复用现有设计系统

### SESSION_ID（供后续使用）
- CODEX_SESSION (v2): `019c71b9-59dc-79c1-bb37-23744264bd28`
- GEMINI_SESSION (v2): `b53b93da-6e08-46c9-9b33-9bad43bd403d`
- GEMINI_SESSION (v3): `2ee80b46-0dbe-448d-86d1-76a75d4be386`

---

## 附录

### A. 工作流程记录

本研究遵循 Agent Teams 需求研究工作流（8 阶段）：

**v1（2026-02-18）**:
1. ✅ **Prompt 增强**: 使用 Claude 自增强（mcp______enhance 不可用）
2. ✅ **代码库评估**: 检索三术项目上下文（ace-tool）
3. ✅ **定义探索边界**: 划分 Codex/Gemini 探索范围
4. ⚠️ **多模型并行探索**: 基于文档完成分析（外部模型工具不可用）
5. ✅ **聚合约束集**: 整合硬约束/软约束/依赖/风险
6. ✅ **歧义消解**: 用户选择"暂不实施，仅记录"
7. ✅ **写入研究文件**: 输出到 `.doc/agent-teams/research/`
8. ⏳ **归档**: 调用 ji 存储关键信息（待执行）

**v2（2026-02-19）**:
1. ✅ **修复 codeagent-wrapper 调用问题**: 修正 renderer.md 和测试文件中的错误 CLI 语法
2. ✅ **审查其他调用点**: 确认 22 个相关文件无同类问题
3. ✅ **多模型并行探索**: Codex + Gemini 成功调用（dual_model_status=SUCCESS）
4. ✅ **聚合约束集**: 新增 HC-5~HC-8、SC-5~SC-12、DEP-3~DEP-6、R-4~R-6
5. ✅ **歧义消解**: 用户确认"接受约束集，继续写入研究文件"
6. ✅ **写入研究文件**: 合并 v1 + v2 内容
7. ⏳ **归档**: 调用 ji 存储关键信息（v3 统一归档）

**v3（2026-02-19 查漏补缺）**:
1. ✅ **Prompt 增强**: Claude 自增强（mcp______enhance 失败，降级执行）
2. ✅ **代码库评估**: 6 路并行 ace-tool 检索（安全/测试/配置/日志/跨平台/资源限制）
3. ✅ **多模型并行探索**: Codex 超时（>5min）→ Claude 自补充后端分析；Gemini 成功
4. ✅ **聚合约束集**: 新增 HC-9~HC-15、SC-13~SC-20、DEP-7~DEP-9、R-7~R-10
5. ✅ **歧义消解**: 用户确认"接受约束集，写入研究文件"
6. ✅ **写入研究文件**: 合并 v1 + v2 + v3 内容
7. ✅ **归档**: 调用 ji 存储关键信息（含 v1/v2 遗留归档）

### B. 环境限制说明

**v1**: 由于 codeagent-wrapper 路径不存在，无法调用 Codex/Gemini 进行并行探索。采用降级方案：基于 5 个项目的完整文档进行分析。

**v2**: 修复 codeagent-wrapper CLI 语法后，成功调用 Codex + Gemini 进行并行探索。修复内容：
- `renderer.md` 中的示例使用了不存在的 `--role`、`--task`、`--workdir` 参数，已更正为 stdin 管道模式
- `command-renderer.spec.cjs` 中 3 个测试用例使用了错误的 CLI 语法，已更正

**v3**: Codex 后端超时（>5 分钟未返回），采用降级方案：基于 ace-tool 已检索的源码数据进行 Claude 自分析，补充后端视角。Gemini 前端成功调用（SESSION_ID: `2ee80b46-0dbe-448d-86d1-76a75d4be386`）。`mcp______enhance` 增强工具失败（返回"未能从响应中提取增强结果"），降级为 Claude 自增强。

---

**研究完成时间**: 2026-02-18 20:15（v1）→ 2026-02-19（v2）→ 2026-02-19（v3）
**下一步**: 调用 ji 工具归档关键设计决策
