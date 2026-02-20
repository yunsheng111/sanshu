# 并行调用执行模块

collab Skill 的并行调用逻辑，负责启动 Codex 和 Gemini 进程并等待结果。

## 执行模式

### 并行模式（默认）

```markdown
1. 同时启动 Codex 和 Gemini 进程
2. 使用 `run_in_background: true` 避免阻塞
3. 并行等待两个进程完成
4. 合并结果
```

### 串行模式

```markdown
1. 先启动 Codex 进程
2. 等待 Codex 完成
3. 将 Codex 结果作为 Gemini 的输入
4. 启动 Gemini 进程
5. 等待 Gemini 完成
```

## 调用流程

### 步骤 1：渲染命令

```markdown
1. 调用 renderer.md 渲染 Codex 命令模板
2. 调用 renderer.md 渲染 Gemini 命令模板
3. 验证无残留占位符
```

### 步骤 2：启动进程

#### Codex 启动

```markdown
使用 Bash 工具：
- command: <渲染后的 Codex 命令>
- run_in_background: true
- description: "启动 Codex 后端分析"

记录返回的 task_id 为 codex_task_id
```

#### Gemini 启动

```markdown
使用 Bash 工具：
- command: <渲染后的 Gemini 命令>
- run_in_background: true
- description: "启动 Gemini 前端分析"

记录返回的 task_id 为 gemini_task_id
```

### 步骤 3：等待结果

```markdown
使用 TaskOutput 工具轮询：
- task_id: codex_task_id / gemini_task_id
- block: false（非阻塞轮询）
- timeout: 30000（单次轮询超时）

轮询策略：
- 每 5 秒检查一次状态
- 总超时由 Skill 参数 timeout 控制（默认 600000ms）
- 超时后触发降级策略
```

### 步骤 4：提取 SESSION_ID

```markdown
从输出中提取 SESSION_ID：
- 正则：`SESSION_ID:\s*([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})`（严格 UUID v4，大小写不敏感）
- 若未找到，标记为 null
- SESSION_ID 用于后续会话复用
```

## 命令模板

### Codex 命令

```bash
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/codex/{{ROLE}}.md
<TASK>
需求：{{TASK}}
</TASK>
OUTPUT: structured-analysis' | {{CCG_BIN}} --backend codex {{LITE_MODE_FLAG}}- "{{WORKDIR}}"
```

### Gemini 命令

```bash
echo 'ROLE_FILE: ~/.claude/.ccg/prompts/gemini/{{ROLE}}.md
<TASK>
需求：{{TASK}}
</TASK>
OUTPUT: structured-analysis' | {{CCG_BIN}} --backend gemini {{GEMINI_MODEL_FLAG}}- "{{WORKDIR}}"
```

## 结果结构

### 单模型结果

```json
{
  "backend": "codex" | "gemini",
  "task_id": "string",
  "session_id": "uuid | null",
  "status": "SUCCESS" | "TIMEOUT" | "FAILED",
  "output": "string",
  "duration_ms": 12345,
  "error": "string | null"
}
```

### 合并结果

```json
{
  "codex": { /* 单模型结果 */ },
  "gemini": { /* 单模型结果 */ },
  "overall_status": "SUCCESS" | "DEGRADED" | "FAILED",
  "total_duration_ms": 12345
}
```

## 超时处理

### 单模型超时

```markdown
1. 记录超时时间点
2. 继续等待另一模型
3. 标记超时模型状态为 "timeout"
4. 触发 Level 1 降级（重试）
```

### 双模型超时

```markdown
1. 记录双超时
2. 标记 overall_status 为 "failed"
3. 触发 Level 3 降级（主代理接管）
```

## 错误处理

### 进程启动失败

```markdown
- 记录错误信息
- 尝试单模型模式
- 若单模型也失败，触发 Level 3 降级
```

### 进程异常退出

```markdown
- 检查退出码
- 提取错误输出
- 根据错误类型决定是否重试
```

### 网络错误

```markdown
- 区分临时性错误和永久性错误
- 临时性错误：重试
- 永久性错误：降级
```

## 与其他模块的关系

```
renderer.md (占位符渲染)
    ↓ 渲染后的命令
executor.md (本模块)
    ↓ 执行结果
state-machine.md (状态管理)
    ↓ 状态更新
reporter.md (进度汇报)
```

## 使用示例

### 并行调用

```markdown
输入：
- backend: both
- role: architect
- task: 分析用户认证模块
- parallel: true
- timeout: 600000

执行：
1. 渲染 Codex 命令
2. 渲染 Gemini 命令
3. 并行启动两个进程
4. 并行等待结果
5. 合并结果返回
```

### 单模型调用

```markdown
输入：
- backend: codex
- role: reviewer
- task: 审查 API 安全性

执行：
1. 渲染 Codex 命令
2. 启动 Codex 进程
3. 等待结果
4. 返回单模型结果
```

## 性能考虑

### 并行效率

```markdown
- 并行模式下，总耗时 ≈ max(codex_time, gemini_time)
- 串行模式下，总耗时 ≈ codex_time + gemini_time
- 默认使用并行模式以提高效率
```

### 资源占用

```markdown
- 每个模型进程独立运行
- 内存占用取决于任务复杂度
- 建议在资源充足时使用并行模式
```
