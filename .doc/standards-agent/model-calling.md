---
version: v1.0.0
---

# 多模型调用规范

## collab Skill（推荐）

**推荐使用 `/collab` Skill 封装双模型调用**，自动处理占位符渲染、状态机管理、SESSION_ID 提取、门禁校验、超时处理和进度汇报。

### 调用语法

```text
/collab backend=both role=analyzer task="<任务描述>"
```

### 参数说明

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `backend` | string | 否 | `both` | 调用后端：`codex`、`gemini`、`both` |
| `role` | string | 是 | - | 角色：`architect`、`analyzer`、`reviewer`、`developer` |
| `task` | string | 是 | - | 任务描述（自然语言） |
| `resume` | string | 否 | - | 复用的 SESSION_ID（用于会话续接） |

### 会话复用

```text
/collab backend=both role=architect task="基于分析生成计划" resume=<CODEX_SESSION>
```

### 输出格式

collab Skill 返回：
- `codex_session`: Codex 会话 ID
- `gemini_session`: Gemini 会话 ID
- `status`: `SUCCESS` / `DEGRADED` / `FAILED`（状态枚举真值源：`skills/collab/SKILL.md`；缺失时沿用兼容模式定义）
- `degraded_level`: `ACCEPTABLE` / `UNACCEPTABLE` / `null`（仅当 `status=DEGRADED` 时有值）
- `missing_dimensions`: `['backend']` / `['frontend']` / `null`（仅当 `status=DEGRADED` 时有值）
- `codex_output`: Codex 输出内容
- `gemini_output`: Gemini 输出内容

### collab 缺失兼容模式（来自 ccg-workflow）

当 `skills/collab/SKILL.md` 不存在时，禁止中断双模型流程，改用 `codeagent-wrapper` 直连模式继续执行。

#### 一键修复缺失文件

```bash
# 先检查目标仓库
node ~/.claude/.ccg/scripts/check-collab-setup.cjs --repo "<你的项目目录>"

# 自动补齐 collab Skill + 模板文档
node ~/.claude/.ccg/scripts/bootstrap-collab.cjs --repo "<你的项目目录>"

# 严格复检
node ~/.claude/.ccg/scripts/check-collab-setup.cjs --repo "<你的项目目录>" --strict
```

> 若你的 CCG 安装目录不是 `~/.claude`，请改用实际脚本路径，或在执行前设置 `CCG_HOME` / `COLLAB_SKILL_DIR`。

#### 兼容模式判定

1. 先检查 `skills/collab/SKILL.md` 是否存在
2. 存在：继续使用 `/collab ...`
3. 不存在：切换到下述 wrapper 模板（stdin + resume 子命令）

#### 兼容模式命令模板

```bash
# Codex 新会话
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/codex/<role>.md
<TASK>
需求：<增强后的需求>
上下文：<项目上下文>
</TASK>
OUTPUT: <期望输出格式>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend codex - "{{WORKDIR}}"

# Gemini 新会话
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/gemini/<role>.md
<TASK>
需求：<增强后的需求>
上下文：<项目上下文>
</TASK>
OUTPUT: <期望输出格式>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend gemini {{GEMINI_MODEL_FLAG}}- "{{WORKDIR}}"
```

```bash
# Codex 复用会话
echo '<TASK>继续任务：<补充需求></TASK>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend codex resume <CODEX_SESSION> - "{{WORKDIR}}"

# Gemini 复用会话
echo '<TASK>继续任务：<补充需求></TASK>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend gemini {{GEMINI_MODEL_FLAG}}resume <GEMINI_SESSION> - "{{WORKDIR}}"
```

兼容模式下仍需满足同一门禁：`liteMode || codexSession || geminiSession`。

---

## single-model 代理调用规范

### 适用代理

以下代理使用单一外部模型，**必须显式指定 `backend` 参数**：

| 代理 | 主导模型 | `backend` 值 | ROLE_FILE 前缀 |
|------|----------|--------------|----------------|
| `frontend-agent` | Gemini | `gemini` | `.ccg/prompts/gemini/` |
| `backend-agent` | Codex | `codex` | `.ccg/prompts/codex/` |

### 强制规则

1. **禁止省略 `backend` 参数**。
2. **禁止依赖默认行为**，必须显式写 `backend=codex` 或 `backend=gemini`。
3. **ROLE_FILE 必须匹配模型**，Gemini 代理使用 `gemini/`，Codex 代理使用 `codex/`。

### Gemini 专用命令模板（frontend-agent）

```text
/collab backend=gemini role=analyzer task="<增强后的需求 + 项目上下文>"
/collab backend=gemini role=<architect|reviewer> resume=<GEMINI_SESSION> task="<增强后的需求 + 项目上下文>"
```

### Codex 专用命令模板（backend-agent）

```text
/collab backend=codex role=analyzer task="<增强后的需求 + 项目上下文>"
/collab backend=codex role=<architect|reviewer> resume=<CODEX_SESSION> task="<增强后的需求 + 项目上下文>"
```

> 说明：single-model 代理仍由单模型主导，但统一通过 collab 封装，避免 shell 兼容和参数拼接错误。

### 降级策略（single-model 专用）

single-model 代理降级时**不切换到另一个模型**，由 Claude 独立完成：

- frontend-agent：Gemini 失败 -> Claude 独立完成（禁止降级到 Codex）
- backend-agent：Codex 失败 -> Claude 独立完成（禁止降级到 Gemini）

降级时通过 `mcp______zhi` 通知用户当前处于降级模式。

### 与 config.toml routing 配置的关系

`config.toml` 中的 `[routing.<command>]` 配置声明命令主导模型：

```toml
[routing.frontend]
primary = "gemini"

[routing.backend]
primary = "codex"
```

运行时可通过 `command-renderer.cjs` 的 `getRoutingBackend()` 读取配置，并渲染为 `{{BACKEND_FLAG}}`（若启用）。代理模板中的显式 `backend` 参数必须与该配置保持一致。

---

## 底层机制（collab Skill 实现层）

以下内容仅用于 collab Skill 内部实现，业务代理优先使用 `/collab`。

### 占位符

| 占位符 | 替换值 | 来源 |
|--------|--------|------|
| `{{CCG_BIN}}` | codeagent-wrapper 路径 | `.ccg/config.toml`，默认 `~/.claude/bin/codeagent-wrapper.exe` |
| `{{WORKDIR}}` | 当前工作目录绝对路径 | 运行时 `process.cwd()` |
| `{{LITE_MODE_FLAG}}` | `--lite ` 或空字符串 | 环境变量 `LITE_MODE=true` 时生成（带尾随空格） |
| `{{GEMINI_MODEL_FLAG}}` | `--gemini-model <model> ` 或空字符串 | 环境变量 `GEMINI_MODEL` 非空时生成（带尾随空格） |

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `LITE_MODE` | `true` 时跳过外部模型调用，使用模拟响应 | `false` |
| `GEMINI_MODEL` | Gemini 模型版本 | `gemini-3-pro-preview` |

**LITE_MODE 检查**：调用外部模型前必须检查。若为 `true`，跳过 Codex/Gemini 调用，使用占位符响应继续流程。

### 调用语法（底层）

> 禁止使用 `--prompt` 或 `--resume`。会话复用必须使用 `resume <SESSION_ID>` 子命令。

#### 新会话（stdin 模式）

```bash
echo 'ROLE_FILE: <角色提示词路径>
<TASK>
需求：<增强后的需求>
上下文：<检索到的项目上下文>
</TASK>
OUTPUT: <期望输出格式>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend <codex|gemini> {{GEMINI_MODEL_FLAG}}- "{{WORKDIR}}"
```

#### 复用会话（resume 子命令）

```bash
echo 'ROLE_FILE: <角色提示词路径>
<TASK>
需求：<增强后的需求>
上下文：<检索到的项目上下文>
</TASK>
OUTPUT: <期望输出格式>' | {{CCG_BIN}} {{LITE_MODE_FLAG}}--backend <codex|gemini> {{GEMINI_MODEL_FLAG}}resume <SESSION_ID> - "{{WORKDIR}}"
```

#### 并行调用

使用 `run_in_background: true` + `timeout: 3600000`。

#### TaskOutput 等待

```text
TaskOutput({ task_id: "<task_id>", block: true, timeout: 600000 })
```

- 必须指定 `timeout: 600000`（10 分钟）
- 超时后继续轮询，**绝对不要 Kill 进程**
- 等待过长时调用 `mcp______zhi` 询问用户是否继续等待

### 角色提示词路径

| 阶段 | Codex | Gemini |
|------|-------|--------|
| 分析 | `~/.claude/.ccg/prompts/codex/analyzer.md` | `~/.claude/.ccg/prompts/gemini/analyzer.md` |
| 规划 | `~/.claude/.ccg/prompts/codex/architect.md` | `~/.claude/.ccg/prompts/gemini/architect.md` |
| 审查 | `~/.claude/.ccg/prompts/codex/reviewer.md` | `~/.claude/.ccg/prompts/gemini/reviewer.md` |
| 前端 | — | `~/.claude/.ccg/prompts/gemini/frontend.md` |

### 会话复用

每次调用返回 `SESSION_ID: xxx`。后续阶段用 `resume <SESSION_ID>` 复用上下文。

### 信任规则

- 后端（API、数据库、性能、安全）：**Codex 为准**
- 前端（UI、交互、可访问性、设计）：**Gemini 为准**
- 外部模型对文件系统**零写入权限**，所有修改由 Claude 执行

### 占位符生命周期

占位符在运行时动态渲染，由 `.ccg/runtime/command-renderer.cjs` 负责：
1. 读取 `.ccg/config.toml` 获取 `CCG_BIN`
2. 构建运行时变量映射（`LITE_MODE`、`GEMINI_MODEL`、`process.cwd()`）
3. 替换模板中的 `{{...}}`
4. 验证无残留占位符；若残留则拒绝执行
5. 仅在校验通过后执行 Bash 命令

## 强制调用规则

对于标记为 `template: multi-model` 的代理：

1. **LITE_MODE 检查**：`LITE_MODE=true` 时跳过外部模型调用。
2. **门禁校验**：执行门禁为 `liteMode || codexSession || geminiSession`。
3. **关键规则**：双模型均无 SESSION_ID => `FAILED`（非 `DEGRADED`）。
4. **超时处理**：超时继续轮询，不自动重启任务。
5. **降级策略**：重试 -> 单模型 -> 主代理兜底。
6. **复用模板**：引用 `.doc/standards-agent/dual-model-orchestration.md`，不重复造轮子。

### 适用代理

- `fullstack-agent`、`analyze-agent`、`planner`、`execute-agent`
- `team-plan-agent`、`team-review-agent`、`team-research-agent`
- `spec-research-agent`、`spec-plan-agent`、`spec-review-agent`、`spec-impl-agent`
- `fullstack-light-agent`

> **语义真源引用**：状态枚举、门禁逻辑、降级策略优先以 `skills/collab/SKILL.md` 为准；若缺失则按本文档“collab 缺失兼容模式”执行。门禁权威定义见 `.doc/standards-agent/dual-model-orchestration.md`。

> **Legacy 警告**：`codexCalled`/`geminiCalled` 私有标志位已废弃。门禁只基于 `SESSION_ID`（`codexSession || geminiSession`）。
