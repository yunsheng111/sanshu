# 状态机和门禁校验模块

collab Skill 的状态管理和门禁校验逻辑，确保执行流程的正确性和可靠性。

## 状态定义

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
| `INIT` | 初始化状态 | Skill 启动时 |
| `RUNNING` | 执行中 | 进程启动后 |
| `SUCCESS` | 成功完成 | 双模型均成功返回且均有有效 SESSION_ID |
| `DEGRADED` | 降级运行 | 单模型成功（有 SESSION_ID），另一模型失败/超时 |
| `FAILED` | 执行失败 | **双模型均失败**或**双模型均无 SESSION_ID** |

### DEGRADED 分级（degraded_level）

| 级别 | 说明 | 后续动作 |
|------|------|----------|
| `ACCEPTABLE` | 非核心维度缺失，核心目标已达成 | 标注 `missing_dimensions`，经用户确认后继续 |
| `UNACCEPTABLE` | 核心维度缺失或质量不达标 | 标注 `missing_dimensions` + 影响评估，用户介入决策 |

### 关键规则

- **双模型均无 SESSION_ID => `FAILED`**：即使有文字输出，无有效 SESSION_ID 即为 `FAILED`
- **`missing_dimensions`**：`DEGRADED` 时必须标注缺失维度（`["backend"]` 或 `["frontend"]`）
- **`degraded_reason`**：`DEGRADED` 时必须记录降级原因（超时/错误/连接失败等）

## 状态转换规则

### INIT → RUNNING

```markdown
触发条件：
- 命令渲染成功
- 至少一个进程启动成功

动作：
- 记录启动时间
- 初始化进度计数器
- 启动进度汇报定时器
```

### RUNNING → SUCCESS

```markdown
触发条件：
- 双模型均返回有效结果
- 双模型均提取到 SESSION_ID（或明确无 SESSION_ID）

动作：
- 停止进度汇报定时器
- 计算总耗时
- 合并结果
```

### RUNNING → DEGRADED

```markdown
触发条件：
- 单模型成功返回
- 另一模型超时或失败
- 重试后仍失败

动作：
- 记录降级原因
- 使用成功模型的结果
- 通过 zhi 通知用户降级情况
```

### DEGRADED → SUCCESS

```markdown
触发条件：
- 降级后任务仍可完成
- 用户确认接受降级结果

动作：
- 标记最终状态为 SUCCESS（带降级标记）
- 输出降级说明
```

### RUNNING/DEGRADED → FAILED

```markdown
触发条件：
- 双模型均失败
- 降级后仍无法完成任务
- 用户取消执行

动作：
- 停止所有进程
- 记录失败原因
- 触发 Level 3 降级（主代理接管）
```

## 门禁校验

### 启动门禁

```markdown
检查项：
1. CCG_BIN 文件存在且可执行
2. 工作目录存在且可访问
3. 网络连接可用（可选）

失败处理：
- 记录失败原因
- 直接进入 FAILED 状态
- 通过 zhi 通知用户
```

### 结果门禁

```markdown
校验逻辑（OR 执行门禁 + 质量门禁分层）：

**执行门禁（OR 逻辑）**：
- `liteMode || codexSession || geminiSession` 为真即通过执行门禁
- 双模型均无 SESSION_ID 且非 liteMode => 状态为 FAILED（非 DEGRADED）

**质量门禁（代理层判断）**：
- `status=SUCCESS`：直接进入下一阶段
- `status=DEGRADED`：必须标注 `missing_dimensions` + `degraded_level`，经 zhi 确认后才能继续
- `status=FAILED`：触发 Level 3 降级（主代理接管）或终止

校验规则：
1. 至少一个模型返回有效 SESSION_ID（liteMode 豁免此规则）
2. 输出格式符合预期
3. 无严重错误信息
4. 双模型均无 SESSION_ID 且非 liteMode => FAILED（不得标记为 DEGRADED）
```

### 超时门禁

```markdown
超时阈值：
- 单模型超时：timeout / 2（默认 300000ms）
- 总超时：timeout（默认 600000ms）

超时处理：
1. 单模型超时 → 触发 Level 1 降级（重试）
2. 重试超时 → 触发 Level 2 降级（单模型）
3. 总超时 → 触发 Level 3 降级（主代理）
```

## 降级策略

### Level 1: 重试

```markdown
触发条件：
- 单模型首次超时
- 临时性网络错误

处理方式：
1. 使用 timeout/2 重试一次
2. 记录重试日志
3. 成功则继续，失败则进入 Level 2
```

### Level 2: 单模型

```markdown
触发条件：
- Level 1 重试失败
- 单模型持续不可用

处理方式：
1. 使用成功模型的结果
2. 标记状态为 DEGRADED
3. 通过 zhi 说明降级原因
4. 继续执行后续流程
```

### Level 3: 主代理接管

```markdown
触发条件：
- 双模型均失败
- Level 2 降级后仍无法完成

处理方式：
1. 标记状态为 FAILED
2. 通过 zhi 通知用户
3. 返回控制权给主代理
4. 主代理使用内置能力完成任务
```

## 状态持久化

### 状态快照

```json
{
  "state": "RUNNING",
  "degraded_level": null,
  "missing_dimensions": [],
  "started_at": "2026-02-15T10:00:00Z",
  "codex": {
    "task_id": "abc-123",
    "status": "running",
    "session_id": null
  },
  "gemini": {
    "task_id": "def-456",
    "status": "success",
    "session_id": "uuid-789"
  },
  "retries": {
    "codex": 1,
    "gemini": 0
  },
  "degraded_reason": null
}
```

### 恢复机制

```markdown
若 Skill 执行中断：
1. 读取状态快照
2. 检查进程是否仍在运行
3. 若进程存活，继续等待
4. 若进程已终止，根据状态决定下一步
```

## 与其他模块的关系

```
executor.md (并行调用)
    ↓ 执行结果
state-machine.md (本模块)
    ↓ 状态更新
reporter.md (进度汇报)
    ↓ 用户通知
SKILL.md (主入口)
```

## 错误码定义

| 错误码 | 说明 | 处理建议 |
|--------|------|----------|
| `E001` | CCG_BIN 不存在 | 检查安装路径 |
| `E002` | 命令渲染失败 | 检查配置文件 |
| `E003` | 进程启动失败 | 检查系统资源 |
| `E004` | 单模型超时 | 自动重试 |
| `E005` | 双模型超时 | 降级到主代理 |
| `E006` | SESSION_ID 提取失败 | 检查输出格式 |
| `E007` | 用户取消 | 正常终止 |
