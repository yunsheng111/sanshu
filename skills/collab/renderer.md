# 占位符渲染模块

collab Skill 的占位符渲染逻辑，负责将命令模板中的占位符替换为实际值。

## 占位符定义

| 占位符 | 说明 | 来源 |
|--------|------|------|
| `{{CCG_BIN}}` | codeagent-wrapper 可执行文件路径 | `.ccg/config.toml` 或默认值 |
| `{{WORKDIR}}` | 当前工作目录绝对路径 | 运行时获取 |
| `{{LITE_MODE_FLAG}}` | 轻量模式标志 | 环境变量 `LITE_MODE` |
| `{{GEMINI_MODEL_FLAG}}` | Gemini 模型指定标志 | 环境变量 `GEMINI_MODEL` |
| `{{ROLE}}` | 角色参数 | Skill 输入参数 |
| `{{TASK}}` | 任务描述 | Skill 输入参数 |

## 渲染规则

### CCG_BIN

```markdown
1. 读取 `.ccg/config.toml` 中的 `CCG_BIN` 配置
2. 若不存在，使用默认值：`~/.claude/bin/codeagent-wrapper.exe`（Windows）
3. 若不存在，使用默认值：`~/.claude/bin/codeagent-wrapper`（Unix）
```

### WORKDIR

```markdown
1. 获取当前工作目录的绝对路径
2. 使用正斜杠（/）作为路径分隔符（跨平台兼容）
```

### LITE_MODE_FLAG

```markdown
1. 检查环境变量 `LITE_MODE`
2. 若 `LITE_MODE=true`，替换为 `--lite `（注意尾随空格）
3. 否则替换为空字符串
```

### GEMINI_MODEL_FLAG

```markdown
1. 检查环境变量 `GEMINI_MODEL`
2. 若存在且非空，替换为 `--gemini-model <model> `（注意尾随空格）
3. 否则替换为空字符串
```

## 渲染流程

### 步骤 1：收集变量

```markdown
1. 读取配置文件获取 CCG_BIN
2. 获取当前工作目录
3. 读取环境变量
4. 接收 Skill 输入参数
```

### 步骤 2：构建替换映射

```javascript
const replacements = {
  '{{CCG_BIN}}': ccgBinPath,
  '{{WORKDIR}}': workdir,
  '{{LITE_MODE_FLAG}}': liteModeFlag,
  '{{GEMINI_MODEL_FLAG}}': geminiModelFlag,
  '{{ROLE}}': role,
  '{{TASK}}': task
}
```

### 步骤 3：执行替换

```markdown
1. 遍历替换映射
2. 对命令模板执行字符串替换
3. 处理特殊字符转义（引号、反斜杠）
```

### 步骤 4：残留检测

```markdown
1. 使用正则 `/\{\{[^}]+\}\}/g` 检测残留占位符
2. 若存在残留，抛出错误并列出未替换的占位符
3. 若无残留，返回渲染后的命令
```

## 错误处理

### 配置文件不存在

```markdown
- 使用默认 CCG_BIN 路径
- 记录警告日志
```

### CCG_BIN 文件不存在

```markdown
- 抛出错误：`CCG_BIN 路径无效：{path}`
- 建议用户检查安装
```

### 残留占位符

```markdown
- 抛出错误：`渲染失败：命令中存在残留占位符 {{UNKNOWN_VAR}}`
- 列出所有残留占位符
- 拒绝执行命令
```

## 使用示例

### 输入

```markdown
模板（stdin 模式）：
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/codex/{{ROLE}}.md
<TASK>
需求：{{TASK}}
</TASK>
OUTPUT: structured-analysis' | {{CCG_BIN}} --backend codex {{LITE_MODE_FLAG}}- "{{WORKDIR}}"

参数：
- role: architect
- task: 分析用户认证模块
- LITE_MODE: true
- GEMINI_MODEL: (未设置)
```

### 输出

```bash
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/codex/architect.md
<TASK>
需求：分析用户认证模块
</TASK>
OUTPUT: structured-analysis' | C:/Users/Administrator/.claude/bin/codeagent-wrapper.exe --backend codex --lite - "C:/project"
```

> **注意**：codeagent-wrapper 的有效标志参数仅为 `--backend`、`--lite`、`--gemini-model`、`--skip-permissions`。
> `--workdir`、`--role`、`--task` 不是有效参数。工作目录通过位置参数传递，任务内容通过 stdin（`-`）或第一个位置参数传递。

## 安全考虑

### 命令注入防护

```markdown
1. 对 TASK 参数进行转义处理
2. 禁止在 TASK 中包含 shell 特殊字符（`; | & $ \` 等）
3. 使用双引号包裹 TASK 和 WORKDIR
```

### 路径验证

```markdown
1. CCG_BIN 必须是绝对路径
2. WORKDIR 必须是存在的目录
3. 禁止路径遍历（..）
```

## 与其他模块的关系

```
renderer.md (本模块)
    ↓ 渲染后的命令
executor.md (并行调用)
    ↓ 执行结果
state-machine.md (状态管理)
```
