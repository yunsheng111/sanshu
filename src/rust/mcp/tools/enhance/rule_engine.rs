// 规则引擎降级模块
// 无 API 时通过内置规则对 prompt 进行结构化增强

use regex::Regex;

/// 规则匹配策略
#[derive(Debug, Clone)]
pub enum RuleMatchStrategy {
    FirstMatch, // 首次匹配后停止（默认，避免规则冲突）
    AllMatch,   // 全部匹配（叠加应用）
}

/// 增强上下文（可选，用于规则模板变量替换）
#[derive(Debug, Clone, Default)]
pub struct EnhanceContext {
    /// 当前文件路径
    pub current_file: Option<String>,
    /// 项目根路径
    pub project_root: Option<String>,
}

/// 单条增强规则
pub struct EnhanceRule {
    pub trigger: Regex,
    pub template: String,
    pub priority: u32,
}

/// 规则增强器
pub struct RuleEnhancer {
    rules: Vec<EnhanceRule>,
    strategy: RuleMatchStrategy,
}

impl RuleEnhancer {
    /// 创建带内置 10 条规则的默认增强器
    pub fn new_default() -> Self {
        let raw_rules: &[(&str, &str, u32)] = &[
            // (触发正则, 追加模板, 优先级)
            (r"(?i)fix|bug|error|错误|修复|问题",
             "\n\n请提供：\n1. 错误的完整堆栈或日志\n2. 期望行为 vs 实际行为\n3. 复现步骤",
             100),
            (r"(?i)refactor|重构|优化结构",
             "\n\n重构约束：\n1. 保持现有公共 API 不变\n2. 不引入新的外部依赖\n3. 每次重构步骤可独立验证",
             90),
            (r"(?i)test|测试|单元测试|集成测试",
             "\n\n测试要求：\n1. 覆盖正常路径、边界条件、异常场景\n2. 测试名称描述行为而非实现\n3. 每个测试只验证一个行为",
             90),
            (r"(?i)doc|文档|注释|readme",
             "\n\n文档格式：\n1. 使用简体中文\n2. 包含用法示例\n3. 说明参数类型和返回值",
             80),
            (r"(?i)api|接口|endpoint|路由",
             "\n\n API 设计要求：\n1. 遵循 RESTful 规范\n2. 明确请求/响应格式\n3. 包含错误码说明",
             80),
            (r"(?i)performance|性能|优化|slow|慢",
             "\n\n性能优化要求：\n1. 先测量再优化（提供基准数据）\n2. 说明优化前后的对比\n3. 避免过早优化",
             80),
            (r"(?i)security|安全|auth|认证|授权",
             "\n\n安全要求：\n1. 不在日志中输出敏感信息\n2. 输入验证和边界检查\n3. 遵循最小权限原则",
             90),
            (r"(?i)deploy|部署|ci|cd|pipeline",
             "\n\n部署要求：\n1. 说明目标环境（dev/staging/prod）\n2. 包含回滚方案\n3. 列出环境变量依赖",
             70),
            (r"(?i)database|数据库|sql|query|查询",
             "\n\n数据库要求：\n1. 说明表结构和索引\n2. 考虑事务和并发安全\n3. 避免 N+1 查询",
             80),
            (r"(?i)ui|界面|component|组件|frontend|前端",
             "\n\n前端要求：\n1. 说明目标设备和分辨率\n2. 考虑无障碍访问（a11y）\n3. 提供交互状态说明",
             70),
        ];

        let mut rules: Vec<EnhanceRule> = raw_rules
            .iter()
            .filter_map(|(pattern, template, priority)| {
                Regex::new(pattern).ok().map(|trigger| EnhanceRule {
                    trigger,
                    template: template.to_string(),
                    priority: *priority,
                })
            })
            .collect();

        // 按优先级降序排列
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        Self {
            rules,
            strategy: RuleMatchStrategy::FirstMatch,
        }
    }

    /// 使用 AllMatch 策略（叠加所有匹配规则）
    pub fn with_all_match(mut self) -> Self {
        self.strategy = RuleMatchStrategy::AllMatch;
        self
    }

    /// 对 prompt 应用规则增强
    pub fn enhance(&self, prompt: &str, _context: &EnhanceContext) -> String {
        let mut result = prompt.to_string();

        match self.strategy {
            RuleMatchStrategy::FirstMatch => {
                for rule in &self.rules {
                    if rule.trigger.is_match(prompt) {
                        result.push_str(&rule.template);
                        break;
                    }
                }
            }
            RuleMatchStrategy::AllMatch => {
                for rule in &self.rules {
                    if rule.trigger.is_match(prompt) {
                        result.push_str(&rule.template);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_engine_fix() {
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let result = enhancer.enhance("帮我 fix 这个 bug", &ctx);
        assert!(result.contains("错误的完整堆栈"));
    }

    #[test]
    fn test_rule_engine_no_match() {
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let prompt = "你好，今天天气怎么样";
        let result = enhancer.enhance(prompt, &ctx);
        assert_eq!(result, prompt);
    }
}
