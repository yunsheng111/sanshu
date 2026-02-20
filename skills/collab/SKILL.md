# collab Skill

> **⛔ 强制执行指令（读取即生效）**
>
> 任何读取本文档的代理必须：
> 1. 按照下方"执行流程"章节的 4 个步骤**完整执行**，不得跳过任何步骤
> 2. 按照 `reporter.md` 的规范**每 30 秒**通过 `mcp______zhi` 向用户汇报进度
> 3. 通过 Bash 工具**实际调用** codeagent-wrapper，不得由 Claude 自行分析替代
> 4. 从 Bash 输出中**提取真实 SESSION_ID**，不得编造或伪造
> 5. 将 SESSION_ID 写入最终产出文档，供 PreToolUse Hook 验证
>
> 违反以上任一条即视为执行失败，PreToolUse Hook（`ccg-dual-model-validator.cjs`）将拦截缺少 SESSION_ID 的报告写入。

双模型（Codex + Gemini）协作调用 Skill，封装 codeagent-wrapper 的并行调用、状态管理和进度汇报。

> **📌 真值源声明**
>
> 本文档（`skills/collab/SKILL.md`）是 CCG 框架中以下定义的**唯一真值源**：
> - **状态枚举**：`SUCCESS | DEGRADED | FAILED`
> - **事件枚举**：`init | running | success | degraded | failed`
> - **降级分级**：`ACCEPTABLE | UNACCEPTABLE`
> - **门禁语义**：执行门禁（OR 逻辑）和质量门禁的判定规则
>
> 其他文档（`model-calling.md`、`dual-model-orchestration.md`、代理文件等）中的状态枚举、事件和门禁描述必须与本文档一致。如发现不一致，以本文档为准。

## 触发条件

当用户需要同时调用 Codex 和 Gemini 进行技术分析、架构规划或代码审查时使用。

触发关键词：
- "双模型分析"、"多模型协作"
- "Codex + Gemini"、"后端 + 前端视角"
- 显式调用 `/collab`

## 输入参数

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `backend` | string | 否 | `both` | 调用后端：`codex`、`gemini`、`both` |
| `role` | string | 是 | - | 角色：`architect`、`analyzer`、`reviewer`、`developer` |
| `task` | string | 是 | - | 任务描述（自然语言） |
| `parallel` | boolean | 否 | `true` | 是否并行调用（仅 `backend=both` 时有效） |
| `timeout` | number | 否 | `600000` | 超时时间（毫秒） |
| `resume` | string | 否 | - | 复用的 SESSION_ID（用于会话续接） |
| `progress_callback` | boolean | 否 | `true` | 是否启用进度回调 |
| `progress_interval` | number | 否 | `30000` | 进度汇报间隔（毫秒） |
| `stream` | boolean | 否 | `false` | 是否启用流式输出 |

## 输出格式

```json
{
  "status": "SUCCESS | DEGRADED | FAILED",
  "degraded_level": "ACCEPTABLE | UNACCEPTABLE | null",
  "missing_dimensions": ["backend" | "frontend"],
  "codex_session": "uuid-string | null",
  "gemini_session": "uuid-string | null",
  "codex_output": "string | null",
  "gemini_output": "string | null",
  "duration_ms": 12345,
  "degraded_reason": "string | null"
}
```

### 状态契约说明

| 字段 | 说明 |
|------|------|
| `status` | 唯一状态枚举：`SUCCESS`（双模型均成功）、`DEGRADED`（单模型成功）、`FAILED`（双模型均失败或双模型均无 SESSION_ID） |
| `degraded_level` | 仅当 `status=DEGRADED` 时有值：`ACCEPTABLE`（非核心维度缺失）、`UNACCEPTABLE`（核心维度缺失，需用户介入） |
| `missing_dimensions` | 仅当 `status=DEGRADED` 时有值：标注缺失的分析维度（`backend` 或 `frontend`） |

## 信任规则

- **后端领域**（API、数据库、性能、安全）：**Codex 为准**
- **前端领域**（UI、交互、可访问性、设计）：**Gemini 为准**
- **冲突时**：通过 `mcp______zhi` 展示双方观点，由用户决策

## 状态机

```
INIT → RUNNING → SUCCESS
              ↓
         DEGRADED (degraded_level: ACCEPTABLE / UNACCEPTABLE)
              ↓ (ACCEPTABLE + 用户确认)
           SUCCESS
              ↓ (UNACCEPTABLE 或双模型均无 SESSION_ID)
           FAILED
```

### 状态说明

| 状态 | 说明 | 触发条件 |
|------|------|----------|
| `INIT` | 初始化，准备调用 | Skill 启动时 |
| `RUNNING` | 模型正在执行 | 进程启动后 |
| `SUCCESS` | 双模型均成功返回且满足业务门禁 | `codex_session` 和 `gemini_session` 均存在 |
| `DEGRADED` | 单模型成功，另一模型失败/超时 | 仅 `codex_session` 或 `gemini_session` 其一存在 |
| `FAILED` | 执行失败 | 双模型均失败，或**双模型均无 SESSION_ID** |

### DEGRADED 分级（degraded_level）

| 级别 | 说明 | 后续动作 |
|------|------|----------|
| `ACCEPTABLE` | 非核心维度缺失，核心目标已达成（满足 OR 执行门禁） | 标注 `missing_dimensions`，经用户确认后可继续 |
| `UNACCEPTABLE` | 核心维度缺失或质量不达标 | 标注 `missing_dimensions` + 影响评估，必须用户介入决策 |

### 关键规则

- **双模型均无 SESSION_ID => `FAILED`**：即使有文字输出，若双模型均未返回有效 SESSION_ID，状态为 `FAILED`，不得标记为 `DEGRADED`
- **单模型有 SESSION_ID => `DEGRADED`**：必须标注 `missing_dimensions`（缺失的维度：`backend` 或 `frontend`）
- **DEGRADED 产出前置动作**：标注缺失维度 + 风险影响 + 补偿分析，经 `mcp______zhi` 确认后才能进入下一阶段

## 门禁协议 (HC-2)

1. **AND 门禁**：双模型必须全部 SUCCESS。
2. **OR 门禁 (默认)**：至少一个核心视角（由 role 决定）SUCCESS，次要视角允许缺失但必须标注为 DEGRADED。
   - `role=architect`: 后端(Codex) 为核心
   - `role=analyzer`: 双重视角均为核心
   - `role=reviewer`: 前端(Gemini) 为核心 (UI/UX 场景)

## 事件枚举

> **真值源**：以下事件枚举是 CCG 框架中双模型编排事件的唯一权威定义。

| 事件 | 说明 | 触发时机 |
|------|------|----------|
| `init` | Skill 初始化 | 参数解析完成、占位符渲染完成后 |
| `running` | 模型进程已启动 | Bash 进程启动后 |
| `success` | 双模型均成功返回 | 状态机转入 `SUCCESS` |
| `degraded` | 单模型成功，另一模型失败/超时 | 状态机转入 `DEGRADED`，触发降级分级评估（`degraded_level`） |
| `failed` | 双模型均失败或均无有效 SESSION_ID | 状态机转入 `FAILED` |

## 降级策略

| 级别 | 触发条件 | 处理方式 |
|------|----------|----------|
| Level 1 | 单模型超时 | 重试 1 次（timeout/2） |
| Level 2 | 重试失败 | 使用成功的单模型结果，标记 `DEGRADED` |
| Level 3 | 双模型均失败 | 回退到主代理直接处理，标记 `FAILED` |

## 执行流程

### 快速开始（推荐）

**如果你想快速调用双模型，使用以下简化流程**：

#### 步骤 1：读取配置

```text
- 读取 CCG_BIN（默认：~/.claude/bin/codeagent-wrapper.exe）
- 读取 WORKDIR（当前工作目录绝对路径）
- 读取 LITE_MODE / GEMINI_MODEL
```

#### 步骤 2：启动 Codex 和 Gemini（并行）

```text
使用 Bash 工具分别启动两条命令（run_in_background: true）：
- Codex：echo <ROLE_FILE + TASK> | "${CCG_BIN}" --backend codex - "${WORKDIR}"
- Gemini：echo <ROLE_FILE + TASK> | "${CCG_BIN}" --backend gemini --gemini-model "${GEMINI_MODEL}" - "${WORKDIR}"

记录两个 task_id：codex_task_id / gemini_task_id
```

#### 步骤 3：等待结果（TaskOutput）

```text
使用 TaskOutput 等待，不要用 shell 级 wait/grep：
- TaskOutput({ task_id: codex_task_id, block: true, timeout: 600000 })
- TaskOutput({ task_id: gemini_task_id, block: true, timeout: 600000 })

从完整输出文本中提取 SESSION_ID：
- 正则：SESSION_ID:\s*([a-f0-9-]{36})
```

#### 步骤 4：记录结果

```text
- CODEX_SESSION：提取到则记录 UUID，否则为空
- GEMINI_SESSION：提取到则记录 UUID，否则为空
- 保留原始输出用于后续交叉验证和降级说明
```

#### 步骤 5：状态判定

```bash
# 判定状态
if [[ -n "$CODEX_SESSION" && -n "$GEMINI_SESSION" ]]; then
  STATUS="SUCCESS"
elif [[ -n "$CODEX_SESSION" || -n "$GEMINI_SESSION" ]]; then
  STATUS="DEGRADED"
else
  STATUS="FAILED"
fi

echo "最终状态: $STATUS"
```

---

### 完整流程（详细版）

**如果你需要更精细的控制，参考以下完整流程**：

### 1. 初始化阶段

```markdown
1. 读取 `.ccg/config.toml` 获取 CCG_BIN 路径
2. 读取环境变量（LITE_MODE, GEMINI_MODEL）
3. 渲染命令模板，替换占位符
4. 验证无残留占位符
```

### 2. 调用阶段

```markdown
1. 根据 `backend` 参数决定调用目标
2. 使用 Bash 工具 + `run_in_background: true` 启动进程
3. 记录 task_id 用于后续轮询
```

### 3. 等待阶段

```markdown
1. 使用 TaskOutput 轮询结果
2. 每 progress_interval 通过 zhi 汇报进度
3. 超时后触发降级策略（重试最多 3 次）
```

### 4. 结果处理阶段

```markdown
1. 提取 SESSION_ID（正则：`SESSION_ID:\s*([a-f0-9-]+)`）
2. 根据信任规则整合结果
3. 通过 zhi 展示最终输出
```

## 命令模板

### Codex 调用

```bash
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/codex/{{ROLE}}.md
<TASK>
需求：{{TASK}}
</TASK>
OUTPUT: structured-analysis' | {{CCG_BIN}} --backend codex {{LITE_MODE_FLAG}}- "{{WORKDIR}}"
```

### Gemini 调用

```bash
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/gemini/{{ROLE}}.md
<TASK>
需求：{{TASK}}
</TASK>
OUTPUT: structured-analysis' | {{CCG_BIN}} --backend gemini {{GEMINI_MODEL_FLAG}}- "{{WORKDIR}}"
```

## 进度汇报格式

通过 `mcp______zhi` 推送的进度消息：

```markdown
## collab 执行状态

| 模型 | 状态 | 耗时 |
|------|------|------|
| Codex | 🟢 运行中 | 45s |
| Gemini | 🟡 等待中 | - |

**当前阶段**：execution
**进度**：▓▓▓▓▓▓░░░░ 65%
```

## 使用示例

### 基础调用

```
/collab role=architect task="分析用户认证模块的架构设计"
```

### 指定单模型

```
/collab backend=codex role=reviewer task="审查 API 安全性"
```

### 会话复用

```
/collab role=developer task="继续实现上次的方案" resume=abc-123-def
```

## 依赖模块

- `renderer.md` - 占位符渲染
- `executor.md` - 并行调用执行
- `state-machine.md` - 状态管理
- `reporter.md` - 进度汇报

## 注意事项

1. **不要直接执行 Bash 命令**：必须通过本 Skill 封装调用
2. **SESSION_ID 必须提取**：用于后续会话复用
3. **超时不等于失败**：区分"等待超时"和"任务失败"
4. **降级时通知用户**：通过 zhi 说明降级原因
