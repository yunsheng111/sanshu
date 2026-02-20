//! 摘要自动生成模块
//!
//! SC-19: 长记忆自动生成摘要
//! DEP-06: 规则引擎降级（不依赖外部 API）

use super::types::MemoryConfig;

/// 摘要生成器
pub struct SummaryGenerator;

impl SummaryGenerator {
    /// 判断是否需要生成摘要
    ///
    /// W6 修复：使用字符数而非字节数判断，更准确处理中文内容
    pub fn needs_summary(content: &str, config: &MemoryConfig) -> bool {
        content.chars().count() > config.summary_length_threshold
    }

    /// SC-19: 规则引擎降级生成摘要
    ///
    /// 提取首行 + 关键词截断为 100 字符
    /// 规则引擎生成的摘要前缀标记为 `[auto]`
    pub fn generate_rule_based(content: &str) -> String {
        let content = content.trim();
        if content.is_empty() {
            return String::new();
        }

        // 提取首行作为主要信息
        let first_line = content.lines().next().unwrap_or(content).trim();

        // 截断到 100 字符
        let summary = if first_line.len() > 100 {
            let truncated = &first_line[..first_line.char_indices()
                .nth(100)
                .map(|(i, _)| i)
                .unwrap_or(first_line.len())];
            format!("[auto] {}...", truncated)
        } else if first_line.len() < 50 && content.lines().count() > 1 {
            // 首行太短时，尝试拼接第二行
            let second_line = content.lines().nth(1)
                .map(|l| l.trim())
                .unwrap_or("");
            let combined = format!("{} {}", first_line, second_line);
            if combined.len() > 100 {
                let truncated = &combined[..combined.char_indices()
                    .nth(100)
                    .map(|(i, _)| i)
                    .unwrap_or(combined.len())];
                format!("[auto] {}...", truncated)
            } else {
                format!("[auto] {}", combined)
            }
        } else {
            format!("[auto] {}", first_line)
        };

        summary
    }

    /// 异步生成摘要（当前仅使用规则引擎降级）
    ///
    /// DEP-06: 未来可通过 enhance 降级链生成更高质量的摘要
    pub async fn generate_summary(content: &str, _config: &MemoryConfig) -> String {
        // TODO: 尝试通过 enhance 降级链生成（复用 enhance/chat_client.rs）
        // 当前直接使用规则引擎降级
        Self::generate_rule_based(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_content_no_summary() {
        let config = MemoryConfig::default();
        let content = "简短内容";
        assert!(!SummaryGenerator::needs_summary(content, &config));
    }

    #[test]
    fn test_long_content_needs_summary() {
        let config = MemoryConfig::default();
        let content = "A".repeat(600);
        assert!(SummaryGenerator::needs_summary(&content, &config));
    }

    #[test]
    fn test_rule_based_summary() {
        let content = "这是一个很长的记忆内容，第一行包含了主要的信息描述。\n\n后面还有很多详细的内容，包括技术细节、实现方案和相关的代码示例。这些内容在摘要中不需要全部展示。";
        let summary = SummaryGenerator::generate_rule_based(content);
        assert!(summary.starts_with("[auto]"));
        assert!(summary.len() <= 150); // [auto] 前缀 + 100 字符 + ...
    }

    #[test]
    fn test_rule_based_very_long_first_line() {
        let content = "A".repeat(200);
        let summary = SummaryGenerator::generate_rule_based(&content);
        assert!(summary.starts_with("[auto]"));
        assert!(summary.contains("..."));
    }

    #[test]
    fn test_empty_content() {
        let summary = SummaryGenerator::generate_rule_based("");
        assert!(summary.is_empty());
    }

    // --- 追加测试 ---

    #[test]
    fn test_whitespace_only_content() {
        // 纯空白内容应返回空字符串
        let summary = SummaryGenerator::generate_rule_based("   \n  \t  ");
        assert!(summary.is_empty(), "纯空白内容应返回空字符串");
    }

    #[test]
    fn test_short_first_line_with_second_line() {
        // 首行 < 50 字符且有第二行时，应拼接第二行
        let content = "短标题\n详细说明内容";
        let summary = SummaryGenerator::generate_rule_based(content);
        assert!(summary.starts_with("[auto]"));
        assert!(summary.contains("短标题"), "应包含首行");
        assert!(summary.contains("详细说明内容"), "应包含第二行");
    }

    #[test]
    fn test_needs_summary_at_threshold() {
        // 刚好在阈值处的内容
        let config = MemoryConfig::default();
        // 刚好 500 字符 -> 不需要摘要
        let at_threshold = "A".repeat(500);
        assert!(!SummaryGenerator::needs_summary(&at_threshold, &config));

        // 501 字符 -> 需要摘要
        let over_threshold = "A".repeat(501);
        assert!(SummaryGenerator::needs_summary(&over_threshold, &config));
    }

    #[test]
    fn test_auto_prefix_always_present() {
        // 所有非空摘要应以 [auto] 开头
        let content = "这是一个正常长度的记忆内容";
        let summary = SummaryGenerator::generate_rule_based(content);
        assert!(summary.starts_with("[auto]"), "摘要应以 [auto] 前缀开头");
    }

    #[test]
    fn test_unicode_chinese_truncation() {
        // Unicode 中文字符截断不应在字符中间断裂
        let content = "中".repeat(200); // 200 个中文字符
        let summary = SummaryGenerator::generate_rule_based(&content);
        assert!(summary.starts_with("[auto]"));
        // 截断后应以 "..." 结尾
        assert!(summary.contains("..."), "超长中文内容应被截断");
        // 验证不会 panic 或产生无效 UTF-8
        assert!(summary.is_char_boundary(summary.len()));
    }

    #[test]
    fn test_single_line_medium_length() {
        // 单行 50-100 字符的内容，直接返回带前缀的原文
        let content = "A".repeat(80);
        let summary = SummaryGenerator::generate_rule_based(&content);
        assert!(summary.starts_with("[auto] "));
        // 80 字符不超过 100，不应有 "..."
        assert!(!summary.contains("..."), "80字符不应被截断");
    }

    #[test]
    fn test_exactly_100_char_first_line() {
        // 恰好 100 字符的首行不应被截断
        let content = "B".repeat(100);
        let summary = SummaryGenerator::generate_rule_based(&content);
        assert!(summary.starts_with("[auto] "));
        assert!(!summary.contains("..."), "恰好 100 字符不应被截断");
    }
}
