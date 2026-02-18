# Agent Teams 并行开发工作流

存放 Agent Teams 工作流（研究→规划→并行实施→审查）的产出文档。

## 目录结构

- `wip/` - 临时工作文件（自动清理 > 30 天）
  - `research/` - team-research 过程记录
  - `planning/` - team-plan 过程记录
  - `execution/` - team-exec 执行日志
  - `review/` - team-review 过程记录
- `research/` - team-research 正式产出
- `plans/` - team-plan 正式计划
- `reviews/` - team-review 正式审查报告
- `progress/` - 进度追踪
- `archive/` - 已归档文档

## 工作流

`team-research` → `team-plan` → `team-exec` → `team-review`

## 适用场景

- 多模块并行开发（文件范围可隔离）
- 需要多 Builder 同时写码
- 需要 Codex + Gemini 交叉验证
