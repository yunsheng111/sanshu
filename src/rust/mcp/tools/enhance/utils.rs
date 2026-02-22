/// API Key 脱敏（用于日志，防止泄露）
/// 安全：使用 char 边界切片，避免 Unicode 多字节字符导致 panic
pub fn mask_api_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    match chars.len() {
        0 => "(空)".to_string(),
        1..=8 => "****".to_string(),
        _ => {
            let prefix: String = chars[..4].iter().collect();
            let suffix: String = chars[chars.len()-4..].iter().collect();
            format!("{}****{}", prefix, suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_api_key_empty() {
        assert_eq!(mask_api_key(""), "(空)");
    }

    #[test]
    fn test_mask_api_key_short() {
        assert_eq!(mask_api_key("abc"), "****");
    }

    #[test]
    fn test_mask_api_key_normal() {
        let result = mask_api_key("sk-abcdefgh1234");
        assert!(result.starts_with("sk-a"));
        assert!(result.ends_with("1234"));
        assert!(result.contains("****"));
    }

    // ========================================================================
    // 安全测试
    // ========================================================================

    /// 安全：掩码后不应包含完整密钥
    #[test]
    fn test_security_mask_never_leaks_full_key() {
        let keys = [
            "sk-1234567890abcdef",
            "gsk_very_long_api_key_that_should_be_masked_properly",
            "short",
            "12345678",  // 恰好 8 字符边界
            "123456789", // 9 字符，刚好显示首尾
        ];
        for key in keys {
            let masked = mask_api_key(key);
            if key.len() > 8 {
                // 长密钥：掩码结果不应等于原始密钥
                assert_ne!(masked, key, "掩码后不应等于原始密钥: {}", key);
                // 掩码结果应包含 ****
                assert!(masked.contains("****"), "掩码结果应包含 ****: {}", masked);
            }
        }
    }

    /// 安全：8 字符边界值应完全掩码
    #[test]
    fn test_security_mask_boundary_8_chars() {
        assert_eq!(mask_api_key("12345678"), "****");
        assert_eq!(mask_api_key("123456789"), "1234****6789");
    }

    /// 安全：含特殊字符的密钥不应导致 panic
    #[test]
    fn test_security_mask_special_chars() {
        let special_keys = [
            "sk-abc\n\r\t123456",     // 控制字符
            "sk-abc\0def1234",         // null 字节
            "sk-🔐密钥🔑abcd",        // Unicode + emoji
            "sk-<script>alert(1)</script>", // XSS payload
        ];
        for key in special_keys {
            // 不应 panic
            let _ = mask_api_key(key);
        }
    }
}
