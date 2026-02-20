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

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_rule_engine_empty_input() {
        // Arrange - 空字符串输入
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();

        // Act
        let result = enhancer.enhance("", &ctx);

        // Assert - 空输入应返回空字符串，不 panic
        assert_eq!(result, "");
    }

    #[test]
    fn test_rule_engine_long_input() {
        // Arrange - 超长输入（>1000 字符）
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let long_prompt = "fix ".repeat(300); // 1200 字符，包含触发词

        // Act
        let result = enhancer.enhance(&long_prompt, &ctx);

        // Assert - 超长输入应正常处理，不 panic，且触发了 fix 规则
        assert!(result.len() > long_prompt.len(), "应追加规则模板");
        assert!(result.contains("错误的完整堆栈"));
    }

    #[test]
    fn test_rule_engine_first_match_stops_at_highest_priority() {
        // Arrange - "fix" 触发 priority=100 的规则，"refactor" 触发 priority=90
        // FirstMatch 策略下，只应用优先级最高的第一个匹配规则
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let prompt = "fix and refactor this code"; // 同时匹配 fix(100) 和 refactor(90)

        // Act
        let result = enhancer.enhance(prompt, &ctx);

        // Assert - FirstMatch：只追加 fix 规则（priority=100），不追加 refactor 规则
        assert!(result.contains("错误的完整堆栈"), "应包含 fix 规则内容");
        assert!(!result.contains("保持现有公共 API 不变"), "不应包含 refactor 规则内容");
    }

    #[test]
    fn test_rule_engine_all_match_applies_multiple_rules() {
        // Arrange - AllMatch 策略应叠加所有匹配规则
        let enhancer = RuleEnhancer::new_default().with_all_match();
        let ctx = EnhanceContext::default();
        let prompt = "fix and refactor this code"; // 同时匹配 fix 和 refactor

        // Act
        let result = enhancer.enhance(prompt, &ctx);

        // Assert - AllMatch：fix 和 refactor 规则都应被追加
        assert!(result.contains("错误的完整堆栈"), "应包含 fix 规则内容");
        assert!(result.contains("保持现有公共 API 不变"), "应包含 refactor 规则内容");
    }

    #[test]
    fn test_rule_engine_chinese_keyword_trigger() {
        // Arrange - 中文关键词触发
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();

        // Act & Assert - 中文"修复"触发 fix 规则
        let result = enhancer.enhance("帮我修复这个问题", &ctx);
        assert!(result.contains("错误的完整堆栈"), "中文'修复'应触发 fix 规则");

        // 中文"数据库"触发 database 规则（避免使用"优化"，因其同时匹配 performance 规则）
        let result2 = enhancer.enhance("查询数据库 sql 表结构", &ctx);
        assert!(result2.contains("说明表结构和索引"), "中文'数据库'应触发 database 规则");
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_rule_engine_priority_order_security_beats_deploy() {
        // Arrange - "auth" 同时匹配 security(90) 和 api(80)
        // FirstMatch 按优先级降序排列，security(90) 应先匹配
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let prompt = "auth endpoint security"; // 匹配 security(90) 和 api(80)

        // Act
        let result = enhancer.enhance(prompt, &ctx);

        // Assert - security 规则优先级 90 > api 规则优先级 80
        assert!(result.contains("不在日志中输出敏感信息"), "security 规则应优先触发");
        assert!(!result.contains("遵循 RESTful 规范"), "api 规则不应触发（FirstMatch）");
    }

    #[test]
    fn test_rule_engine_result_starts_with_original_prompt() {
        // Arrange - 增强结果必须以原始 prompt 开头
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext::default();
        let prompt = "帮我写一个 test 用例";

        // Act
        let result = enhancer.enhance(prompt, &ctx);

        // Assert - 原始 prompt 必须保留在结果开头
        assert!(result.starts_with(prompt), "增强结果必须以原始 prompt 开头");
        assert!(result.len() > prompt.len(), "应追加了规则模板");
    }

    #[test]
    fn test_rule_engine_with_context_fields() {
        // Arrange - 提供完整的 EnhanceContext（当前实现忽略 context，但不应 panic）
        let enhancer = RuleEnhancer::new_default();
        let ctx = EnhanceContext {
            current_file: Some("src/main.rs".to_string()),
            project_root: Some("/home/user/project".to_string()),
        };

        // Act
        let result = enhancer.enhance("fix this bug", &ctx);

        // Assert - 有 context 时应正常工作
        assert!(result.contains("错误的完整堆栈"));
    }

    #[test]
    fn test_rule_engine_default_has_10_rules() {
        // Arrange & Act
        let _enhancer = RuleEnhancer::new_default();

        // Assert - 默认应有 10 条规则（通过触发不同关键词验证）
        // 验证规则数量通过覆盖所有规则类型
        let ctx = EnhanceContext::default();
        let test_cases = vec![
            ("fix bug", "错误的完整堆栈"),
            ("refactor code", "保持现有公共 API 不变"),
            ("write test", "覆盖正常路径"),
            ("update doc", "使用简体中文"),
            ("api endpoint", "遵循 RESTful 规范"),
            ("performance slow", "先测量再优化"),
            ("security auth", "不在日志中输出敏感信息"),
            ("deploy pipeline", "说明目标环境"),
            ("database query", "说明表结构和索引"),
            ("ui component", "说明目标设备和分辨率"),
        ];

        // 使用 AllMatch 策略逐一验证每条规则可触发
        let all_match_enhancer = RuleEnhancer::new_default().with_all_match();
        for (prompt, expected_content) in test_cases {
            let result = all_match_enhancer.enhance(prompt, &ctx);
            assert!(
                result.contains(expected_content),
                "规则触发失败：prompt='{}' 应包含 '{}'",
                prompt,
                expected_content
            );
        }
    }
}
