/// API Key 脱敏（用于日志，防止泄露）
pub fn mask_api_key(key: &str) -> String {
    match key.len() {
        0 => "(空)".to_string(),
        1..=8 => "****".to_string(),
        _ => format!("{}****{}", &key[..4], &key[key.len()-4..]),
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
}
