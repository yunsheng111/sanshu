//! HC-9: API 密钥安全存储
//!
//! 使用系统凭据管理器安全存储敏感 API 密钥，
//! 避免明文存储在配置文件中。
//!
//! 支持的密钥类型：
//! - Acemcp API Token
//! - Context7 API Key
//! - Enhance API Key
//! - Sou Embedding API Key

use anyhow::{Result, anyhow};
use keyring::Entry;

/// 服务名称（用于凭据管理器中的标识）
const SERVICE_NAME: &str = "sanshu";

/// 密钥类型枚举
#[derive(Debug, Clone, Copy)]
pub enum ApiKeyType {
    /// Acemcp API Token
    AcemcpToken,
    /// Context7 API Key
    Context7ApiKey,
    /// Enhance API Key
    EnhanceApiKey,
    /// Sou Embedding API Key
    SouEmbeddingApiKey,
}

impl ApiKeyType {
    /// 获取密钥在凭据管理器中的标识符
    fn as_str(&self) -> &'static str {
        match self {
            ApiKeyType::AcemcpToken => "acemcp_token",
            ApiKeyType::Context7ApiKey => "context7_api_key",
            ApiKeyType::EnhanceApiKey => "enhance_api_key",
            ApiKeyType::SouEmbeddingApiKey => "sou_embedding_api_key",
        }
    }
}

/// 安全密钥管理器
pub struct SecureKeyStore;

impl SecureKeyStore {
    /// 存储 API 密钥到系统凭据管理器
    ///
    /// # Arguments
    /// * `key_type` - 密钥类型
    /// * `value` - 密钥值
    ///
    /// # Returns
    /// * `Ok(())` - 存储成功
    /// * `Err(...)` - 存储失败（凭据管理器不可用等）
    pub fn store(key_type: ApiKeyType, value: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, key_type.as_str())
            .map_err(|e| anyhow!("创建凭据条目失败: {}", e))?;

        entry
            .set_password(value)
            .map_err(|e| anyhow!("存储密钥失败: {}", e))?;

        Ok(())
    }

    /// 从系统凭据管理器获取 API 密钥
    ///
    /// # Arguments
    /// * `key_type` - 密钥类型
    ///
    /// # Returns
    /// * `Ok(Some(value))` - 找到密钥
    /// * `Ok(None)` - 密钥不存在
    /// * `Err(...)` - 获取失败
    pub fn retrieve(key_type: ApiKeyType) -> Result<Option<String>> {
        let entry = Entry::new(SERVICE_NAME, key_type.as_str())
            .map_err(|e| anyhow!("创建凭据条目失败: {}", e))?;

        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow!("获取密钥失败: {}", e)),
        }
    }

    /// 从系统凭据管理器删除 API 密钥
    ///
    /// # Arguments
    /// * `key_type` - 密钥类型
    ///
    /// # Returns
    /// * `Ok(true)` - 删除成功
    /// * `Ok(false)` - 密钥不存在
    /// * `Err(...)` - 删除失败
    pub fn delete(key_type: ApiKeyType) -> Result<bool> {
        let entry = Entry::new(SERVICE_NAME, key_type.as_str())
            .map_err(|e| anyhow!("创建凭据条目失败: {}", e))?;

        match entry.delete_password() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(anyhow!("删除密钥失败: {}", e)),
        }
    }

    /// 检查密钥是否存在
    ///
    /// # Arguments
    /// * `key_type` - 密钥类型
    ///
    /// # Returns
    /// * `Ok(true)` - 密钥存在
    /// * `Ok(false)` - 密钥不存在
    pub fn exists(key_type: ApiKeyType) -> Result<bool> {
        Self::retrieve(key_type).map(|opt| opt.is_some())
    }

    /// 获取密钥或从配置降级读取
    ///
    /// 优先从安全存储获取，如果不存在则从配置文件读取（兼容旧版本）
    ///
    /// # Arguments
    /// * `key_type` - 密钥类型
    /// * `fallback_from_config` - 配置文件中的值（可选）
    ///
    /// # Returns
    /// * 密钥值（可能为 None）
    pub fn get_or_fallback(
        key_type: ApiKeyType,
        fallback_from_config: Option<&str>,
    ) -> Option<String> {
        // 优先从安全存储获取
        if let Ok(Some(value)) = Self::retrieve(key_type) {
            if !value.is_empty() {
                return Some(value);
            }
        }

        // 降级到配置文件
        fallback_from_config.map(|s| s.to_string())
    }
}

/// Tauri 命令：存储 API 密钥
#[tauri::command]
pub async fn store_api_key(key_type: String, value: String) -> Result<(), String> {
    let key = parse_key_type(&key_type)?;
    SecureKeyStore::store(key, &value).map_err(|e| e.to_string())
}

/// Tauri 命令：获取 API 密钥
#[tauri::command]
pub async fn retrieve_api_key(key_type: String) -> Result<Option<String>, String> {
    let key = parse_key_type(&key_type)?;
    SecureKeyStore::retrieve(key).map_err(|e| e.to_string())
}

/// Tauri 命令：删除 API 密钥
#[tauri::command]
pub async fn delete_api_key(key_type: String) -> Result<bool, String> {
    let key = parse_key_type(&key_type)?;
    SecureKeyStore::delete(key).map_err(|e| e.to_string())
}

/// Tauri 命令：检查 API 密钥是否存在
#[tauri::command]
pub async fn api_key_exists(key_type: String) -> Result<bool, String> {
    let key = parse_key_type(&key_type)?;
    SecureKeyStore::exists(key).map_err(|e| e.to_string())
}

/// 解析密钥类型字符串
fn parse_key_type(key_type: &str) -> Result<ApiKeyType, String> {
    match key_type {
        "acemcp_token" => Ok(ApiKeyType::AcemcpToken),
        "context7_api_key" => Ok(ApiKeyType::Context7ApiKey),
        "enhance_api_key" => Ok(ApiKeyType::EnhanceApiKey),
        "sou_embedding_api_key" => Ok(ApiKeyType::SouEmbeddingApiKey),
        _ => Err(format!("未知的密钥类型: {}", key_type)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_type_as_str() {
        assert_eq!(ApiKeyType::AcemcpToken.as_str(), "acemcp_token");
        assert_eq!(ApiKeyType::Context7ApiKey.as_str(), "context7_api_key");
        assert_eq!(ApiKeyType::EnhanceApiKey.as_str(), "enhance_api_key");
        assert_eq!(
            ApiKeyType::SouEmbeddingApiKey.as_str(),
            "sou_embedding_api_key"
        );
    }

    #[test]
    fn test_parse_key_type() {
        assert!(parse_key_type("acemcp_token").is_ok());
        assert!(parse_key_type("context7_api_key").is_ok());
        assert!(parse_key_type("unknown").is_err());
    }
}
