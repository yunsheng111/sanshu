# OpenSpec 提案：记忆管理系统全面优化

> 日期：2026-02-19
> 约束集：`.doc/spec/constraints/20260219-memory-optimization-constraints.md`
> 研究文档：`.doc/workflow/research/20260219-memory-optimization-analysis.md`
> 状态：约束研究完成，待零决策规划

---

## 1. 提案概述

### 1.1 问题陈述

三术的记忆管理系统（ji 工具）当前基于 JSON 文件 + 线性扫描 + 4 种固定分类，在以下场景面临瓶颈：

| 问题 | 当前状态 | 目标状态 |
|------|----------|----------|
| 实例重复创建 | 每次 MCP 调用创建新 MemoryManager | 全局池缓存 + Weak 引用回收 |
| 分类维度不足 | 4 种固定枚举 | URI 路径 + 自由标签混合方案 |
| 搜索效率低 | 模糊匹配（线性扫描） | FTS5 索引 + 意图识别 |
| 写入质量无控制 | 去重后置（批量整理） | Write Guard 前置拦截 |
| 无生命周期管理 | 硬上限 1000 条 | Vitality Decay 活力衰减 |
| 前端信息过载 | 扁平列表 | 树形浏览 + 渐进式披露 |

### 1.2 研究来源

本提案整合 4 个参考源的最佳实践：

| 来源 | 定位 | 核心贡献 |
|------|------|----------|
| Codex 分析 | 后端架构视角 | MemoryManagerRegistry、FTS5 Sidecar、StorageEngine 抽象 |
| Gemini 分析 | 前端 UX 视角 | 树形面板、标签云、批量操作、搜索语法、850px 布局策略 |
| Memory-Palace | AI 记忆操作系统 | Write Guard、Vitality Decay、URI 路径、意图识别 |
| claude-mem | 会话持久化插件 | 3 层渐进式披露、AI 压缩、会话自动捕获、Token 效率 |

---

## 2. 修订后的数据模型

### 2.1 MemoryEntry v2.2

```rust
/// 记忆条目结构（v2.2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // === v2.1 已有字段 ===
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub content_normalized: String,
    pub category: MemoryCategory,          // 保留（向后兼容）
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub snapshots: Vec<MemorySnapshot>,

    // === v2.2 新增字段 ===

    /// URI 路径，如 "core://architecture/backend"、"project://sanshu/mcp"
    /// 用于树形浏览和层级组织
    #[serde(default)]
    pub uri_path: Option<String>,

    /// 域名，从 uri_path 中提取的顶级域
    /// 如 "core", "project", "session", "legacy"
    #[serde(default)]
    pub domain: Option<String>,

    /// 自由标签，支持横向筛选
    /// 如 ["rust", "性能", "P0", "架构"]
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// 活力值 (0.0-3.0)，用于生命周期管理
    /// 默认 1.5，每次访问 +0.5，指数衰减（半衰期 30 天）
    #[serde(default = "default_vitality_score")]
    pub vitality_score: Option<f64>,

    /// 最后访问时间，用于活力衰减计算
    #[serde(default)]
    pub last_accessed_at: Option<DateTime<Utc>>,

    /// 自动生成的摘要（长记忆 > 500 字符时生成）
    #[serde(default)]
    pub summary: Option<String>,
}

fn default_vitality_score() -> Option<f64> {
    Some(1.5)
}
```

### 2.2 MemoryConfig v2.2 扩展

```rust
/// 记忆配置（v2.2 扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    // === v2.1 已有 ===
    pub similarity_threshold: f64,        // 去重阈值，默认 0.70
    pub dedup_on_startup: bool,           // 启动去重，默认 true
    pub enable_dedup: bool,               // 启用去重，默认 true
    pub max_entry_bytes: usize,           // 单条上限，默认 10240
    pub max_entries: usize,               // 总数上限，默认 1000

    // === v2.2 新增 ===

    /// Write Guard 语义匹配阈值（>= 此值自动 NOOP）
    #[serde(default = "default_wg_semantic")]
    pub write_guard_semantic_threshold: f64,   // 默认 0.80

    /// Write Guard 更新匹配阈值（此值到 semantic 之间自动 UPDATE）
    #[serde(default = "default_wg_update")]
    pub write_guard_update_threshold: f64,     // 默认 0.60

    /// 活力衰减半衰期（天）
    #[serde(default = "default_decay_half_life")]
    pub vitality_decay_half_life_days: u32,    // 默认 30

    /// 活力清理阈值
    #[serde(default = "default_cleanup_threshold")]
    pub vitality_cleanup_threshold: f64,       // 默认 0.35

    /// 不活跃天数阈值
    #[serde(default = "default_inactive_days")]
    pub vitality_cleanup_inactive_days: u32,   // 默认 14

    /// 每次访问提升的活力值
    #[serde(default = "default_access_boost")]
    pub vitality_access_boost: f64,            // 默认 0.5

    /// 最大活力值
    #[serde(default = "default_max_vitality")]
    pub vitality_max_score: f64,               // 默认 3.0

    /// 摘要生成的内容长度阈值（字符数）
    #[serde(default = "default_summary_threshold")]
    pub summary_length_threshold: usize,       // 默认 500
}
```

### 2.3 MemoryStore v2.2

```rust
pub struct MemoryStore {
    pub version: String,              // "2.2"
    pub project_path: String,
    pub entries: Vec<MemoryEntry>,
    pub last_dedup_at: DateTime<Utc>,
    pub config: MemoryConfig,

    // === v2.2 新增 ===
    /// 域注册表：记录所有使用中的域及其描述
    #[serde(default)]
    pub domains: Option<HashMap<String, DomainInfo>>,
}

pub struct DomainInfo {
    pub name: String,
    pub description: Option<String>,
    pub entry_count: usize,
}
```

---

## 3. 修订后的后端架构

### 3.1 模块结构变更

```
src/rust/mcp/tools/memory/
  |-- mod.rs                    # 模块导出（更新）
  |-- types.rs                  # 数据类型（v2.2 升级）
  |-- manager.rs                # 核心管理器（集成 Write Guard + Vitality Decay）
  |-- registry.rs               # [新增] MemoryManagerRegistry 全局池
  |-- write_guard.rs            # [新增] Write Guard 三级写入拦截
  |-- vitality.rs               # [新增] Vitality Decay 活力衰减引擎
  |-- uri_path.rs               # [新增] URI 路径解析和验证
  |-- summary.rs                # [新增] 摘要自动生成（复用 enhance 降级链）
  |-- fts_index.rs              # [新增 P2] FTS5 Sidecar 索引
  |-- observation_store.rs      # [新增 P3] 会话工具观察存储
  |-- similarity.rs             # 相似度算法（不变）
  |-- dedup.rs                  # 去重检测器（不变）
  |-- migration.rs              # 格式迁移（更新：支持 v2.1 -> v2.2）
  |-- mcp.rs                    # MCP 工具入口（更新：新增操作）
```

### 3.2 MemoryManagerRegistry（P0）

```
全局单例 REGISTRY: Lazy<MemoryManagerRegistry>
  |
  +-- HashMap<CanonicalPath, WeakEntry>
  |     |-- WeakEntry { weak: Weak<RwLock<MemoryManager>>, last_access: Instant }
  |     +-- TTL: 30 分钟
  |
  +-- get_or_create(project_path) -> SharedMemoryManager
  |     |-- canonical = normalize_project_path(project_path)
  |     |-- if cached && Weak::upgrade() -> 返回缓存
  |     +-- else -> SharedMemoryManager::new() -> 缓存 -> 返回
  |
  +-- cleanup_expired() -- 每 5 分钟清理过期项
  +-- pool_size_limit: 16
```

### 3.3 Write Guard 流程（P0）

```
add_memory(content, category)
  |
  +-- [1] TextSimilarity::calculate_enhanced(content, existing)
  |     |-- similarity >= 0.8 -> NOOP（静默拒绝，日志记录）
  |     |-- 0.6 <= similarity < 0.8 -> UPDATE（自动合并到匹配条目）
  |     +-- similarity < 0.6 -> ADD（正常新增）
  |
  +-- [2] 如果 ADD -> 执行现有 add_memory 逻辑
  +-- [3] 如果 UPDATE -> 调用 update_memory(matched_id, merged_content, false)
  +-- [4] 返回 WriteGuardResult { action, similarity, matched_id }
```

### 3.4 Vitality Decay 引擎（P1）

```
衰减公式：V(t) = V0 * 2^(-t/half_life)
  V0 = 上次访问时的活力值
  t = 当前时间 - last_accessed_at（天）
  half_life = 30（天，可配置）

访问提升：V_new = min(V_current + boost, max_vitality)
  boost = 0.5（可配置）
  max_vitality = 3.0（可配置）

清理候选条件：
  vitality_score < cleanup_threshold (0.35)
  AND last_accessed_at < now - inactive_days (14 天)
  AND category != Rule（Rule 分类永不自动清理）
```

---

## 4. 修订后的前端架构

### 4.1 组件结构

```
MemoryManager.vue（主容器 -- 替代 MemoryConfig.vue）
  |
  +-- MemoryLayout.vue（布局容器 -- NLayout）
  |   |
  |   +-- DomainTree.vue（左侧域/路径树 -- NLayoutSider, collapsible）
  |   |   +-- NTree（数据源：domains + uri_paths）
  |   |   +-- 拖拽分类（将记忆拖入域节点）
  |   |   +-- 右键菜单（新建域、重命名、删除空域）
  |   |
  |   +-- MemoryWorkspace.vue（中间工作区）
  |       |
  |       +-- SearchBar.vue（搜索栏 -- 支持前缀语法）
  |       |   +-- NInput（带 @domain #tag 高亮）
  |       |   +-- 300ms debounce 实时搜索
  |       |
  |       +-- TagFilter.vue（标签筛选条 -- NTag + NSpace）
  |       |   +-- 当前域/搜索结果的标签云
  |       |   +-- 点击标签切换筛选
  |       |
  |       +-- MemoryCardList.vue（记忆卡片列表 -- NDataTable virtual）
  |       |   +-- 渐进式披露三态：
  |       |   |   L1: collapsed（标题 + 分类 + VitalityBadge + 时间）
  |       |   |   L2: expanded（+ 内容预览 100 字 + 标签）
  |       |   |   L3: detail（+ 完整内容 + 版本历史 + Diff）
  |       |   +-- NCheckbox 多选支持
  |       |   +-- 虚拟滚动（>100 条时启用）
  |       |
  |       +-- VitalityBadge.vue（活力值徽章 -- NProgress circular + NTooltip）
  |       |   +-- 颜色编码：绿(>2.0) / 黄(1.0-2.0) / 红(<1.0)
  |       |   +-- Tooltip 显示衰减趋势
  |       |
  |       +-- BatchActionBar.vue（批量操作条 -- NAffix 底部固定）
  |           +-- 批量删除 / 重新分类 / 导出 / 刷新活力值
  |
  +-- MemorySearch.vue（搜索面板 -- 增强版，Tab 切换）
  +-- MemoryConfig.vue（配置面板 -- 精简版，含 Write Guard + Vitality 参数）
```

### 4.2 新增 Composables

```typescript
// composables/useMemoryTree.ts
// 将 MemoryEntry[] 转换为域树结构
function useMemoryTree(entries: Ref<MemoryEntry[]>): {
  treeData: ComputedRef<TreeOption[]>
  expandedKeys: Ref<string[]>
  selectedDomain: Ref<string | null>
  filterByDomain: (domain: string) => MemoryEntry[]
}

// composables/useVitalityDecay.ts
// 活力值计算和视觉化
function useVitalityDecay(): {
  calculateCurrentVitality: (entry: MemoryEntry) => number
  vitalityColor: (score: number) => string
  isCleanupCandidate: (entry: MemoryEntry) => boolean
}

// composables/useProgressiveDisclosure.ts
// 渐进式披露状态管理
function useProgressiveDisclosure(): {
  disclosureLevel: Ref<Map<string, 'collapsed' | 'expanded' | 'detail'>>
  toggle: (id: string) => void
  expandAll: () => void
  collapseAll: () => void
}
```

---

## 5. 修订后的 MCP 操作接口

### 5.1 更新操作列表

| 操作 | 说明 | 新增/变更 | 优先级 |
|------|------|-----------|--------|
| `记忆` | 添加新记忆（集成 Write Guard） | **变更**：返回 WriteGuardResult | P0 |
| `回忆` | 查询记忆（压缩格式） | **变更**：支持 verbose 参数 | P3 |
| `整理` | 去重检测 | 不变 | - |
| `列表` | 列出记忆（分页） | **变更**：支持 page/page_size/summary_only | P3 |
| `预览相似` | 相似度检测 | 不变 | - |
| `配置` | 获取/更新配置 | **变更**：新增 Write Guard + Vitality 参数 | P0 |
| `删除` | 删除记忆 | 不变 | - |
| `更新` | 更新记忆 | 不变 | - |
| `分类` | 设置/更新 URI 路径和标签 | **新增** | P1 |
| `域列表` | 获取所有域及其统计 | **新增** | P1 |
| `清理候选` | 获取低活力清理候选列表 | **新增** | P1 |
| `执行清理` | 确认并执行清理 | **新增** | P1 |
| `获取快照` | 获取指定记忆的版本历史 | **新增** | P2 |

---

## 6. 优先级实施路线图

### 第 1 阶段（第 1-2 周）：P0 质量基础

**后端**：
- [ ] 实现 Write Guard 写入守卫（`write_guard.rs`）
  - 复用 TextSimilarity::calculate_enhanced()
  - 三级判定：NOOP / UPDATE / ADD
  - 可配置阈值
- [ ] 实现 MemoryManagerRegistry 全局池（`registry.rs`）
  - Lazy<Arc<MemoryManagerRegistry>>
  - Weak 引用 + TTL 回收 + 池大小上限
- [ ] 升级 schema_version 到 2.2
  - types.rs 新增字段（全部 Option + serde default）
  - migration.rs 新增 v2.1 -> v2.2 迁移路径
- [ ] 更新 mcp.rs 调用入口
  - `记忆` 操作集成 Write Guard
  - `配置` 操作暴露新参数

**前端**：
- [ ] 实现 300ms debounce 实时搜索（现有 MemorySearch.vue 增强）

**预估工时**：20h

### 第 2 阶段（第 3-4 周）：P1 组织增强

**后端**：
- [ ] 实现 Vitality Decay 引擎（`vitality.rs`）
  - 指数衰减公式
  - 访问提升逻辑
  - 清理候选标记
- [ ] 实现 URI 路径解析和验证（`uri_path.rs`）
  - domain://path 格式解析
  - 域注册表管理
- [ ] 新增 MCP 操作：`分类`、`域列表`、`清理候选`、`执行清理`

**前端**：
- [ ] 重构为 MemoryManager.vue 主容器
- [ ] 实现 DomainTree.vue（左侧域树）
- [ ] 实现 MemoryWorkspace.vue（中间工作区）
- [ ] 实现 TagFilter.vue（标签筛选条）
- [ ] 实现 VitalityBadge.vue（活力值徽章）
- [ ] 实现渐进式披露三态交互
- [ ] 实现搜索前缀语法（@domain #tag）

**预估工时**：40h

### 第 3 阶段（第 5-6 周）：P2 检索升级

**后端**：
- [ ] 引入 rusqlite 依赖
- [ ] 实现 FTS5 Sidecar 索引（`fts_index.rs`）
  - 双写一致性（JSON 先 → FTS5 后）
  - 定时校验任务
- [ ] 实现意图识别检索（关键词评分法）
- [ ] 实现记忆摘要自动生成（`summary.rs`）
  - 复用 enhance 降级链
- [ ] 新增 MCP 操作：`获取快照`

**前端**：
- [ ] 实现 Snapshot Diff 视图
- [ ] 实现 BatchActionBar.vue（批量操作条）
- [ ] 虚拟滚动优化（>100 条）

**预估工时**：30h

### 第 4 阶段（第 7-8 周）：P3 体验升级

**后端**：
- [ ] 实现会话工具观察自动捕获（`observation_store.rs`）
  - call_tool 后置钩子
  - tokio channel 异步写入
  - 可配置跳过列表
- [ ] Token 效率优化
  - `回忆` 操作 verbose 参数
  - `列表` 操作分页 + summary_only

**前端**：
- [ ] 活力衰减趋势图（VitalityBadge 增强）
- [ ] ARIA 标注 + 键盘导航
- [ ] 骨架屏加载状态

**预估工时**：26h

### 第 5 阶段（按需）：P4 远期探索

- [ ] SQLite 全面迁移（替换 JSON 为 SQLite 主存储）
  - 触发条件：记忆 > 2w 条 或 P95 查询 > 200ms
- [ ] 知识图谱可视化
  - 触发条件：记忆 > 200 条 且用户需求明确

---

## 7. 约束冲突说明

### 7.1 HC-16 vs P4 SQLite 迁移

- **冲突**：HC-16 规定 P0-P2 阶段 JSON 为真实数据源，P4 计划 SQLite 替换 JSON
- **裁决建议**：P4 启动前需重新评估 HC-16，由用户决定是否解除

### 7.2 SC-19 摘要生成 vs enhance 工具可用性

- **冲突**：摘要生成依赖 enhance 降级链，如果 Ollama 和云端 API 均不可用，规则引擎只能生成低质量摘要
- **裁决建议**：规则引擎摘要标记为"自动截取"，前端显示不同样式区分

### 7.3 HC-11 Write Guard vs 现有去重检测

- **冲突**：Write Guard（前置）和现有 dedup（后置）功能重叠
- **裁决建议**：Write Guard 启用后，dedup_on_startup 默认改为 false（仍可手动整理）。两者阈值独立配置

---

## 8. 性能预估

| 规模 | JSON 线性扫描（当前） | + FTS5 Sidecar（P2 后） | + SQLite 主存储（P4 后） |
|------|----------------------|------------------------|------------------------|
| 1k 条 | 20-80ms | 10-30ms | 5-15ms |
| 5k 条 | 100-400ms | 15-50ms | 8-25ms |
| 1w 条 | 200-800ms | 20-80ms | 10-40ms |
| 5w 条 | 不可接受 | 30-150ms | 15-80ms |

---

## 9. 验收标准

### P0 验收
- [ ] Write Guard 正确拦截 >= 0.8 相似度的重复记忆
- [ ] Write Guard 正确合并 0.6-0.8 相似度的记忆
- [ ] MemoryManagerRegistry 第二次访问同项目无冷启动延迟
- [ ] v2.2 schema 升级不破坏现有 v2.1 数据
- [ ] 所有现有 cargo test 通过

### P1 验收
- [ ] URI 路径正确解析且前端域树渲染正确
- [ ] 活力衰减在 30 天后正确将初始值 1.5 衰减至 ~0.75
- [ ] 前端渐进式披露三态交互流畅
- [ ] 搜索前缀语法正确过滤（@domain、#tag）
- [ ] 850px 窗口宽度下布局不溢出

### P2 验收
- [ ] FTS5 搜索结果与 JSON 全量遍历一致（校验通过）
- [ ] 长记忆（>500 字符）自动生成摘要
- [ ] Snapshot Diff 视图正确展示版本差异
- [ ] 批量操作（>10 条）执行成功

### P3 验收
- [ ] 会话工具观察自动捕获不阻塞 MCP 主流程
- [ ] 回忆操作的 verbose=false 模式 token 消耗减少 >50%
- [ ] ARIA 标注通过 axe-core 基础检查

---

## 10. 参考来源索引

| 来源 | 链接 | 贡献领域 |
|------|------|----------|
| Codex 分析 | SESSION: 019c75e4-93db-75d0-81da-2d80630282a8 | 后端架构、FTS5、Registry |
| Gemini 分析 | SESSION: e60f4a6b-1c88-4691-aa55-1b340335ec11 | 前端 UX、布局策略、搜索语法 |
| Memory-Palace | https://github.com/AGI-is-going-to-arrive/Memory-Palace | Write Guard、Vitality Decay、URI 路径 |
| claude-mem | https://github.com/thedotmack/claude-mem | 渐进式披露、AI 压缩、会话捕获 |
| claude-mem 文档 | https://docs.claude-mem.ai | 架构细节、Hook 生命周期、数据库 Schema |
