//! 记忆摘要生成服务
//!
//! 提供带有 Provider 链和超时保护的摘要生成功能：
//! - 优先尝试调用 enhance 工具（Ollama → 云端）
//! - 失败时降级使用规则引擎
//! - 超时保护（5 秒），避免阻塞写入流程

use anyhow::Result;
use std::time::Duration;
use tokio::time::timeout;

use super::summary::SummaryGenerator;
use super::types::MemoryConfig;
use crate::log_debug;

/// 摘要生成服务
///
/// 封装摘要生成的 Provider 链逻辑，支持：
/// 1. enhance 工具调用（异步，带超时）
/// 2. 规则引擎兜底
pub struct SummaryService;

/// 摘要生成结果
#[derive(Debug, Clone)]
pub struct SummaryResult {
    /// 生成的摘要文本
    pub summary: String,
    /// 使用的 Provider 类型
    pub provider: SummaryProvider,
}

/// 摘要生成 Provider 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryProvider {
    /// enhance 工具（Ollama 或云端）
    Enhance,
    /// 规则引擎（兜底）
    RuleEngine,
    /// 超时后使用规则引擎
    RuleEngineTimeout,
}

impl SummaryProvider {
    /// 获取 Provider 的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Enhance => "enhance",
            Self::RuleEngine => "rule-engine",
            Self::RuleEngineTimeout => "rule-engine (timeout)",
        }
    }
}

impl SummaryService {
    /// 默认超时时间（5 秒）
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

    /// 判断内容是否需要生成摘要
    ///
    /// 当内容长度超过配置的阈值（默认 500 字符）时返回 true
    pub fn needs_summary(content: &str, config: &MemoryConfig) -> bool {
        SummaryGenerator::needs_summary(content, config)
    }

    /// 异步生成摘要（带超时保护）
    ///
    /// Provider 链：enhance 工具 → 规则引擎兜底
    /// 超时保护：5 秒内未完成则使用规则引擎
    pub async fn generate_summary(content: &str, _config: &MemoryConfig) -> SummaryResult {
        // 尝试使用 enhance 工具生成摘要（带超时）
        let enhance_result = timeout(
            Self::DEFAULT_TIMEOUT,
            Self::try_enhance_summary(content),
        )
        .await;

        match enhance_result {
            Ok(Ok(summary)) => {
                log_debug!("[SummaryService] 使用 enhance 工具生成摘要成功");
                SummaryResult {
                    summary,
                    provider: SummaryProvider::Enhance,
                }
            }
            Ok(Err(e)) => {
                log_debug!("[SummaryService] enhance 工具失败，降级到规则引擎: {}", e);
                SummaryResult {
                    summary: SummaryGenerator::generate_rule_based(content),
                    provider: SummaryProvider::RuleEngine,
                }
            }
            Err(_) => {
                log_debug!("[SummaryService] enhance 工具超时，降级到规则引擎");
                SummaryResult {
                    summary: SummaryGenerator::generate_rule_based(content),
                    provider: SummaryProvider::RuleEngineTimeout,
                }
            }
        }
    }

    /// 同步生成摘要（仅使用规则引擎）
    ///
    /// 用于不需要异步环境的场景
    pub fn generate_summary_sync(content: &str) -> SummaryResult {
        SummaryResult {
            summary: SummaryGenerator::generate_rule_based(content),
            provider: SummaryProvider::RuleEngine,
        }
    }

    /// 尝试使用 enhance 工具生成摘要
    ///
    /// ⚠️ 当前版本：enhance 工具尚未集成，直接返回错误强制降级到规则引擎
    ///
    /// TODO: 集成 enhance 降级链（Ollama → 云端）
    /// 1. 尝试调用本地 Ollama
    /// 2. 失败时尝试云端 API
    /// 3. 构造摘要生成的 prompt
    ///
    /// 示例 prompt:
    /// "请用一句话（80-120字）概括以下内容的核心要点：\n\n{content}"
    async fn try_enhance_summary(_content: &str) -> Result<String> {
        // 当前版本：直接返回错误，强制降级到规则引擎
        Err(anyhow::anyhow!("enhance 工具尚未集成（W6: 已在文档中标注）"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_summary_short_content() {
        let config = MemoryConfig::default();
        let content = "短内容";
        assert!(!SummaryService::needs_summary(content, &config));
    }

    #[test]
    fn test_needs_summary_long_content() {
        let config = MemoryConfig::default();
        let content = "A".repeat(600);
        assert!(SummaryService::needs_summary(&content, &config));
    }

    #[test]
    fn test_generate_summary_sync() {
        let content = "这是一段需要生成摘要的长文本内容。";
        let result = SummaryService::generate_summary_sync(content);
        assert_eq!(result.provider, SummaryProvider::RuleEngine);
        assert!(result.summary.starts_with("[auto]"));
    }

    #[tokio::test]
    async fn test_generate_summary_fallback_to_rule_engine() {
        let content = "这是一段需要生成摘要的长文本内容，包含很多信息。";
        let config = MemoryConfig::default();
        let result = SummaryService::generate_summary(content, &config).await;

        // 由于 enhance 尚未集成，应降级到规则引擎
        assert!(
            result.provider == SummaryProvider::RuleEngine
                || result.provider == SummaryProvider::RuleEngineTimeout,
            "应降级到规则引擎"
        );
        assert!(result.summary.starts_with("[auto]"));
    }

    #[test]
    fn test_summary_provider_display_name() {
        assert_eq!(SummaryProvider::Enhance.display_name(), "enhance");
        assert_eq!(SummaryProvider::RuleEngine.display_name(), "rule-engine");
        assert_eq!(
            SummaryProvider::RuleEngineTimeout.display_name(),
            "rule-engine (timeout)"
        );
    }
}
