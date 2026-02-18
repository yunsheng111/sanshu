# 技能运行时 (skills)

[根目录](../../../../../CLAUDE.md) > [rust](../../../CLAUDE.md) > [mcp](../../CLAUDE.md) > [tools](../CLAUDE.md) > **skills**

---

## 模块职责

技能运行时 (skills)，提供动态加载和执行 Python 技能脚本的能力。支持多生态目录扫描、配置驱动、安全沙箱和动态工具注册。

---

## 入口与启动

### MCP 工具定义
- **工具名**: `skill_run` (通用入口)
- **动态工具**: `skill_<name>` (每个技能一个工具)
- **状态**: 默认启用

### 核心结构
```rust
pub struct SkillsTool;

impl SkillsTool {
    pub fn list_dynamic_tools(project_root: &Path) -> Vec<Tool>
    pub async fn call_tool(tool_name: &str, request: SkillRunRequest, project_root: &Path) -> Result<CallToolResult, McpError>
}
```

---

## 对外接口

### MCP 工具调用

#### 通用入口
```json
{
  "tool": "skill_run",
  "arguments": {
    "skill_name": "ui-ux-pro-max",
    "action": "search",
    "query": "glassmorphism",
    "args": []
  }
}
```

#### 动态工具
```json
{
  "tool": "skill_ui-ux-pro-max",
  "arguments": {
    "action": "search",
    "query": "glassmorphism"
  }
}
```

### 请求参数
```rust
pub struct SkillRunRequest {
    /// 技能名称（仅 skill_run 需要）
    pub skill_name: Option<String>,

    /// 动作名称（如 search/design_system/custom）
    pub action: Option<String>,

    /// 查询或输入（可选）
    pub query: Option<String>,

    /// 追加参数（可选）
    pub args: Option<Vec<String>>,
}
```

---

## 关键依赖与配置

### 核心依赖
```toml
tokio = { version = "1.0", features = ["process"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
```

### 技能配置
- **文件**: `skill.config.json`
- **位置**: 技能目录根目录
- **格式**:
```json
{
  "default_action": "search",
  "actions": [
    {
      "name": "search",
      "entry": "scripts/search.py",
      "args_template": ["{query}"],
      "allow_args": true,
      "description": "搜索设计样式"
    },
    {
      "name": "design_system",
      "entry": "scripts/design_system.py",
      "args_template": ["{query}", "--stack", "react"],
      "allow_args": false,
      "description": "生成设计系统"
    }
  ]
}
```

---

## 核心功能

### 1. 技能扫描

#### 扫描路径
```rust
fn build_skill_roots(project_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // 项目内多生态目录
    let rel_paths = [
        ".codex/skills",
        ".claude/skills",
        ".continue/skills",
        ".opencode/skills",
        ".trae/skills",
        ".windsurf/skills",
        ".cursor/skills",
        ".gemini/skills",
        ".roo/skills",
        ".kiro/skills",
        ".qoder/skills",
        ".codebuddy/skills",
        ".agent/skills",
        ".shared/skills",
        "skills",
    ];

    for rel in rel_paths {
        roots.push(project_root.join(rel));
    }

    // 用户全局（Codex）
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".codex").join("skills"));
    }

    roots
}
```

#### 技能识别
```rust
fn scan_skills(project_root: &Path) -> Vec<SkillInfo> {
    let mut skills_map: HashMap<String, SkillInfo> = HashMap::new();

    for root in build_skill_roots(project_root) {
        if !root.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&root)?.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // 检查 SKILL.md
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            // 解析 Front Matter
            let content = std::fs::read_to_string(&skill_md)?;
            let (name, description) = parse_skill_front_matter(&content);

            // 加载配置
            let config = load_skill_config(&path);

            skills_map.insert(
                normalize_skill_name(&name),
                SkillInfo {
                    name: normalize_skill_name(&name),
                    description,
                    path: path.clone(),
                    config,
                }
            );
        }
    }

    skills_map.into_values().collect()
}
```

### 2. 动态工具注册

#### 工具列表生成
```rust
pub fn list_dynamic_tools(project_root: &Path) -> Vec<Tool> {
    let mut tools = Vec::new();

    // 1. 通用入口
    tools.push(Self::get_skill_run_tool_definition());

    // 2. 扫描技能
    let skills = scan_skills(project_root);

    // 3. 为每个技能注册工具
    for skill in skills {
        let tool_name = format!("skill_{}", skill.name);
        let description = skill.description.unwrap_or_else(|| "技能工具".to_string());

        tools.push(Tool {
            name: Cow::Owned(tool_name),
            description: Some(Cow::Owned(description)),
            input_schema: Arc::new(skills_input_schema()),
            ..Default::default()
        });
    }

    tools
}
```

### 3. 动作解析

#### 参数模板渲染
```rust
fn resolve_action_args(
    skill: &SkillInfo,
    action_name: &str,
    request: &mut SkillRunRequest,
) -> Result<(String, Vec<String>)> {
    // 1. 尝试使用显式配置
    if let Some(config) = &skill.config {
        if let Some(action) = config.actions.iter().find(|a| a.name == action_name) {
            let mut args = Vec::new();

            // 2. 渲染参数模板
            if let Some(template) = &action.args_template {
                for token in template {
                    if token.contains("{query}") {
                        let q = request.query.clone()
                            .ok_or_else(|| anyhow!("缺少 query 参数"))?;
                        args.push(token.replace("{query}", &q));
                    } else {
                        args.push(token.clone());
                    }
                }
            }

            // 3. 追加额外参数
            if action.allow_args.unwrap_or(false) {
                if let Some(extra) = &request.args {
                    args.extend(extra.clone());
                }
            }

            return Ok((action.entry.clone(), args));
        }
    }

    // 4. 兜底：约定式入口
    let search_entry = skill.path.join("scripts").join("search.py");
    let main_entry = skill.path.join("scripts").join("main.py");

    if search_entry.exists() {
        let mut args = Vec::new();
        if let Some(q) = &request.query {
            args.push(q.clone());
        }
        return Ok(("scripts/search.py".to_string(), args));
    }

    if main_entry.exists() {
        return Ok(("scripts/main.py".to_string(), vec![]));
    }

    Err(anyhow!("未找到可执行入口"))
}
```

### 4. Python 执行

#### 执行流程
```rust
pub async fn call_tool(
    tool_name: &str,
    mut request: SkillRunRequest,
    project_root: &Path,
) -> Result<CallToolResult, McpError> {
    // 1. 解析技能名称
    let skill_name = if tool_name == "skill_run" {
        request.skill_name.clone().unwrap_or_default()
    } else {
        tool_name.trim_start_matches("skill_").to_string()
    };

    // 2. 查找技能
    let skills = scan_skills(project_root);
    let skill = skills.into_iter()
        .find(|s| s.name.eq_ignore_ascii_case(&skill_name))
        .ok_or_else(|| McpError::invalid_params(format!("未找到技能: {}", skill_name), None))?;

    // 3. 解析动作参数
    let (entry_rel, args) = resolve_action_args(&skill, &action_name, &mut request)?;

    // 4. 构建入口路径（安全检查）
    let entry_path = skill.path.join(&entry_rel);
    let entry_path = entry_path.canonicalize()?;
    let skill_root = skill.path.canonicalize()?;

    if !entry_path.starts_with(&skill_root) {
        return Err(McpError::invalid_params("入口路径不在技能目录内".to_string(), None));
    }

    // 5. 选择 Python 执行器
    let python_bin = load_standalone_config()
        .ok()
        .and_then(|c| c.mcp_config.skill_python_path.clone())
        .unwrap_or_else(|| "python".to_string());

    // 6. 执行 Python 脚本
    let output = Command::new(&python_bin)
        .arg(&entry_path)
        .args(&args)
        .current_dir(&skill.path)
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .await?;

    // 7. 处理输出
    if !output.status.success() {
        let err_text = String::from_utf8_lossy(&output.stderr);
        return Err(McpError::internal_error(format!("技能执行失败: {}", err_text), None));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(CallToolResult::success(vec![Content::text(stdout)]))
}
```

### 5. 安全沙箱

#### 路径穿透检测
```rust
// 构建入口路径
let entry_path = skill.path.join(&entry_rel);

// 规范化路径
let entry_path = entry_path.canonicalize()
    .map_err(|e| McpError::invalid_params(format!("入口路径解析失败: {}", e), None))?;

let skill_root = skill.path.canonicalize()
    .map_err(|e| McpError::invalid_params(format!("技能路径解析失败: {}", e), None))?;

// 检查路径是否在技能目录内
if !entry_path.starts_with(&skill_root) {
    return Err(McpError::invalid_params("入口路径不在技能目录内".to_string(), None));
}
```

---

## 技能开发指南

### 1. 技能结构
```
skills/my-skill/
├── SKILL.md              # 技能文档（必需）
├── skill.config.json     # 技能配置（可选）
├── scripts/
│   ├── search.py         # 搜索入口（约定）
│   ├── main.py           # 主入口（约定）
│   └── custom.py         # 自定义入口
└── data/                 # 数据文件
```

### 2. SKILL.md 格式
```markdown
---
name: my-skill
description: "我的技能描述"
---

# 我的技能

技能详细说明...
```

### 3. skill.config.json 格式
```json
{
  "default_action": "search",
  "actions": [
    {
      "name": "search",
      "entry": "scripts/search.py",
      "args_template": ["{query}"],
      "allow_args": true,
      "description": "搜索功能"
    }
  ]
}
```

### 4. Python 脚本示例
```python
#!/usr/bin/env python3
import sys

def main():
    query = sys.argv[1] if len(sys.argv) > 1 else ""

    # 处理逻辑
    result = f"搜索结果: {query}"

    # 输出到 stdout
    print(result)

if __name__ == "__main__":
    main()
```

---

## 数据流程

### 工具注册流程
```
MCP 启动 → 扫描技能目录 → 解析 SKILL.md → 加载配置 → 注册动态工具 → 返回工具列表
```

### 工具调用流程
```
AI 请求 → 解析技能名称 → 查找技能 → 解析动作 → 构建参数 → 执行 Python → 返回结果
```

---

## 常见问题 (FAQ)

### Q: 如何添加新技能？
A: 在 `skills/` 目录创建新目录，添加 `SKILL.md` 和 Python 脚本

### Q: 如何配置 Python 路径？
A: 在配置文件中设置 `skill_python_path`

### Q: 技能如何接收参数？
A: 通过 `sys.argv` 接收命令行参数

### Q: 如何调试技能？
A: 直接运行 Python 脚本：`python scripts/search.py "test"`

### Q: 支持哪些 Python 版本？
A: Python 3.7+

---

## 相关文件清单

### 核心文件
- `mod.rs` - 技能运行时实现

### 技能目录
- `skills/ui-ux-pro-max/` - UI/UX 设计技能

---

## 使用示例

### 通用入口
```rust
let request = SkillRunRequest {
    skill_name: Some("ui-ux-pro-max".to_string()),
    action: Some("search".to_string()),
    query: Some("glassmorphism".to_string()),
    args: None,
};

let result = SkillsTool::call_tool("skill_run", request, project_root).await?;
```

### 动态工具
```rust
let request = SkillRunRequest {
    skill_name: None,
    action: Some("search".to_string()),
    query: Some("glassmorphism".to_string()),
    args: None,
};

let result = SkillsTool::call_tool("skill_ui-ux-pro-max", request, project_root).await?;
```

---

**最后更新**: 2026-02-18
