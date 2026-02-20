use anyhow::Result;
use rmcp::model::{ErrorData as McpError, CallToolResult};

use crate::mcp::{ZhiRequest, PopupRequest};
use crate::mcp::handlers::{create_tauri_popup, parse_mcp_response};
use crate::mcp::utils::{generate_request_id, popup_error};
use crate::mcp::utils::safe_truncate_clean;
use crate::{log_important, log_debug};

/// 智能代码审查交互工具
///
/// 支持预定义选项、自由文本输入和图片上传
#[derive(Clone)]
pub struct InteractionTool;

impl InteractionTool {
    pub async fn zhi(
        request: ZhiRequest,
    ) -> Result<CallToolResult, McpError> {
        // 默认生成 request_id（MCP server 会优先使用其 call_id 注入到 zhi_with_request_id）
        let request_id = generate_request_id();
        Self::zhi_with_request_id(request, request_id).await
    }

    /// 带 request_id 的 zhi 调用入口
    ///
    /// 中文说明：用于将 MCP 分发层生成的 call_id 贯穿到 GUI 进程与响应，便于全链路日志关联。
    pub async fn zhi_with_request_id(
        request: ZhiRequest,
        request_id: String,
    ) -> Result<CallToolResult, McpError> {
        // 记录 UI/UX 上下文控制信号，便于审计排查
        if request.uiux_intent.is_some() || request.uiux_context_policy.is_some() || request.uiux_reason.is_some() {
            log::info!(
                "UI/UX 上下文信号: intent={:?}, policy={:?}, reason={:?}",
                request.uiux_intent.as_deref(),
                request.uiux_context_policy.as_deref(),
                request.uiux_reason.as_deref()
            );
        }

        log_important!(
            info,
            "[zhi] 弹窗请求: request_id={}, message_len={}, message_preview={}, options_len={}, project={:?}",
            request_id,
            request.message.len(),
            safe_truncate_clean(&request.message, 200),
            request.predefined_options.len(),
            request.project_root_path.as_deref()
        );

        let popup_request = PopupRequest {
            id: request_id.clone(),
            message: request.message,
            predefined_options: if request.predefined_options.is_empty() {
                None
            } else {
                Some(request.predefined_options)
            },
            is_markdown: request.is_markdown,
            project_root_path: request.project_root_path,
            // 透传 UI/UX 上下文控制信号
            uiux_intent: request.uiux_intent,
            uiux_context_policy: request.uiux_context_policy,
            uiux_reason: request.uiux_reason,
        };

        match create_tauri_popup(&popup_request) {
            Ok(response) => {
                log_debug!(
                    "[zhi] 弹窗响应已收到: request_id={}, response_len={}",
                    request_id,
                    response.len()
                );
                // 解析响应内容，支持文本和图片
                let content = parse_mcp_response(&response)?;
                Ok(CallToolResult::success(content))
            }
            Err(e) => {
                log_important!(warn, "[zhi] 弹窗失败: request_id={}, error={}", request_id, e);
                Err(popup_error(e.to_string()).into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_zhi_request_validation() {
        // 测试 zhi 请求参数验证
        // 空消息应返回错误
    }

    #[test]
    fn test_zhi_history_recording() {
        // 测试历史记录功能
        // 注意：需要 mock 文件系统
    }

    #[test]
    fn test_parse_mcp_response() {
        // 测试响应解析
        let _response = r#"{"selected_option": "选项1"}"#;
        // 验证解析结果
    }
}
