# OpenSpec 约束驱动开发工作流

存放 OpenSpec 工作流（初始化→约束研究→零决策规划→执行→合规审查）的产出文档。

## 目录结构

- `wip/` - 临时工作文件（自动清理 > 30 天）
  - `research/` - spec-research 过程记录
  - `planning/` - spec-plan 过程记录
  - `execution/` - spec-impl 执行日志
  - `review/` - spec-review 过程记录
- `constraints/` - spec-research 正式约束集
- `proposals/` - spec-research 正式提案
- `plans/` - spec-plan 正式计划
- `reviews/` - spec-review 审查报告
- `progress/` - 进度追踪
- `templates/` - 模板文件
- `archive/` - 已归档文档

## 工作流

`spec-init` → `spec-research` → `spec-plan` → `spec-impl` → `spec-review`

## 适用场景

- 需求复杂、约束众多
- 需要严格的合规审查
- 希望执行过程零决策（计划可直接执行）
