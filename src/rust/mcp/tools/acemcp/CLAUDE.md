# 搜索工具 (acemcp)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **acemcp**

---

## 模块职责

代码库索引与混合检索工具 (sou)，提供 BM25 关键词检索 + 向量语义检索 + RRF 融合排序的混合检索能力。支持本地向量索引、多提供者嵌入、三层缓存、增量索引、智能等待、文件监听和多编码。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `sou`
- **标识符**: `mcp______sou`
- **状态**: 默认关闭（可选启用）

### 核心结构
```rust
pub struct AcemcpTool;

impl AcemcpTool {
    pub async fn search_context(request: AcemcpRequest) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "sou",
  "arguments": {
    "project_root_path": "/path/to/project",
    "query": "用户认证逻辑"
  }
}
```

---

## 核心功能

### 1. 混合检索引擎 (`hybrid_search.rs`) - P1 新增

BM25 关键词检索 + 向量语义检索，通过 RRF（Reciprocal Rank Fusion）融合排名。

```rust
pub struct HybridResult {
    pub path: String,
    pub rrf_score: f32,         // RRF 融合分数
    pub bm25_score: Option<f32>,    // BM25 分数（调试用）
    pub vector_score: Option<f32>,  // 向量相似度分数（调试用）
}
```

**BM25 参数**：
- `k1 = 1.5` - 词频饱和系数
- `b = 0.75` - 文档长度归一化系数

**RRF 参数**：
- `k = 60` - 平滑常数

**融合流程**：
```
查询 → 并行执行 BM25 + 向量检索 → RRF 融合排名 → 返回 Top-N
```

### 2. 本地向量索引 (`local_index.rs`) - P1 新增

支持增量更新、并发安全、原子写入的本地向量索引。

```rust
pub struct LocalIndexManager {
    index: Arc<RwLock<IndexFile>>,
    project_root: PathBuf,
    index_dir: PathBuf,
}

pub struct IndexEntry {
    pub path: String,           // 文件路径（相对于项目根）
    pub content_hash: String,   // 内容 hash（增量更新判断）
    pub embedding: Vec<f32>,    // 嵌入向量
    pub updated_at: u64,        // 最后更新时间戳
}
```

**约束**：
- **磁盘空间限制**: 500MB（`MAX_INDEX_SIZE_BYTES`）
- **索引目录**: `.sanshu-index/`
- **并发安全**: `Arc<RwLock>` 保护

### 3. 多提供者嵌入客户端 (`embedding_client.rs`) - P1 新增

```rust
pub enum EmbeddingProvider {
    Jina,          // 批量 100
    SiliconFlow,   // 批量 50
    Cloudflare,    // 批量 50
    Nomic,         // 批量 100
    Cohere,        // 批量 96
    Ollama,        // 批量 10
}

pub struct EmbeddingClient {
    provider: EmbeddingProvider,
    base_url: String,
    api_key: Option<String>,
    model: String,
}
```

**功能**：
- 批量嵌入（根据提供者自动调整批量大小）
- 自动超时和重试
- 向量归一化

### 4. 三层缓存 (`cache.rs`) - P1 新增

```rust
/// 内存级缓存（LRU + TTL）
pub struct SearchCache<T> {
    entries: HashMap<String, CacheEntry<T>>,
    max_entries: usize,  // 默认 100
    ttl: Duration,       // 默认 5 分钟
}
```

**三层架构**：
| 层级 | 存储 | 容量 | TTL | 说明 |
|------|------|------|-----|------|
| L1 | 内存 LRU | 100 条 | 5 分钟 | `SearchCache` 泛型实现 |
| L2 | 磁盘 | 无限制 | 24 小时 | `.sanshu-index/cache/`，SHA256 key |
| L3 | API | - | - | 回源到远程 API |

### 5. 文件监听 (`watcher.rs`)

```rust
pub struct WatcherManager {
    watchers: Arc<Mutex<HashMap<String, WatcherHandle>>>,
}
```

- **防抖延迟**: 2 秒
- **触发条件**: 文件创建、修改、删除
- **忽略规则**: 遵循 `.gitignore`

### 6. 智能等待

```
Missing/Idle/Failed → 启动后台索引
Indexing → 智能等待（1-5 秒随机）
```

### 7. 多编码支持

UTF-8（优先）-> GBK（中文）-> Windows-1252（西欧）

---

## 数据流程

### 混合检索流程（P1 升级）
```
AI 请求
    → 检查三层缓存
    → [L1 命中] 返回内存缓存
    → [L2 命中] 返回磁盘缓存，提升到 L1
    → [未命中] 并行执行：
        |-- BM25 关键词检索（本地倒排索引）
        +-- 向量语义检索（本地嵌入索引）
    → RRF 融合排名
    → 存入 L1 + L2 缓存
    → 返回结果
```

### 索引流程
```
文件变更 → 防抖延迟(2s) → 收集变更文件
    → 计算内容 hash（增量判断）
    → 调用 EmbeddingClient 生成向量
    → 更新 LocalIndexManager
    → 写入 .sanshu-index/
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = [
  "stream",
  "json",
  "socks"
] }
tokio = { version = "1.0", features = [
  "fs",
  "process",
  "sync",
  "time"
] }
ignore = "0.4"
notify = "6.0"
notify-debouncer-full = "0.3"
encoding_rs = "0.8"
globset = "0.4"
ring = "0.17" # SHA256 hash（缓存键）
```

### 配置字段
```rust
pub base_url: Option<String>,          // API 基础 URL
pub token: Option<String>,             // API Token
pub smart_wait_range: Option<(u64, u64)>, // 智能等待范围（秒）
pub proxy: Option<ProxyConfig>,        // 代理配置
pub embedding_provider: Option<String>,// 嵌入提供者
pub embedding_api_key: Option<String>, // 嵌入 API Key
pub embedding_model: Option<String>,   // 嵌入模型
```

---

## 常见问题 (FAQ)

### Q: 混合检索与纯 API 检索有什么区别？
A: 混合检索在本地执行 BM25 + 向量检索，不依赖远程 API，延迟更低且可离线使用

### Q: 如何配置嵌入提供者？
A: 在配置中设置 `embedding_provider`（如 "jina"）和 `embedding_api_key`

### Q: 索引占用多少空间？
A: 最大 500MB（`MAX_INDEX_SIZE_BYTES`），超出时需手动清理

### Q: 缓存如何工作？
A: 三层缓存（内存 5min -> 磁盘 24h -> API），逐级回源

---

## 相关文件清单

### 核心文件
- `mcp.rs` - MCP 工具实现
- `hybrid_search.rs` - P1 混合检索引擎（BM25 + 向量 + RRF）
- `local_index.rs` - P1 本地向量索引管理
- `embedding_client.rs` - P1 多提供者嵌入客户端
- `cache.rs` - P1 三层缓存（内存 LRU + 磁盘 + API）
- `watcher.rs` - 文件监听管理器
- `commands.rs` - Tauri 命令
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 数据目录
- `.sanshu-index/` - 本地向量索引
- `.sanshu-index/cache/` - 磁盘缓存

---

**最后更新**: 2026-02-19
