# 双模型调用门禁模板 v1.0.0

> 本模板供所有标记为 `template: multi-model` 的代理在双模型调用阶段引用。
> 引用方式：在代理文档的双模型阶段写 `引用 _templates/multi-model-gate.md`，然后执行以下全部步骤。

## ⛔ 硬门禁（最高优先级）

**本阶段优先通过 collab Skill 调用 Codex 和/或 Gemini；若 `skills/collab/SKILL.md` 缺失，必须切换到 codeagent-wrapper 兼容模式继续执行。**

### 禁止行为

- **禁止**由本代理（Claude）自行分析替代双模型调用
- **禁止**编造 SESSION_ID 或伪造模型输出
- **禁止**在 collab Skill 缺失时直接中断；必须按兼容模式执行
- **禁止**跳过进度汇报步骤

### 进入下一阶段前的自检

以下条件必须全部满足，否则禁止进入下一阶段：

1. ✅ 已完成调用模式判定（collab / 兼容模式)
2. ✅ collab 模式下已读取 SKILL.md + executor.md + renderer.md；兼容模式下已读取 `.doc/standards-agent/model-calling.md` 对应章节
3. ✅ 至少执行了 1 次 Bash 命令调用 codeagent-wrapper
4. ✅ 从 Bash 输出中提取到了真实的 SESSION_ID（UUID 格式）
5. ✅ 获得了外部模型的实际输出文本
6. ✅ 按 reporter.md 规范通过 zhi 向用户汇报了进度

## 执行步骤

### 步骤 0：调用模式判定（强制）

```markdown
先检查 `skills/collab/SKILL.md`：

A) 文件存在（collab 模式）
1. Read("~/.claude/skills/collab/SKILL.md")
2. Read("~/.claude/skills/collab/executor.md")
3. Read("~/.claude/skills/collab/renderer.md")
4. Read("~/.claude/skills/collab/reporter.md")

B) 文件缺失（兼容模式）
1. Read(".doc/standards-agent/model-calling.md")
2. 按“collab 缺失兼容模式（来自 ccg-workflow）”使用 wrapper 模板调用
3. 保持同一门禁与 SESSION_ID 提取规则

**[Ledger Event - 强制]** 上报 `docs_read` 事件：
- **状态**：SUCCESS
- **payload**：`{ "mode": "collab|wrapper", "docs": [...] }`
- 此事件上报为强制要求，不可跳过
```

### 步骤 1：初始化

```markdown
1. 读取 `.ccg/config.toml` 获取 CCG_BIN 路径（默认：`~/.claude/bin/codeagent-wrapper.exe`）
2. 检查环境变量：LITE_MODE、GEMINI_MODEL
3. 若 LITE_MODE=true，跳过外部模型调用，使用占位符响应（标注为 LITE 模式）
```

### 步骤 2：渲染并执行 Codex 命令

```markdown
1. 按 renderer.md 渲染命令模板，替换所有占位符
2. 验证无残留占位符（{{...}}）
3. 使用 Bash 工具执行（run_in_background: true）
4. 记录返回的 task_id

**[Ledger Event - 强制]** 上报 `model_called` 事件：
- **状态**：SUCCESS（命令已提交）/ FAILED（执行失败）
- **payload**：`{ "backend": "codex", "task_id": "<task_id>" }`
```

### 步骤 3：渲染并执行 Gemini 命令

```markdown
1. 按 renderer.md 渲染命令模板，替换所有占位符
2. 验证无残留占位符（{{...}}）
3. 使用 Bash 工具执行（run_in_background: true）
4. 记录返回的 task_id

**[Ledger Event - 强制]** 上报 `model_called` 事件：
- **状态**：SUCCESS（命令已提交）/ FAILED（执行失败）
- **payload**：`{ "backend": "gemini", "task_id": "<task_id>" }`
```

### 步骤 4：等待结果 + 进度汇报

```markdown
1. 使用 TaskOutput 轮询两个进程：
   TaskOutput({ task_id: "<codex_task_id>", block: true, timeout: 600000 })
   TaskOutput({ task_id: "<gemini_task_id>", block: true, timeout: 600000 })

2. **超时处理（重要）**：
   - 首次超时：继续轮询，最多重试 3 次（每次 600000ms = 10分钟）
   - 每次超时后通过 mcp______zhi 通知用户："模型响应超时，继续等待..."
   - 3 次超时后：标记该模型为失败，触发降级策略
   - **关键**：超时不等于失败，继续轮询，不要 Kill 进程

3. **进度汇报（每 30 秒）**：
   - 按 reporter.md 格式通过 mcp______zhi 推送进度状态
   - 显示：模型状态、已耗时、预计剩余时间

4. **SESSION_ID 提取（关键）**：
   - 从**完整输出**中提取 SESSION_ID（正则：`SESSION_ID:\s*([a-f0-9-]+)`）
   - **确保扫描整个输出**，不要截断（使用 TaskOutput 获取完整输出）
   - 如果未找到，尝试备用正则：`session[_-]?id[:\s]*([a-f0-9-]+)`（不区分大小写）
   - 仍未找到：记录完整输出到日志，标记为失败

5. 模型返回后立即通过 zhi 推送输出摘要和 SESSION_ID

**[Ledger Event - 强制]** 每成功提取一个 SESSION_ID 后上报 `session_captured` 事件：
- **状态**：SUCCESS（提取到有效 SESSION_ID）/ FAILED（未提取到）
- **payload**：`{ "backend": "codex|gemini", "session_id": "<SESSION_ID>" }`
```

### 步骤 5：门禁校验

```markdown
**执行门禁（OR 逻辑）** — 输入仅为 status + sessions：
- LITE_MODE=true（豁免）
- codex_session 存在（Codex 成功）
- gemini_session 存在（Gemini 成功）

**状态判定**：
- codex_session && gemini_session => status = SUCCESS
- codex_session || gemini_session => status = DEGRADED
  - 标注 degraded_level（ACCEPTABLE / UNACCEPTABLE）
  - 标注 missing_dimensions（["backend"] 或 ["frontend"]）
- 无任何 SESSION_ID => status = FAILED（**禁止标记为 DEGRADED**）

**关键规则：无 SESSION_ID 的 DEGRADED 是被禁止的**
- 即使模型有文字输出，若无有效 SESSION_ID，状态必须为 FAILED
- 无 SESSION_ID -> FAILED，这是不可违反的硬规则

**DEGRADED 产出前置动作**（status=DEGRADED 时必须执行）：
1. 标注缺失维度（missing_dimensions）
2. 评估风险影响（缺失维度对当前任务的影响程度）
3. 将缺失维度相关约束转为风险约束
4. 通过 mcp______zhi 向用户展示降级详情并确认是否继续
5. 仅在用户确认后才能进入下一阶段

**[Ledger Event - 强制]** status=DEGRADED 时必须上报 `degraded` 事件：
- **状态**：DEGRADED
- **payload**：`{ "degraded_level": "ACCEPTABLE|UNACCEPTABLE", "missing_dimensions": [...], "risk_impact": "..." }`
- DEGRADED 状态不上报此事件视为违规，Hook 将拦截后续写入

若门禁失败（双模型均未返回 SESSION_ID => FAILED）：
1. 重试 1 次（Level 1 降级）
2. 重试失败 → 使用单模型结果（Level 2 降级）
3. 单模型也失败 → 通过 mcp______zhi 报告失败（Level 3 降级）

降级时必须通过 zhi 通知用户，说明降级原因和影响。
```

## 适用代理

以下代理必须在双模型调用阶段引用本模板：

| 代理 | 双模型阶段 |
|------|-----------|
| `analyze-agent` | 阶段 3 |
| `fullstack-agent` | 阶段 2/3/5 |
| `planner` | 阶段 0.3/0.5 |
| `execute-agent` | 阶段 3/5 |
| `fullstack-light-agent` | 全栈场景 |
| `team-plan-agent` | 阶段 2 |
| `team-review-agent` | 阶段 2 |
| `team-research-agent` | 阶段 2 |
| `spec-research-agent` | 阶段 2 |
| `spec-plan-agent` | 阶段 2 |
| `spec-review-agent` | 阶段 2 |
| `spec-impl-agent` | 阶段 3 |

## 错误处理

### Codex 调用失败

```markdown
1. 检查日志：查看最近的 codeagent-wrapper 日志
   - Windows: `C:\Users\Administrator\AppData\Local\Temp\codeagent-wrapper-*.log`
   - Unix: `/tmp/codeagent-wrapper-*.log`
2. 重试 1 次（使用相同参数）
3. 仍失败 → 标记为 DEGRADED，使用 Gemini 结果
4. 通过 mcp______zhi 通知用户降级原因
```

### Gemini 调用失败

```markdown
1. 检查日志（同上）
2. 重试 1 次
3. 仍失败 → 标记为 DEGRADED，使用 Codex 结果
4. 通过 mcp______zhi 通知用户降级原因
```

### SESSION_ID 提取失败

```markdown
1. 检查输出是否完整（不要截断）
2. 尝试备用正则：`session[_-]?id[:\s]*([a-f0-9-]+)`（不区分大小写）
3. 手动搜索：在输出中查找包含 "session" 的行
4. 记录完整输出到日志文件
5. 标记为失败，触发降级
```

### 双模型均失败

```markdown
1. 检查 LITE_MODE 是否为 true（如果是，应该跳过外部模型调用）
2. 检查 codeagent-wrapper 是否可执行
3. 检查网络连接
4. 通过 mcp______zhi 询问用户：
   - 选项：["重试双模型", "主代理接管", "终止任务"]
5. 根据用户选择执行相应操作
```

---

## 运行时安全网

Layer 3 保障：`hooks/ccg-dual-model-validator.cjs` 在 Write 工具写入研究产出目录时验证 SESSION_ID 存在性。即使代理绕过了本模板的提示词约束，hook 也会拦截缺少 SESSION_ID 的报告写入。
