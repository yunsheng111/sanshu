# UI/UX 设计工具 (uiux)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **uiux**

---

## 模块职责

UI/UX Pro Max 设计工具，提供设计系统生成、样式搜索、技术栈推荐和设计规范查询。内置 50+ 样式、97 色板、57 字体对和 9 种技术栈支持。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `uiux`
- **标识符**: `mcp______uiux`
- **状态**: 默认启用

### 核心结构
```rust
pub struct UiuxTool;

impl UiuxTool {
    pub async fn call_from_skill(action: &str, request: &SkillRunRequest) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用
```json
{
  "tool": "skill_ui-ux-pro-max",
  "arguments": {
    "action": "search",
    "query": "glassmorphism dashboard"
  }
}
```

### 支持的操作
| 操作 | 说明 | 参数 |
|------|------|------|
| `search` | 搜索设计样式 | `query` |
| `design_system` | 生成设计系统 | `query`, `stack` |
| `color` | 查询色板 | `query` |
| `typography` | 查询字体对 | `query` |
| `chart` | 查询图表类型 | `query` |
| `stack` | 查询技术栈 | `query` |

### 请求参数
```rust
pub struct SkillRunRequest {
    pub skill_name: Option<String>,
    pub action: Option<String>,
    pub query: Option<String>,
    pub args: Option<Vec<String>>,
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
rust-embed = "8.0"
csv = "1.3"
serde = { version = "1.0", features = [ "derive" ] }
serde_json = "1.0"
regex = "1.0"
```

### 数据源
- **位置**: `skills/ui-ux-pro-max/data/`
- **格式**: CSV 文件（嵌入式）
- **数据集**:
  - `styles.csv` - 50+ 设计样式
  - `colors.csv` - 97 色板
  - `typography.csv` - 57 字体对
  - `charts.csv` - 25 图表类型
  - `stacks/*.csv` - 9 种技术栈

---

## 核心功能

### 1. 设计样式搜索 (`engine.rs`)

#### 搜索引擎
```rust
pub struct UiuxEngine;

impl UiuxEngine {
    /// 搜索设计样式
    pub fn search(domain: &str, query: &str) -> Result<String> {
        // 1. 加载数据
        let data = load_domain_data(domain)?;

        // 2. 计算相似度
        let mut scored_rows: Vec<(f64, &Row)> = data
            .iter()
            .map(|row| {
                let score = calculate_similarity(query, row);
                (score, row)
            })
            .collect();

        // 3. 排序（相似度降序）
        scored_rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        // 4. 取前 N 个结果
        let top_results = scored_rows.iter().take(MAX_RESULTS);

        // 5. 格式化输出
        format_results(domain, top_results)
    }
}
```

#### 相似度算法
```rust
fn calculate_similarity(query: &str, row: &Row) -> f64 {
    let query_lower = query.to_lowercase();
    let mut score = 0.0;

    // 1. 精确匹配（权重 1.0）
    for col in search_cols {
        if let Some(value) = row.get(col) {
            if value.to_lowercase().contains(&query_lower) {
                score += 1.0;
            }
        }
    }

    // 2. 词级匹配（权重 0.5）
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    for col in search_cols {
        if let Some(value) = row.get(col) {
            let value_lower = value.to_lowercase();
            for word in &query_words {
                if value_lower.contains(word) {
                    score += 0.5;
                }
            }
        }
    }

    // 3. 使用 TextSimilarity（权重 0.3）
    for col in search_cols {
        if let Some(value) = row.get(col) {
            let similarity = TextSimilarity::calculate(query, value);
            score += similarity * 0.3;
        }
    }

    score
}
```

### 2. 设计系统生成

#### 生成流程
```rust
pub fn generate_design_system(query: &str, stack: Option<&str>) -> Result<String> {
    let mut output = String::new();

    // 1. 样式推荐
    let styles = search("style", query)?;
    output.push_str("## 样式推荐\n\n");
    output.push_str(&styles);

    // 2. 色板推荐
    let colors = search("color", query)?;
    output.push_str("\n## 色板推荐\n\n");
    output.push_str(&colors);

    // 3. 字体推荐
    let typography = search("typography", query)?;
    output.push_str("\n## 字体推荐\n\n");
    output.push_str(&typography);

    // 4. 技术栈推荐
    if let Some(stack_name) = stack {
        let stack_guide = search_stack(stack_name, query)?;
        output.push_str(&format!("\n## {} 实现指南\n\n", stack_name));
        output.push_str(&stack_guide);
    }

    // 5. UX 指南
    let ux_guidelines = search("ux", query)?;
    output.push_str("\n## UX 指南\n\n");
    output.push_str(&ux_guidelines);

    Ok(output)
}
```

### 3. 数据加载 (`engine.rs`)

#### 嵌入式数据
```rust
#[derive(RustEmbed)]
#[folder = "skills/ui-ux-pro-max/data"]
struct EmbeddedUiuxData;

fn load_domain_data(domain: &str) -> Result<Vec<Row>> {
    // 1. 获取配置
    let config = DOMAIN_CONFIGS.get(domain)
        .ok_or_else(|| anyhow!("未知域: {}", domain))?;

    // 2. 读取嵌入式文件
    let file_content = EmbeddedUiuxData::get(config.file)
        .ok_or_else(|| anyhow!("文件不存在: {}", config.file))?;

    // 3. 解析 CSV
    let mut reader = csv::Reader::from_reader(file_content.data.as_ref());
    let mut rows = Vec::new();

    for result in reader.records() {
        let record = result?;
        rows.push(record);
    }

    Ok(rows)
}
```

### 4. 输出格式化 (`response.rs`)

#### 格式化策略
```rust
pub fn format_results(domain: &str, results: &[(f64, &Row)]) -> String {
    let mut output = String::new();

    // 1. 标题
    output.push_str(&format!("# {} 搜索结果\n\n", domain_title(domain)));

    // 2. 结果列表
    for (i, (score, row)) in results.iter().enumerate() {
        output.push_str(&format!("## {}. {}\n\n", i + 1, row.get("name")?));

        // 3. 字段输出
        for col in output_cols {
            if let Some(value) = row.get(col) {
                output.push_str(&format!("**{}**: {}\n\n", col, value));
            }
        }

        // 4. 相似度（调试模式）
        if cfg!(debug_assertions) {
            output.push_str(&format!("_相似度: {:.2}_\n\n", score));
        }

        output.push_str("---\n\n");
    }

    output
}
```

### 5. 本地化 (`localize.rs`)

#### 中文输出
```rust
pub fn localize_output(output: &str) -> String {
    let mut localized = output.to_string();

    // 1. 替换字段名
    localized = localized.replace("Style Category", "样式分类");
    localized = localized.replace("Primary Colors", "主色调");
    localized = localized.replace("Best For", "适用场景");

    // 2. 替换值
    localized = localized.replace("Dashboard", "仪表盘");
    localized = localized.replace("Landing Page", "落地页");
    localized = localized.replace("E-commerce", "电商");

    localized
}
```

---

## 支持的域

### 设计域
| 域 | 数据文件 | 搜索列 | 输出列 |
|------|----------|--------|--------|
| `style` | `styles.csv` | Style Category, Keywords, Best For | Style Category, Type, Keywords, Primary Colors, Effects & Animation, Best For |
| `color` | `colors.csv` | Product Type, Keywords | Product Type, Keywords, Primary (Hex), Secondary (Hex), Accent (Hex) |
| `typography` | `typography.csv` | Heading Font, Body Font, Keywords | Heading Font, Body Font, Keywords, Personality, Best For |
| `chart` | `charts.csv` | Chart Type, Keywords | Chart Type, Keywords, Best For, Data Type |
| `ux` | `ux-guidelines.csv` | Rule ID, Keywords | Rule ID, Priority, Impact, Description |

### 技术栈
- React
- Next.js
- Vue
- Nuxt.js
- Svelte
- SwiftUI
- React Native
- Flutter
- Tailwind CSS
- shadcn/ui

---

## 数据流程

### 搜索流程
```
AI 请求 → 解析域 → 加载数据 → 计算相似度 → 排序 → 格式化 → 本地化 → 返回结果
```

### 设计系统生成流程
```
AI 请求 → 样式搜索 → 色板搜索 → 字体搜索 → 技术栈搜索 → UX 指南 → 整合输出
```

---

## 单元测试

### 测试覆盖
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_styles() {
        let result = UiuxEngine::search("style", "glassmorphism");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Glassmorphism"));
    }

    #[test]
    fn test_search_colors() {
        let result = UiuxEngine::search("color", "dashboard");
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_design_system() {
        let result = generate_design_system("dashboard", Some("react"));
        assert!(result.is_ok());
    }
}
```

---

## 常见问题 (FAQ)

### Q: 如何添加新的设计样式？
A: 编辑 `skills/ui-ux-pro-max/data/styles.csv`，添加新行

### Q: 支持哪些技术栈？
A: React, Vue, Svelte, SwiftUI, React Native, Flutter, Tailwind CSS, shadcn/ui

### Q: 如何自定义输出格式？
A: 修改 `response.rs` 中的 `format_results()` 函数

### Q: 数据如何更新？
A: 更新 CSV 文件后重新编译（数据嵌入式）

---

## 相关文件清单

### 核心文件
- `engine.rs` - 搜索引擎
- `lexicon.rs` - 词汇表
- `mcp.rs` - MCP 工具实现
- `response.rs` - 输出格式化
- `sanitize.rs` - 路径清理
- `localize.rs` - 本地化
- `types.rs` - 数据类型定义
- `mod.rs` - 模块导出

### 数据文件
- `skills/ui-ux-pro-max/data/*.csv` - 设计数据
- `skills/ui-ux-pro-max/SKILL.md` - 技能文档

---

## 使用示例

### 搜索样式
```rust
let request = SkillRunRequest {
    skill_name: Some("ui-ux-pro-max".to_string()),
    action: Some("search".to_string()),
    query: Some("glassmorphism dashboard".to_string()),
    args: None,
};

let result = UiuxTool::call_from_skill("search", &request).await?;
```

### 生成设计系统
```rust
let request = SkillRunRequest {
    skill_name: Some("ui-ux-pro-max".to_string()),
    action: Some("design_system".to_string()),
    query: Some("e-commerce".to_string()),
    args: Some(vec!["react".to_string()]),
};

let result = UiuxTool::call_from_skill("design_system", &request).await?;
```

### 查询色板
```rust
let request = SkillRunRequest {
    skill_name: Some("ui-ux-pro-max".to_string()),
    action: Some("color".to_string()),
    query: Some("dashboard".to_string()),
    args: None,
};

let result = UiuxTool::call_from_skill("color", &request).await?;
```

---

**最后更新**: 2026-02-18
