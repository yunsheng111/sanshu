# 进度汇报模块

> **⛔ 强制汇报指令**
>
> 本模块定义的汇报规范是**强制性**的，不是可选的。
> 所有通过 collab Skill 调用双模型的代理必须：
> 1. 在 Bash 后台任务启动后，**每 30 秒**通过 `mcp______zhi` 推送一次进度状态
> 2. 在单模型完成时**立即**推送该模型的输出摘要和 SESSION_ID
> 3. 在降级触发时**立即**通过 `mcp______zhi` 通知用户降级原因
> 4. 在全部完成时推送最终状态汇总
>
> 不汇报 = 用户无法感知进度 = 体验失败。

collab Skill 的进度汇报逻辑，通过 `mcp______zhi` 向用户展示双模型执行状态。

## 汇报时机

### 定时汇报

```markdown
- 默认间隔：30 秒（可通过 progress_interval 参数调整）
- 触发条件：状态为 RUNNING 且未完成
- 内容：当前状态、耗时、进度百分比
```

### 事件汇报

```markdown
触发事件：
1. 状态变化（INIT → RUNNING → SUCCESS/DEGRADED/FAILED）
2. 单模型完成
3. 降级触发
4. 超时警告（45 秒无响应）
5. 执行完成
```

## 消息格式

### 运行中状态

```markdown
## collab 执行状态

| 模型 | 状态 | 耗时 |
|------|------|------|
| Codex | 🟢 运行中 | 45s |
| Gemini | 🟡 等待中 | - |

**当前阶段**：execution
**进度**：▓▓▓▓▓▓░░░░ 65%
**预计剩余**：约 30 秒
```

### 单模型完成

```markdown
## collab 执行状态

| 模型 | 状态 | 耗时 | SESSION_ID |
|------|------|------|------------|
| Codex | ✅ 完成 | 45s | abc-123 |
| Gemini | 🟢 运行中 | 50s | - |

**当前阶段**：execution
**进度**：▓▓▓▓▓▓▓▓░░ 80%
```

### 降级通知

```markdown
## collab 执行状态 ⚠️

| 模型 | 状态 | 耗时 | SESSION_ID |
|------|------|------|------------|
| Codex | ✅ 完成 | 45s | abc-123 |
| Gemini | ⏱️ 超时 | 300s | - |

**状态**：DEGRADED（降级运行）
**原因**：Gemini 响应超时，使用 Codex 单模型结果
**影响**：前端相关分析可能不完整

---
是否继续使用降级结果？
```

### 执行完成

```markdown
## collab 执行完成 ✅

| 模型 | 状态 | 耗时 | SESSION_ID |
|------|------|------|------------|
| Codex | ✅ 完成 | 45s | abc-123 |
| Gemini | ✅ 完成 | 52s | def-456 |

**总耗时**：52 秒
**状态**：SUCCESS

### 结果摘要

**Codex（后端视角）**：
- 发现 3 个 API 设计问题
- 建议优化数据库查询

**Gemini（前端视角）**：
- 发现 2 个可访问性问题
- 建议改进表单交互
```

### 执行失败

```markdown
## collab 执行失败 ❌

| 模型 | 状态 | 耗时 | 错误 |
|------|------|------|------|
| Codex | ❌ 失败 | 10s | 连接超时 |
| Gemini | ❌ 失败 | 10s | 连接超时 |

**状态**：FAILED
**原因**：双模型均无法连接

### 建议操作

1. 检查网络连接
2. 验证 CCG_BIN 路径
3. 重试执行

---
是否由主代理直接处理？
```

## 超时警告

### 45 秒警告

```markdown
## collab 执行状态 ⚠️

| 模型 | 状态 | 耗时 |
|------|------|------|
| Codex | 🟡 响应缓慢 | 45s |
| Gemini | 🟢 运行中 | 45s |

**警告**：Codex 已 45 秒无响应
**建议**：继续等待或取消重试

---
选择操作：
```

### 选项

```json
["继续等待", "取消 Codex", "取消全部"]
```

## 进度计算

### 进度百分比

```markdown
计算公式：
- 单模型模式：(elapsed_time / timeout) * 100
- 双模型模式：(completed_models / 2) * 50 + (running_time / timeout) * 50

上限：99%（完成前不显示 100%）
```

### 进度条渲染

```markdown
格式：▓▓▓▓▓▓░░░░ {percent}%

规则：
- 10 个字符宽度
- ▓ 表示已完成
- ░ 表示未完成
- 百分比右对齐
```

## zhi 调用参数

### 运行中汇报

```json
{
  "message": "<Markdown 格式的状态消息>",
  "is_markdown": true,
  "project_root_path": "<工作目录>"
}
```

### 需要用户决策

```json
{
  "message": "<Markdown 格式的状态消息>",
  "is_markdown": true,
  "project_root_path": "<工作目录>",
  "predefined_options": ["继续等待", "使用降级结果", "取消执行"]
}
```

## 心跳机制

### 心跳检测

```markdown
- 检测间隔：10 秒
- 超时阈值：45 秒
- 超时动作：显示黄色警告
```

### 心跳恢复

```markdown
- 检测到响应后重置计时器
- 更新状态为正常
- 清除警告标记
```

## 与其他模块的关系

```
state-machine.md (状态管理)
    ↓ 状态变化事件
reporter.md (本模块)
    ↓ zhi 消息
用户界面
```

## 配置选项

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `progress_callback` | boolean | true | 是否启用进度回调 |
| `progress_interval` | number | 30000 | 汇报间隔（毫秒） |
| `warning_threshold` | number | 45000 | 超时警告阈值（毫秒） |
| `show_reasoning` | boolean | false | 是否展示推理过程 |

## 日志记录

### 关键事件日志

关键事件日志记录状态变化、降级触发、执行完成等重要节点，每条必须包含以下 5 个字段：

```markdown
必须字段：
- task_id：当前任务的唯一标识（由调用方传入或自动生成）
- backend：模型标识（codex / gemini / both）
- session_id：该模型返回的 SESSION_ID（未获取时为 null）
- duration_ms：该事件从开始到触发的耗时（毫秒）
- degraded_reason：降级原因（非降级时为 null）
```

**触发时机**：
1. 状态变化（INIT -> RUNNING -> SUCCESS/DEGRADED/FAILED）
2. 单模型完成（记录 session_id 和 duration_ms）
3. 降级触发（记录 degraded_reason）
4. 执行完成（记录最终状态和总耗时）
5. 执行失败（记录错误详情）

**日志格式示例**：
```json
{
  "type": "critical_event",
  "timestamp": "2026-02-15T10:30:00Z",
  "task_id": "spec-research-20260215",
  "backend": "codex",
  "session_id": "abc-123-def",
  "duration_ms": 45000,
  "degraded_reason": null,
  "event": "model_completed",
  "message": "Codex 分析完成"
}
```

### 轮询日志

轮询日志记录定时汇报和心跳检测的常规状态，频率高但信息密度低。

```markdown
每次汇报记录：
- 时间戳
- 汇报类型（定时/事件）
- 消息内容摘要
- 用户响应（如有）
```

**与关键事件日志的区别**：
- 轮询日志：高频、低信息密度，用于进度展示和心跳检测
- 关键事件日志：低频、高信息密度，用于审计追踪和降级诊断

### 调试日志

```markdown
开发模式下额外记录：
- zhi 调用参数
- 响应时间
- 错误信息
```
