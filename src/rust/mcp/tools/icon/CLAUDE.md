# 图标工坊 (icon)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **icon**

---

## 模块职责

图标工坊 (tu)，提供 Iconfont 图标搜索、预览、批量下载和 SVG 转 PNG 功能。支持多种图标风格和智能缓存机制。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `tu`
- **标识符**: `mcp______tu`
- **状态**: 内置工具（始终可用）

### 核心结构
```rust
pub struct IconTool;

impl IconTool {
    pub async fn search(request: IconSearchRequest) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "tu",
  "arguments": {
    "query": "用户",
    "style": "线性",
    "page": 1,
    "page_size": 50
  }
}
```

### 请求参数
```rust
pub struct IconSearchRequest {
    /// 搜索关键词
    pub query: String,

    /// 图标风格（可选）
    pub style: Option<String>,

    /// 填充方式（可选）
    pub fills: Option<String>,

    /// 排序方式（可选）
    pub sort_type: Option<String>,

    /// 页码（可选，默认 1）
    pub page: Option<u32>,

    /// 每页数量（可选，默认 50）
    pub page_size: Option<u32>,

    /// 是否仅搜索收藏（可选）
    pub from_collection: Option<bool>,
}
```

### 响应格式
```rust
pub struct IconSearchResult {
    pub icons: Vec<IconItem>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

pub struct IconItem {
    pub id: u32,
    pub name: String,
    pub show_svg: String,      // SVG 内容
    pub unicode: String,        // Unicode 编码
    pub font_class: String,     // CSS 类名
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = ["json"] }
once_cell = "1.19"
base64 = "0.21"
resvg = "0.38"      # SVG 转 PNG
usvg = "0.38"       # SVG 解析
tiny-skia = "0.11"  # 图像渲染
```

### 缓存配置
```rust
/// 缓存过期时间（默认 30 分钟）
const DEFAULT_CACHE_EXPIRY_SECS: u64 = 30 * 60;

/// HTTP 请求超时时间
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// 最大重试次数
const MAX_RETRIES: usize = 3;
```

---

## 核心功能

### 1. 图标搜索 (`api.rs`)

#### 搜索流程
```rust
pub async fn search_icons(request: &IconSearchRequest) -> Result<IconSearchResult> {
    // 1. 生成缓存键
    let cache_key = generate_cache_key(request);

    // 2. 检查缓存
    if let Some(cached) = get_from_cache(&cache_key) {
        return Ok(cached);
    }

    // 3. 调用 Iconfont API
    let response = call_iconfont_api(request).await?;

    // 4. 解析响应
    let result = parse_api_response(response)?;

    // 5. 写入缓存
    put_to_cache(&cache_key, &result);

    Ok(result)
}
```

#### API 端点
```rust
const ICONFONT_SEARCH_API: &str = "https://www.iconfont.cn/api/icon/search.json";
```

#### 请求参数
```rust
let params = vec![
    ("q", request.query.as_str()),
    ("t", request.style.as_deref().unwrap_or("all")),
    ("fills", request.fills.as_deref().unwrap_or("all")),
    ("sortType", request.sort_type.as_deref().unwrap_or("relate")),
    ("page", &request.page.unwrap_or(1).to_string()),
    ("pageSize", &request.page_size.unwrap_or(50).to_string()),
    ("fromCollection", &request.from_collection.unwrap_or(false).to_string()),
];
```

### 2. 缓存管理

#### 缓存结构
```rust
struct CacheEntry {
    result: IconSearchResult,
    created_at: Instant,
}

static SEARCH_CACHE: Lazy<RwLock<HashMap<String, CacheEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
```

#### 缓存键生成
```rust
fn generate_cache_key(request: &IconSearchRequest) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        request.query,
        request.style.as_deref().unwrap_or("all"),
        request.fills.as_deref().unwrap_or("all"),
        request.sort_type.as_deref().unwrap_or("relate"),
        request.page.unwrap_or(1),
        request.page_size.unwrap_or(50),
        request.from_collection.unwrap_or(false),
    )
}
```

#### 缓存过期检查
```rust
fn get_from_cache(cache_key: &str) -> Option<IconSearchResult> {
    let cache = SEARCH_CACHE.read().ok()?;
    let entry = cache.get(cache_key)?;

    // 检查是否过期
    let expiry_secs = *CACHE_EXPIRY_SECS.read().ok()?;
    if entry.created_at.elapsed().as_secs() > expiry_secs {
        return None;
    }

    Some(entry.result.clone())
}
```

### 3. SVG 转 PNG

#### 转换流程
```rust
pub fn svg_to_png(svg_content: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    // 1. 解析 SVG
    let tree = usvg::Tree::from_str(svg_content, &usvg::Options::default())?;

    // 2. 创建画布
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow!("无法创建画布"))?;

    // 3. 渲染 SVG
    resvg::render(
        &tree,
        usvg::FitTo::Size(width, height),
        tiny_skia::Transform::default(),
        pixmap.as_mut(),
    )?;

    // 4. 编码为 PNG
    let png_data = pixmap.encode_png()?;

    Ok(png_data)
}
```

#### 批量转换
```rust
pub async fn batch_convert_icons(
    icons: &[IconItem],
    width: u32,
    height: u32,
    output_dir: &Path
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for icon in icons {
        // 1. SVG 转 PNG
        let png_data = svg_to_png(&icon.show_svg, width, height)?;

        // 2. 保存文件
        let filename = format!("{}.png", sanitize_filename(&icon.name));
        let path = output_dir.join(filename);
        std::fs::write(&path, png_data)?;

        paths.push(path);
    }

    Ok(paths)
}
```

### 4. 图标下载

#### 下载流程
```rust
pub async fn download_icons(
    icons: &[IconItem],
    save_path: &str,
    format: IconFormat
) -> Result<Vec<String>> {
    let output_dir = Path::new(save_path);
    std::fs::create_dir_all(output_dir)?;

    match format {
        IconFormat::Svg => download_svg_icons(icons, output_dir).await,
        IconFormat::Png { width, height } => {
            batch_convert_icons(icons, width, height, output_dir).await
        }
    }
}
```

#### SVG 下载
```rust
async fn download_svg_icons(
    icons: &[IconItem],
    output_dir: &Path
) -> Result<Vec<String>> {
    let mut paths = Vec::new();

    for icon in icons {
        let filename = format!("{}.svg", sanitize_filename(&icon.name));
        let path = output_dir.join(filename);
        std::fs::write(&path, &icon.show_svg)?;
        paths.push(path.to_string_lossy().to_string());
    }

    Ok(paths)
}
```

---

## 支持的图标风格

### 风格列表
- **线性** (line)
- **面性** (fill)
- **双色** (duotone)
- **多色** (multicolor)
- **手绘** (handdrawn)
- **扁平** (flat)
- **渐变** (gradient)

### 填充方式
- **单色** (single)
- **多色** (multi)
- **全部** (all)

### 排序方式
- **相关性** (relate)
- **最新** (newest)
- **最热** (hottest)

---

## 数据流程

### 搜索流程
```
AI 请求 → 生成缓存键 → 检查缓存 → 调用 API → 解析响应 → 写入缓存 → 返回结果
```

### 下载流程
```
选择图标 → 创建输出目录 → SVG 转 PNG（可选） → 保存文件 → 返回路径列表
```

---

## 常见问题 (FAQ)

### Q: 如何调整缓存过期时间？
A: 在配置文件中设置 `icon_cache_expiry_minutes`

### Q: 支持哪些图标格式？
A: SVG（原始格式）和 PNG（转换格式）

### Q: 如何批量下载图标？
A: 在前端选择多个图标后点击"批量下载"

### Q: PNG 转换的默认尺寸是多少？
A: 默认 512x512，可自定义

### Q: 搜索结果为空怎么办？
A: 尝试使用更通用的关键词或切换图标风格

---

## 相关文件清单

### 核心文件
- `api.rs` - Iconfont API 封装
- `mcp.rs` - MCP 工具实现
- `commands.rs` - Tauri 命令
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 前端组件
- `IconWorkshop.vue` - 图标工坊主组件
- `IconResultsPanel.vue` - 搜索结果面板
- `IconCard.vue` - 图标卡片
- `IconSaveModal.vue` - 保存对话框

---

## 使用示例

### 基础搜索
```rust
let request = IconSearchRequest {
    query: "用户".to_string(),
    style: Some("线性".to_string()),
    page: Some(1),
    page_size: Some(50),
    ..Default::default()
};

let result = IconTool::search(request).await?;
```

### 批量下载 SVG
```rust
let icons = vec![icon1, icon2, icon3];
let paths = download_icons(
    &icons,
    "./icons",
    IconFormat::Svg
).await?;
```

### 批量下载 PNG
```rust
let icons = vec![icon1, icon2, icon3];
let paths = download_icons(
    &icons,
    "./icons",
    IconFormat::Png { width: 512, height: 512 }
).await?;
```

---

**最后更新**: 2026-02-18
