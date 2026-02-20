//! URI 路径解析和验证模块
//!
//! HC-14: URI 格式 `domain://path/segments`
//! domain 限定为 [a-z][a-z0-9_-]*
//! path segments 不限字符集（支持中文）

use anyhow::Result;
use regex::Regex;
use once_cell::sync::Lazy;

/// URI 路径正则
static URI_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([a-z][a-z0-9_-]*)://(.+)$").unwrap()
});

/// 域名正则
static DOMAIN_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9_-]*$").unwrap()
});

/// 解析后的 URI 路径
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUriPath {
    /// 域名（如 "core", "project"）
    pub domain: String,
    /// 路径段列表（如 ["architecture", "backend"]）
    pub segments: Vec<String>,
    /// 完整路径字符串（如 "core://architecture/backend"）
    pub full_path: String,
}

/// URI 路径解析器
pub struct UriPathParser;

impl UriPathParser {
    /// HC-14: 解析 URI 路径
    ///
    /// 输入格式：`domain://path/segments`
    /// 返回解析后的 ParsedUriPath
    /// 无效格式返回错误
    pub fn parse(uri: &str) -> Result<ParsedUriPath> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err(anyhow::anyhow!("URI 路径不能为空"));
        }

        let caps = URI_PATTERN.captures(uri)
            .ok_or_else(|| anyhow::anyhow!(
                "无效的 URI 路径格式: '{}'\n期望格式: domain://path/segments\n域名规则: [a-z][a-z0-9_-]*",
                uri
            ))?;

        let domain = caps.get(1).unwrap().as_str().to_string();
        let path_str = caps.get(2).unwrap().as_str();

        let segments: Vec<String> = path_str
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if segments.is_empty() {
            return Err(anyhow::anyhow!("URI 路径至少需要一个路径段: '{}'", uri));
        }

        Ok(ParsedUriPath {
            domain,
            segments,
            full_path: uri.to_string(),
        })
    }

    /// 验证域名格式
    pub fn validate_domain(domain: &str) -> bool {
        DOMAIN_PATTERN.is_match(domain)
    }

    /// HC-19 + RISK-08: 为无 URI 路径的旧记忆生成默认路径
    pub fn default_legacy_path() -> String {
        "legacy://uncategorized".to_string()
    }

    /// 从 domain 和 segments 构建完整 URI 路径
    pub fn build(domain: &str, segments: &[&str]) -> String {
        format!("{}://{}", domain, segments.join("/"))
    }

    /// 提取域名（从完整 URI 路径或单独的域名字符串）
    pub fn extract_domain(uri_or_domain: &str) -> Option<String> {
        if let Ok(parsed) = Self::parse(uri_or_domain) {
            Some(parsed.domain)
        } else if Self::validate_domain(uri_or_domain) {
            Some(uri_or_domain.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_uri() {
        let parsed = UriPathParser::parse("core://architecture/backend").unwrap();
        assert_eq!(parsed.domain, "core");
        assert_eq!(parsed.segments, vec!["architecture", "backend"]);
    }

    #[test]
    fn test_parse_chinese_path() {
        let parsed = UriPathParser::parse("project://三术/记忆模块").unwrap();
        assert_eq!(parsed.domain, "project");
        assert_eq!(parsed.segments, vec!["三术", "记忆模块"]);
    }

    #[test]
    fn test_parse_invalid_domain() {
        assert!(UriPathParser::parse("123://path").is_err());
        assert!(UriPathParser::parse("UPPER://path").is_err());
    }

    #[test]
    fn test_parse_empty() {
        assert!(UriPathParser::parse("").is_err());
    }

    #[test]
    fn test_validate_domain() {
        assert!(UriPathParser::validate_domain("core"));
        assert!(UriPathParser::validate_domain("my-project"));
        assert!(UriPathParser::validate_domain("project_1"));
        assert!(!UriPathParser::validate_domain("123"));
        assert!(!UriPathParser::validate_domain("UPPER"));
        assert!(!UriPathParser::validate_domain(""));
    }

    // --- 追加测试：多级路径解析 ---

    #[test]
    fn test_parse_multi_level_path() {
        // 多级路径 a/b/c/d 应解析为 4 个 segments
        let parsed = UriPathParser::parse("core://a/b/c/d").unwrap();
        assert_eq!(parsed.domain, "core");
        assert_eq!(parsed.segments, vec!["a", "b", "c", "d"]);
        assert_eq!(parsed.full_path, "core://a/b/c/d");
    }

    #[test]
    fn test_parse_single_segment() {
        // 单段路径
        let parsed = UriPathParser::parse("session://current").unwrap();
        assert_eq!(parsed.domain, "session");
        assert_eq!(parsed.segments, vec!["current"]);
    }

    #[test]
    fn test_parse_special_characters_in_path() {
        // 路径段中包含特殊字符（下划线、连字符、数字）
        let parsed = UriPathParser::parse("project://src-code/module_v2/test-123").unwrap();
        assert_eq!(parsed.segments, vec!["src-code", "module_v2", "test-123"]);
    }

    #[test]
    fn test_parse_whitespace_trimmed() {
        // 带空白前后缀的 URI 应被 trim
        let parsed = UriPathParser::parse("  core://path/test  ").unwrap();
        assert_eq!(parsed.domain, "core");
        assert_eq!(parsed.segments, vec!["path", "test"]);
    }

    // --- build() 函数验证 ---

    #[test]
    fn test_build_uri() {
        let uri = UriPathParser::build("core", &["architecture", "backend"]);
        assert_eq!(uri, "core://architecture/backend");
    }

    #[test]
    fn test_build_single_segment() {
        let uri = UriPathParser::build("session", &["current"]);
        assert_eq!(uri, "session://current");
    }

    #[test]
    fn test_build_empty_segments() {
        let uri = UriPathParser::build("core", &[]);
        assert_eq!(uri, "core://");
    }

    // --- extract_domain() 验证 ---

    #[test]
    fn test_extract_domain_from_uri() {
        let domain = UriPathParser::extract_domain("core://architecture/backend");
        assert_eq!(domain, Some("core".to_string()));
    }

    #[test]
    fn test_extract_domain_bare() {
        // 传入纯域名字符串
        let domain = UriPathParser::extract_domain("project");
        assert_eq!(domain, Some("project".to_string()));
    }

    #[test]
    fn test_extract_domain_invalid() {
        // 无效域名
        let domain = UriPathParser::extract_domain("123invalid");
        assert_eq!(domain, None);
    }

    #[test]
    fn test_extract_domain_uppercase_invalid() {
        // 大写域名无效
        let domain = UriPathParser::extract_domain("UPPER");
        assert_eq!(domain, None);
    }

    // --- 异常路径 ---

    #[test]
    fn test_parse_no_path_after_scheme() {
        // domain:// 后面为空
        assert!(UriPathParser::parse("core://").is_err());
    }

    #[test]
    fn test_parse_missing_scheme() {
        // 缺少 :// 分隔符
        assert!(UriPathParser::parse("core/path/test").is_err());
    }

    #[test]
    fn test_parse_domain_with_special_start() {
        // 域名不能以数字或特殊字符开头
        assert!(UriPathParser::parse("_invalid://path").is_err());
        assert!(UriPathParser::parse("-invalid://path").is_err());
    }

    // --- 边界条件 ---

    #[test]
    fn test_default_legacy_path() {
        let path = UriPathParser::default_legacy_path();
        assert_eq!(path, "legacy://uncategorized");
        // 验证生成的默认路径可被正确解析
        let parsed = UriPathParser::parse(&path).unwrap();
        assert_eq!(parsed.domain, "legacy");
        assert_eq!(parsed.segments, vec!["uncategorized"]);
    }

    #[test]
    fn test_validate_domain_with_hyphens_and_underscores() {
        assert!(UriPathParser::validate_domain("my-project"));
        assert!(UriPathParser::validate_domain("my_project"));
        assert!(UriPathParser::validate_domain("a"));
        assert!(UriPathParser::validate_domain("a1-b2_c3"));
    }
}
