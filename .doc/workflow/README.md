# Workflow 六阶段工作流

存放六阶段工作流（研究→构思→计划→执行→审查→验收）的产出文档。

## 目录结构

- `wip/` - 临时工作文件（自动清理 > 30 天）
  - `research/` - 阶段 1 研究中间产出
  - `analysis/` - 技术架构分析
  - `execution/` - 阶段 4 执行记录
  - `review/` - 阶段 5 审查修复记录
  - `acceptance/` - 阶段 6 验收记录
- `research/` - 阶段 1 正式研究产出
- `plans/` - 阶段 3 正式计划文件
- `reviews/` - 正式审查报告
- `progress/` - 进度追踪
- `archive/` - 已归档文档

## 命名规范

**临时文件（wip/）**：`YYYYMMDD-<topic>-<type>.md`
**正式文件**：`YYYYMMDD-<topic>-<type>.md`

## 生命周期

- 临时文件：自动清理（> 30 天）
- 正式文件：手动归档
- 进度文件：任务完成后归档
