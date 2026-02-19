/// MCP 错误处理工具模块
///
/// 提供统一的错误处理和转换功能

use rmcp::model::ErrorData as McpError;

/// MCP 错误类型枚举
#[derive(Debug, thiserror::Error)]
pub enum McpToolError {
    #[error("项目路径错误: {0}")]
    ProjectPath(String),

    #[error("弹窗创建失败: {0}")]
    PopupCreation(String),

    #[error("响应解析失败: {0}")]
    ResponseParsing(String),

    #[error("记忆管理错误: {0}")]
    Memory(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("通用错误: {0}")]
    Generic(#[from] anyhow::Error),

    // 新增网络错误分类（HC-6 统一错误分类）
    #[error("网络超时: {0}")]
    NetworkTimeout(String),

    #[error("网络连接失败: {0}")]
    NetworkConnection(String),

    #[error("认证失败: {0}")]
    AuthenticationFailed(String),

    #[error("API 限流: {0}")]
    RateLimited(String),

    #[error("外部服务不可用: {0}")]
    ServiceUnavailable(String),

    #[error("参数验证失败: {0}")]
    ValidationError(String),
}

impl McpToolError {
    /// 判断错误是否可重试
    ///
    /// 可重试的错误类型：
    /// - 网络超时
    /// - 网络连接失败
    /// - API 限流
    /// - 服务不可用
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            McpToolError::NetworkTimeout(_)
                | McpToolError::NetworkConnection(_)
                | McpToolError::RateLimited(_)
                | McpToolError::ServiceUnavailable(_)
        )
    }

    /// 判断是否应该触发降级
    ///
    /// 应降级的错误类型：
    /// - 认证失败（需要换备用服务或提示用户）
    /// - 服务不可用（需要切换到备用服务）
    pub fn should_degrade(&self) -> bool {
        matches!(
            self,
            McpToolError::AuthenticationFailed(_) | McpToolError::ServiceUnavailable(_)
        )
    }
}

impl From<McpToolError> for McpError {
    fn from(error: McpToolError) -> Self {
        match error {
            McpToolError::ProjectPath(msg) => McpError::invalid_params(msg, None),
            McpToolError::ValidationError(msg) => McpError::invalid_params(msg, None),
            McpToolError::PopupCreation(msg)
            | McpToolError::ResponseParsing(msg)
            | McpToolError::Memory(msg) => McpError::internal_error(msg, None),
            McpToolError::Io(e) => McpError::internal_error(format!("IO 错误: {}", e), None),
            McpToolError::Json(e) => McpError::internal_error(format!("JSON 错误: {}", e), None),
            McpToolError::Generic(e) => McpError::internal_error(e.to_string(), None),
            // 网络错误分类映射
            McpToolError::NetworkTimeout(msg) => {
                McpError::internal_error(format!("网络超时: {}", msg), None)
            }
            McpToolError::NetworkConnection(msg) => {
                McpError::internal_error(format!("网络连接失败: {}", msg), None)
            }
            McpToolError::AuthenticationFailed(msg) => {
                McpError::internal_error(format!("认证失败: {}", msg), None)
            }
            McpToolError::RateLimited(msg) => {
                McpError::internal_error(format!("API 限流: {}", msg), None)
            }
            McpToolError::ServiceUnavailable(msg) => {
                McpError::internal_error(format!("服务不可用: {}", msg), None)
            }
        }
    }
}

/// 创建项目路径错误
pub fn project_path_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::ProjectPath(msg.into())
}

/// 创建弹窗错误
pub fn popup_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::PopupCreation(msg.into())
}

/// 创建响应解析错误
pub fn response_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::ResponseParsing(msg.into())
}

/// 创建记忆管理错误
pub fn memory_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::Memory(msg.into())
}

/// 创建网络超时错误
pub fn network_timeout_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::NetworkTimeout(msg.into())
}

/// 创建网络连接错误
pub fn network_connection_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::NetworkConnection(msg.into())
}

/// 创建认证失败错误
pub fn authentication_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::AuthenticationFailed(msg.into())
}

/// 创建 API 限流错误
pub fn rate_limited_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::RateLimited(msg.into())
}

/// 创建服务不可用错误
pub fn service_unavailable_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::ServiceUnavailable(msg.into())
}

/// 创建参数验证错误
pub fn validation_error(msg: impl Into<String>) -> McpToolError {
    McpToolError::ValidationError(msg.into())
}
