# Context7 文档查询工具 (context7)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **context7**

---

## 模块职责

Context7 文档查询工具，提供框架和库的官方文档检索能力，支持 React、Vue、Tailwind CSS 等主流技术栈，具备智能降级搜索和分页浏览功能。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `context7`
- **标识符**: `mcp______context7`
- **状态**: 默认启用

### 核心结构
```rust
pub struct Context7Tool;

impl Context7Tool {
    pub async fn query(request: Context7Request) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "context7",
  "arguments": {
    "query": "React hooks useEffect",
    "page": 1
  }
}
```

### 请求参数
```rust
pub struct Context7Request {
    /// 查询关键词
    pub query: String,

    /// 页码（可选，默认 1）
    pub page: Option<u32>,
}
```

### 响应格式
```json
{
  "results": [
    {
      "title": "useEffect - React Hooks",
      "url": "https://react.dev/reference/react/useEffect",
      "snippet": "useEffect is a React Hook that lets you synchronize a component with an external system.",
      "source": "React Official Docs"
    }
  ],
  "total": 42,
  "page": 1,
  "has_more": true
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
```

### 配置结构
```rust
pub struct Context7Config {
    /// API 基础 URL
    pub base_url: String,

    /// API Key（可选，提高速率限制）
    pub api_key: Option<String>,

    /// 每页结果数
    pub page_size: u32,
}
```

### 默认配置
- **base_url**: `https://api.context7.com`
- **page_size**: 10
- **api_key**: 从配置文件读取（可选）

---

## 核心功能

### 1. 文档查询

#### 查询流程
```rust
pub async fn query(request: Context7Request) -> Result<CallToolResult, McpError> {
    // 1. 加载配置
    let config = get_context7_config().await?;

    // 2. 构建请求
    let client = create_http_client()?;
    let mut url = format!("{}/api/search", config.base_url);

    // 3. 添加查询参数
    let params = vec![
        ("q", request.query.as_str()),
        ("page", &request.page.unwrap_or(1).to_string()),
        ("page_size", &config.page_size.to_string()),
    ];

    // 4. 添加 API Key（如果有）
    let mut headers = HeaderMap::new();
    if let Some(api_key) = &config.api_key {
        headers.insert("X-API-Key", HeaderValue::from_str(api_key)?);
    }

    // 5. 发送请求
    let response = client
        .get(&url)
        .query(&params)
        .headers(headers)
        .send()
        .await?;

    // 6. 解析响应
    let result: Context7Response = response.json().await?;

    // 7. 格式化输出
    Ok(CallToolResult::success(format_results(&result)))
}
```

### 2. 智能降级搜索

#### 降级策略
```rust
async fn search_with_fallback(query: &str, config: &Context7Config) -> Result<Context7Response> {
    // 1. 尝试精确搜索
    match search_exact(query, config).await {
        Ok(result) if !result.results.is_empty() => return Ok(result),
        _ => {}
    }

    // 2. 降级：模糊搜索
    match search_fuzzy(query, config).await {
        Ok(result) if !result.results.is_empty() => return Ok(result),
        _ => {}
    }

    // 3. 降级：关键词搜索
    let keywords = extract_keywords(query);
    search_keywords(&keywords, config).await
}
```

#### 关键词提取
```rust
fn extract_keywords(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter(|word| word.len() > 2)  // 过滤短词
        .map(|word| word.to_lowercase())
        .collect()
}
```

### 3. 分页浏览

#### 分页逻辑
```rust
pub struct Context7Response {
    pub results: Vec<SearchResult>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

impl Context7Response {
    pub fn has_next_page(&self) -> bool {
        self.page * self.page_size < self.total
    }

    pub fn has_prev_page(&self) -> bool {
        self.page > 1
    }

    pub fn total_pages(&self) -> u32 {
        (self.total + self.page_size - 1) / self.page_size
    }
}
```

### 4. 结果格式化

#### 格式化输出
```rust
fn format_results(response: &Context7Response) -> Vec<Content> {
    let mut output = String::new();

    // 标题
    output.push_str(&format!(
        "# Context7 搜索结果\n\n共找到 {} 条结果（第 {}/{} 页）\n\n",
        response.total,
        response.page,
        response.total_pages()
    ));

    // 结果列表
    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!(
            "## {}. {}\n\n",
            i + 1,
            result.title
        ));

        output.push_str(&format!(
            "**来源**: {}\n\n",
            result.source
        ));

        output.push_str(&format!(
            "{}\n\n",
            result.snippet
        ));

        output.push_str(&format!(
            "🔗 [查看文档]({})\n\n",
            result.url
        ));

        output.push_str("---\n\n");
    }

    // 分页提示
    if response.has_next_page() {
        output.push_str("💡 使用 `page: 2` 查看下一页\n");
    }

    vec![Content::text(output)]
}
```

---

## 支持的技术栈

### 前端框架
- React
- Vue
- Angular
- Svelte
- Next.js
- Nuxt.js

### CSS 框架
- Tailwind CSS
- Bootstrap
- Material-UI
- Ant Design
- Chakra UI

### 后端框架
- Express.js
- Fastify
- NestJS
- Django
- Flask

### 工具库
- Lodash
- Axios
- Day.js
- Zod
- Vite

---

## 数据流程

### 查询流程
```
AI 请求 → 加载配置 → 构建请求 → 调用 API → 解析响应 → 格式化输出 → 返回结果
```

### 降级流程
```
精确搜索 → 失败 → 模糊搜索 → 失败 → 关键词搜索 → 返回结果
```

---

## 常见问题 (FAQ)

### Q: 如何配置 API Key？
A: 在配置文件中设置 `context7_config.api_key`

### Q: 支持哪些语言？
A: 主要支持英文文档，部分框架支持中文

### Q: 如何查看下一页？
A: 设置 `page: 2` 参数

### Q: 搜索结果为空怎么办？
A: 尝试使用更通用的关键词或检查拼写

### Q: 如何测试连接？
A: 调用 Tauri 命令 `test_context7_connection`

---

## 相关文件清单

### 核心文件
- `mcp.rs` - MCP 工具实现
- `commands.rs` - Tauri 命令
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 配置文件
- `config.json` - 全局配置

---

## 使用示例

### 基础查询
```rust
let request = Context7Request {
    query: "React hooks useEffect".to_string(),
    page: Some(1),
};

let result = Context7Tool::query(request).await?;
```

### 分页查询
```rust
let request = Context7Request {
    query: "Vue composition API".to_string(),
    page: Some(2),
};

let result = Context7Tool::query(request).await?;
```

### 测试连接
```rust
let config = Context7Config {
    base_url: "https://api.context7.com".to_string(),
    api_key: Some("your-api-key".to_string()),
    page_size: 10,
};

let result = test_context7_connection(config).await?;
```

---

**最后更新**: 2026-02-18
