// 提示词增强核心逻辑
// 支持 Augment chat-stream API / Ollama / OpenAI 兼容 / 规则引擎

use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::atomic::Ordering;
use anyhow::Result;
use reqwest::{Client, header::{AUTHORIZATION, CONTENT_TYPE}};
use serde_json::json;
use regex::Regex;
use futures_util::StreamExt;

use super::types::*;
use super::history::ChatHistoryManager;
use super::utils::mask_api_key;
use super::chat_client::{ChatClient, ChatProvider, Message};
use super::rule_engine::{RuleEnhancer, EnhanceContext};
use crate::mcp::tools::interaction::ZhiHistoryManager;
use crate::mcp::tools::acemcp::mcp::ProjectsFile;
use crate::{log_debug, log_important};

/// 增强系统提示词模板
const ENHANCE_SYSTEM_PROMPT: &str = r#"⚠️ NO TOOLS ALLOWED ⚠️

Here is an instruction that I'd like to give you, but it needs to be improved. Rewrite and enhance this instruction to make it clearer, more specific, less ambiguous, and correct any mistakes. Do not use any tools: reply immediately with your answer, even if you're not sure. Consider the context of our conversation history when enhancing the prompt. If there is code in triple backticks (```) consider whether it is a code sample and should remain unchanged.Reply with the following format:

### BEGIN RESPONSE ###
Here is an enhanced version of the original instruction that is more specific and clear:
<augment-enhanced-prompt>enhanced prompt goes here</augment-enhanced-prompt>

### END RESPONSE ###

Here is my original instruction:

"#;

/// zhi 历史摘要最大条数
const MAX_ZHI_HISTORY_ENTRIES: usize = 5;
/// 单条摘要最大字符数（避免提示词过长）
const MAX_ZHI_HISTORY_TEXT_LEN: usize = 200;
/// 历史兜底文本最大字符数（避免重复注入导致提示词过长）
const MAX_FALLBACK_HISTORY_TEXT_LEN: usize = 500;

#[derive(Debug, Clone, Default)]
struct HistoryBuildDiagnostics {
    /// 实际从磁盘加载到的历史条数（不包含兜底）
    loaded_count: usize,
    /// 历史加载失败原因（用于区分“空/失败”）
    load_error: Option<String>,
    /// 是否启用了“历史为空兜底”（即使 loaded_count 为 0，也会提供临时上下文）
    fallback_used: bool,
}

struct BuildPayloadResult {
    payload: serde_json::Value,
    history_diag: HistoryBuildDiagnostics,
}

/// 提示词增强器
pub struct PromptEnhancer {
    /// 统一 Chat 客户端列表（支持 fallback 降级链）
    chat_clients: Vec<ChatClient>,
    /// Augment API 基础 URL（兼容旧路径）
    base_url: String,
    /// API Token（兼容旧路径）
    token: String,
    /// HTTP 客户端（兼容旧路径）
    client: Client,
    /// 项目根路径
    project_root: Option<String>,
}

impl PromptEnhancer {
    /// 中文注释：清理 Windows 长路径前缀并统一为正斜杠，用于匹配/展示
    fn clean_path_prefix_and_slashes(path: &str) -> String {
        let mut p = path.trim().to_string();

        // 处理 Windows 扩展路径语法：\\?\C:\... 或 \\?\UNC\server\share\...
        if p.starts_with("\\\\?\\UNC\\") {
            // \\?\UNC\server\share\path -> \\server\share\path
            p = format!("\\\\{}", &p[8..]);
        } else if p.starts_with("\\\\?\\") {
            p = p[4..].to_string();
        }

        // 统一使用正斜杠
        p = p.replace('\\', "/");

        // 再处理 //?/（canonicalize/序列化后可能出现）
        if p.starts_with("//?/UNC/") {
            // //?/UNC/server/share/path -> //server/share/path
            p = format!("//{}", &p[8..]);
        } else if p.starts_with("//?/") {
            p = p[4..].to_string();
        }

        // 去除末尾斜杠，避免匹配与显示误差
        p.trim_end_matches('/').to_string()
    }

    /// 创建增强器实例
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self {
            chat_clients: Vec::new(),
            base_url: normalize_base_url(base_url),
            token: token.to_string(),
            client,
            project_root: None,
        })
    }

    /// 创建增强器实例（使用 ChatClient 列表，支持 fallback 降级链）
    pub fn with_chat_clients(chat_clients: Vec<ChatClient>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        // 从第一个候选获取 base_url 和 token（用于旧路径兼容）
        let (base_url, token) = chat_clients.first()
            .map(|c| (c.base_url.clone(), c.api_key.clone().unwrap_or_default()))
            .unwrap_or_default();

        Ok(Self {
            chat_clients,
            base_url,
            token,
            client,
            project_root: None,
        })
    }

    /// 设置项目根路径
    pub fn with_project_root(mut self, path: &str) -> Self {
        self.project_root = Some(path.to_string());
        self
    }

    /// 从 acemcp 配置创建增强器（兼容旧路径）
    pub async fn from_acemcp_config() -> Result<Self> {
        use crate::mcp::tools::acemcp::AcemcpTool;

        let config = AcemcpTool::get_acemcp_config().await?;
        let base_url = config.base_url
            .ok_or_else(|| anyhow::anyhow!("未配置 Acemcp base_url"))?;
        let token = config.token
            .ok_or_else(|| anyhow::anyhow!("未配置 Acemcp token"))?;

        log_important!(info, "使用 Augment API: url={}, token={}", base_url, mask_api_key(&token));
        Self::new(&base_url, &token)
    }

    /// 从 McpConfig 创建增强器（v5 新增，支持三级降级链）
    pub async fn from_mcp_config() -> Result<Self> {
        use crate::config::storage::load_standalone_config;
        use crate::mcp::tools::enhance::provider_factory::build_enhance_candidates_async;

        let config = load_standalone_config().unwrap_or_default();
        let mcp_config = config.mcp_config;
        let chat_clients = build_enhance_candidates_async(&mcp_config).await;

        log_important!(
            info,
            "enhance 候选列表: {} 个候选 [{}]",
            chat_clients.len(),
            chat_clients.iter()
                .map(|c| format!("{:?}", c.provider))
                .collect::<Vec<_>>()
                .join(" -> ")
        );

        // 为第一个非 RuleEngine 候选记录详细日志
        if let Some(primary) = chat_clients.iter().find(|c| c.provider != ChatProvider::RuleEngine) {
            log_important!(info, "主候选: {:?}, model={}", primary.provider, primary.model);
            if let Some(ref key) = primary.api_key {
                if !key.is_empty() {
                    log_important!(info, "API Key: {}", mask_api_key(key));
                }
            }
        }

        Self::with_chat_clients(chat_clients)
    }

    /// 加载项目的 blob_names（返回匹配到的项目根路径）
    fn load_blob_names(&self) -> (Vec<String>, Option<String>) {
        let project_root = match &self.project_root {
            Some(path) => path.clone(),
            None => return (Vec::new(), None),
        };

        // 规范化项目路径（去除末尾斜杠，避免匹配失败）
        let canonical_root = PathBuf::from(&project_root)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&project_root))
            .to_string_lossy()
            .to_string();
        let normalized_root = Self::clean_path_prefix_and_slashes(&canonical_root);

        // 优先读取 acemcp 的 projects.json，兼容旧的 .sanshu/projects.json
        let mut candidates = Vec::new();
        let acemcp_projects = crate::mcp::tools::acemcp::mcp::home_projects_file();
        candidates.push(acemcp_projects);
        let legacy_projects = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".sanshu")
            .join("projects.json");
        if !candidates.iter().any(|p| p == &legacy_projects) {
            candidates.push(legacy_projects);
        }

        for projects_path in candidates {
            if !projects_path.exists() {
                log_debug!("projects.json 不存在，跳过 blob 加载: {:?}", projects_path);
                continue;
            }

            let content = match fs::read_to_string(&projects_path) {
                Ok(c) => c,
                Err(e) => {
                    log_debug!("读取 projects.json 失败: {}", e);
                    continue;
                }
            };

            let projects: ProjectsFile = match serde_json::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    log_debug!("解析 projects.json 失败: {}", e);
                    continue;
                }
            };

            if let Some((names, matched_root)) = Self::find_project_blobs(&projects, &normalized_root) {
                log_debug!(
                    "已加载 blob_names: count={}, source_root={}",
                    names.len(),
                    matched_root
                );
                return (names, Some(matched_root));
            }
        }

        log_debug!("未在 projects.json 中匹配到项目: {}", normalized_root);
        (Vec::new(), None)
    }

    /// 查找项目根路径对应的 blob 列表（兼容 Windows 大小写差异）
    fn find_project_blobs(
        projects: &ProjectsFile,
        normalized_root: &str,
    ) -> Option<(Vec<String>, String)> {
        // 1) 直接匹配
        if let Some(names) = projects.0.get(normalized_root) {
            return Some((names.clone(), Self::clean_path_prefix_and_slashes(normalized_root)));
        }

        // 2) Windows 下：忽略大小写 + 兼容 keys 带长路径前缀的情况
        if cfg!(windows) {
            let target = normalized_root.to_lowercase();
            for (key, names) in projects.0.iter() {
                // 中文注释：对 key 也做同样清理，避免 legacy projects.json 中残留 //?/ 前缀
                let key_clean = Self::clean_path_prefix_and_slashes(key);
                if key_clean.to_lowercase() == target {
                    return Some((names.clone(), key_clean));
                }
            }
        }

        None
    }

    /// 加载对话历史
    fn load_chat_history(&self, count: usize, selected_ids: Option<&[String]>) -> (Vec<ChatHistoryEntry>, Option<String>) {
        let project_root = match &self.project_root {
            Some(path) => path.clone(),
            None => return (Vec::new(), None),
        };

        match ChatHistoryManager::new(&project_root) {
            Ok(manager) => {
                if let Some(ids) = selected_ids {
                    if ids.is_empty() {
                        return (Vec::new(), None);
                    }
                    return match manager.to_api_format_by_ids(ids) {
                        Ok(v) => (v, None),
                        Err(e) => {
                            log_debug!("加载对话历史失败: {}", e);
                            (Vec::new(), Some(e.to_string()))
                        }
                    };
                }
                match manager.to_api_format(count) {
                    Ok(v) => (v, None),
                    Err(e) => {
                        log_debug!("加载对话历史失败: {}", e);
                        (Vec::new(), Some(e.to_string()))
                    }
                }
            },
            Err(e) => {
                log_debug!("加载对话历史失败: {}", e);
                (Vec::new(), Some(e.to_string()))
            }
        }
    }

    /// 构造“历史为空兜底”的临时历史条目
    fn build_fallback_history_entry(prompt: &str) -> Option<ChatHistoryEntry> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return None;
        }

        // 中文注释：截断兜底内容，避免重复注入导致提示词过长
        let prompt = Self::truncate_text(prompt, MAX_FALLBACK_HISTORY_TEXT_LEN);
        let request_id = format!("fallback_{}", uuid::Uuid::new_v4());

        Some(ChatHistoryEntry {
            request_message: prompt.clone(),
            request_id: request_id.clone(),
            request_nodes: vec![
                ChatHistoryRequestNode {
                    id: 0,
                    node_type: 0,
                    text_node: Some(TextNode { content: prompt.clone() }),
                }
            ],
            // 中文注释：兜底场景无真实 AI 回复，使用空字符串占位，避免破坏 API 结构
            response_nodes: vec![
                ChatHistoryResponseNode {
                    id: 1,
                    node_type: 0,
                    content: Some(String::new()),
                    tool_use: None,
                    thinking: None,
                    billing_metadata: None,
                    metadata: None,
                    token_usage: None,
                }
            ],
        })
    }

    /// 截断并清理文本（避免换行和过长内容）
    fn truncate_text(text: &str, max_len: usize) -> String {
        let cleaned = text
            .replace('\r', " ")
            .replace('\n', " ")
            .trim()
            .to_string();

        if cleaned.chars().count() <= max_len {
            return cleaned;
        }

        let mut truncated: String = cleaned.chars().take(max_len).collect();
        truncated.push_str("...");
        truncated
    }

    /// 构建 zhi 交互历史摘要（轻量补充上下文）
    fn build_zhi_history_summary(&self, count: usize) -> (String, usize) {
        let project_root = match &self.project_root {
            Some(path) => path.clone(),
            None => return (String::new(), 0),
        };

        let manager = match ZhiHistoryManager::new(&project_root) {
            Ok(manager) => manager,
            Err(e) => {
                log_debug!("加载 zhi 历史失败: {}", e);
                return (String::new(), 0);
            }
        };

        let entries = manager.get_recent(count);
        if entries.is_empty() {
            return (String::new(), 0);
        }

        let mut lines = Vec::new();
        for entry in entries {
            let prompt = Self::truncate_text(&entry.prompt, MAX_ZHI_HISTORY_TEXT_LEN);
            let reply = Self::truncate_text(&entry.user_reply, MAX_ZHI_HISTORY_TEXT_LEN);
            if prompt.is_empty() && reply.is_empty() {
                continue;
            }
            lines.push(format!("- Q: {}\n  A: {}", prompt, reply));
        }

        if lines.is_empty() {
            return (String::new(), 0);
        }

        (lines.join("\n"), lines.len())
    }

    /// 构建 chat-stream 请求体
    fn build_request_payload(
        &self,
        prompt: &str,
        original_prompt: Option<&str>,
        current_file: Option<&str>,
        include_history: bool,
        selected_history_ids: Option<&[String]>,
        blob_names: &[String],
    ) -> BuildPayloadResult {
        // 支持按 ID 过滤对话历史，未指定则使用最近历史
        let history_enabled = include_history
            && selected_history_ids.map(|ids| !ids.is_empty()).unwrap_or(true);
        let (mut chat_history, history_load_error) = if history_enabled {
            self.load_chat_history(5, selected_history_ids) // 最多5条历史
        } else {
            (Vec::new(), None)
        };
        let loaded_history_count = chat_history.len();

        // 中文注释：兜底——历史为空时，用当前输入构造 1 条临时历史，确保上下文链路不断
        let mut history_fallback_used = false;
        if history_enabled && chat_history.is_empty() {
            let fallback_text = original_prompt.unwrap_or(prompt);
            if let Some(entry) = Self::build_fallback_history_entry(fallback_text) {
                chat_history.push(entry);
                history_fallback_used = true;
            }
        }

        let (zhi_summary, zhi_count) = if history_enabled {
            self.build_zhi_history_summary(MAX_ZHI_HISTORY_ENTRIES)
        } else {
            (String::new(), 0)
        };

        log_important!(
            info,
            "构建增强请求: blob_count={}, history_count={}, history_fallback_used={}, zhi_history_count={}",
            blob_names.len(),
            loaded_history_count,
            history_fallback_used,
            zhi_count
        );

        // 构建完整消息（系统提示词 + 历史摘要 + 原始提示词）
        let mut full_message = String::new();
        full_message.push_str(ENHANCE_SYSTEM_PROMPT);
        if !zhi_summary.is_empty() {
            full_message.push_str("\n\n[最近交互摘要]\n");
            full_message.push_str(&zhi_summary);
            full_message.push_str("\n\n");
        }
        full_message.push_str(prompt);

        let payload = json!({
            "model": "claude-sonnet-4-5",
            "path": current_file.unwrap_or(""),
            "prefix": null,
            "selected_code": null,
            "suffix": null,
            "message": full_message,
            "chat_history": chat_history,
            "lang": "",
            "blobs": {
                "checkpoint_id": null,
                "added_blobs": blob_names,
                "deleted_blobs": []
            },
            "user_guided_blobs": [],
            "context_code_exchange_request_id": "new",
            "external_source_ids": [],
            "disable_auto_external_sources": null,
            "user_guidelines": "",
            "workspace_guidelines": "",
            "feature_detection_flags": {
                "support_tool_use_start": true,
                "support_parallel_tool_use": true
            },
            "tool_definitions": [],
            "nodes": [
                {
                    "id": 1,
                    "type": 0,
                    "text_node": {
                        "content": full_message
                    }
                }
            ],
            "mode": "CHAT",
            "agent_memories": null,
            "persona_type": 1,
            "rules": [],
            "silent": true,
            "third_party_override": null,
            "conversation_id": uuid::Uuid::new_v4().to_string(),
            "canvas_id": null
        });

        BuildPayloadResult {
            payload,
            history_diag: HistoryBuildDiagnostics {
                loaded_count: loaded_history_count,
                load_error: history_load_error,
                fallback_used: history_fallback_used,
            },
        }
    }

    /// 从响应文本中提取增强后的提示词
    pub fn extract_enhanced_prompt(text: &str) -> Option<String> {
        // 匹配 <augment-enhanced-prompt>...</augment-enhanced-prompt>
        let re = Regex::new(r"<augment-enhanced-prompt>([\s\S]*?)</augment-enhanced-prompt>").ok()?;
        re.captures(text)?
            .get(1)
            .map(|m| m.as_str().trim().to_string())
    }

    /// 解析 SSE 单行（兼容 data: 前缀）
    fn parse_sse_json_line(line: &str) -> Option<serde_json::Value> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let payload = trimmed.strip_prefix("data:")
            .map(|s| s.trim())
            .unwrap_or(trimmed);
        serde_json::from_str::<serde_json::Value>(payload).ok()
    }

    /// 处理跨分片的 SSE 行，保留尾部未完整行
    fn drain_sse_lines<F>(buffer: &mut String, chunk: &str, mut on_line: F)
    where
        F: FnMut(&str),
    {
        buffer.push_str(chunk);
        let mut parts = buffer.split('\n').collect::<Vec<_>>();
        let remainder = parts.pop().unwrap_or("");
        for line in parts {
            on_line(line.trim_end_matches('\r'));
        }
        *buffer = remainder.to_string();
    }

    /// 同步增强（等待完成后返回）
    pub async fn enhance(&self, request: EnhanceRequest) -> Result<EnhanceResponse> {
        // 中文注释：为每次请求生成稳定的 request_id，便于前后端关联
        let request_id = request.request_id.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // 预加载 blob 信息，便于返回给前端展示来源与数量
        let (blob_names, blob_source_root) = self.load_blob_names();
        let blob_count = blob_names.len();
        let project_root_path = request.project_root_path.clone().or(self.project_root.clone());

        let build = self.build_request_payload(
            &request.prompt,
            request.original_prompt.as_deref(),
            request.current_file_path.as_deref(),
            request.include_history,
            request.selected_history_ids.as_deref(),
            &blob_names,
        );
        let history_count = build.history_diag.loaded_count;
        let history_load_error = build.history_diag.load_error.clone();
        let history_fallback_used = build.history_diag.fallback_used;
        // 中文注释：返回给前端的"原始提示词"优先使用传入的 original_prompt
        let response_original_prompt = request.original_prompt.clone()
            .unwrap_or_else(|| request.prompt.clone());

        // 如果有 ChatClient 列表，使用 fallback 降级链
        if !self.chat_clients.is_empty() {
            let mut last_error: Option<String> = None;
            for (idx, chat_client) in self.chat_clients.iter().enumerate() {
                log_important!(
                    info,
                    "enhance fallback: 尝试候选 {}/{} — {:?} (model={})",
                    idx + 1,
                    self.chat_clients.len(),
                    chat_client.provider,
                    chat_client.model
                );
                match self.enhance_via_chat_client(
                    chat_client,
                    &request.prompt,
                    &response_original_prompt,
                    blob_count,
                    history_count,
                    history_load_error.clone(),
                    history_fallback_used,
                    project_root_path.clone(),
                    blob_source_root.clone(),
                    request_id.clone(),
                ).await {
                    Ok(resp) if resp.success => return Ok(resp),
                    Ok(resp) => {
                        let err_msg = resp.error.unwrap_or_else(|| "未知错误".to_string());
                        log_important!(
                            warn,
                            "enhance fallback: 候选 {:?} 失败 — {}，尝试下一个",
                            chat_client.provider,
                            err_msg
                        );
                        last_error = Some(err_msg);
                    }
                    Err(e) => {
                        log_important!(
                            warn,
                            "enhance fallback: 候选 {:?} 异常 — {}，尝试下一个",
                            chat_client.provider,
                            e
                        );
                        last_error = Some(format!("{}", e));
                    }
                }
            }
            // 所有候选均失败
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt,
                success: false,
                error: Some(format!(
                    "所有 {} 个候选均失败，最后错误: {}",
                    self.chat_clients.len(),
                    last_error.unwrap_or_default()
                )),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 回退到旧的 Augment API 流式路径
        let payload = build.payload;
        let url = format!("{}/chat-stream", self.base_url);
        log_important!(info, "发送增强请求（Augment API）: url={}", url);

        let response = self.client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt.clone(),
                success: false,
                error: Some(format!("HTTP {} - {}", status, body)),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 处理 SSE 流式响应
        let mut accumulated_text = String::new();
        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // 使用缓冲拆行，避免 JSON 跨分片丢失
                    Self::drain_sse_lines(&mut sse_buffer, &text, |line| {
                        if let Some(json) = Self::parse_sse_json_line(line) {
                            if let Some(text_chunk) = json.get("text").and_then(|t| t.as_str()) {
                                accumulated_text.push_str(text_chunk);
                            }
                        }
                    });
                }
                Err(e) => {
                    log_debug!("读取流式响应失败: {}", e);
                }
            }
        }
        // 处理最后残留的未换行片段
        if !sse_buffer.trim().is_empty() {
            if let Some(json) = Self::parse_sse_json_line(&sse_buffer) {
                if let Some(text_chunk) = json.get("text").and_then(|t| t.as_str()) {
                    accumulated_text.push_str(text_chunk);
                }
            }
        }

        // 提取增强后的提示词
        let enhanced_prompt = Self::extract_enhanced_prompt(&accumulated_text)
            .unwrap_or_default();

        let success = !enhanced_prompt.is_empty();

        Ok(EnhanceResponse {
            enhanced_prompt,
            original_prompt: response_original_prompt,
            success,
            error: if success { None } else { Some("未能从响应中提取增强结果".to_string()) },
            blob_count,
            history_count,
            history_load_error,
            history_fallback_used,
            project_root_path,
            blob_source_root,
            request_id: Some(request_id),
        })
    }

    /// 使用 ChatClient 进行增强（支持 Ollama / OpenAI 兼容 / 规则引擎）
    async fn enhance_via_chat_client(
        &self,
        chat_client: &ChatClient,
        prompt: &str,
        original_prompt: &str,
        blob_count: usize,
        history_count: usize,
        history_load_error: Option<String>,
        history_fallback_used: bool,
        project_root_path: Option<String>,
        blob_source_root: Option<String>,
        request_id: String,
    ) -> Result<EnhanceResponse> {
        // 如果是规则引擎，使用本地规则增强
        if chat_client.provider == ChatProvider::RuleEngine {
            log_important!(info, "使用规则引擎进行增强");
            let rule_enhancer = RuleEnhancer::new_default();
            let context = EnhanceContext {
                current_file: None,
                project_root: project_root_path.clone(),
            };
            let enhanced = rule_enhancer.enhance(prompt, &context);
            return Ok(EnhanceResponse {
                enhanced_prompt: enhanced,
                original_prompt: original_prompt.to_string(),
                success: true,
                error: None,
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 使用 ChatClient 调用 API（Ollama / OpenAI 兼容 / Gemini / Anthropic）
        log_important!(
            info,
            "使用 ChatClient 进行增强: provider={:?}, model={}",
            chat_client.provider,
            chat_client.model
        );

        // 构建 Chat 消息
        let messages = vec![
            Message::system(ENHANCE_SYSTEM_PROMPT),
            Message::user(prompt),
        ];

        match chat_client.chat_with_retry(&messages).await {
            Ok(response_text) => {
                // 提取增强后的提示词
                let enhanced_prompt = Self::extract_enhanced_prompt(&response_text)
                    .unwrap_or_else(|| {
                        // 如果没有找到标签，尝试使用整个响应（可能是简化格式）
                        response_text.trim().to_string()
                    });

                let success = !enhanced_prompt.is_empty();

                Ok(EnhanceResponse {
                    enhanced_prompt,
                    original_prompt: original_prompt.to_string(),
                    success,
                    error: if success { None } else { Some("未能从响应中提取增强结果".to_string()) },
                    blob_count,
                    history_count,
                    history_load_error,
                    history_fallback_used,
                    project_root_path,
                    blob_source_root,
                    request_id: Some(request_id),
                })
            }
            Err(e) => {
                let error_msg = format!("ChatClient 调用失败: {}", e);
                log_important!(warn, "{}", error_msg);
                Ok(EnhanceResponse {
                    enhanced_prompt: String::new(),
                    original_prompt: original_prompt.to_string(),
                    success: false,
                    error: Some(error_msg),
                    blob_count,
                    history_count,
                    history_load_error,
                    history_fallback_used,
                    project_root_path,
                    blob_source_root,
                    request_id: Some(request_id),
                })
            }
        }
    }

    /// 流式增强（通过回调函数推送进度）
    pub async fn enhance_stream<F>(&self, request: EnhanceRequest, mut on_event: F) -> Result<EnhanceResponse>
    where
        F: FnMut(EnhanceStreamEvent) + Send,
    {
        // 中文注释：为每次请求生成稳定的 request_id，便于前后端关联
        let request_id = request.request_id.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let cancel_flag = request.cancel_flag.clone();

        // 预加载 blob 信息，便于返回给前端展示来源与数量
        let (blob_names, blob_source_root) = self.load_blob_names();
        let blob_count = blob_names.len();
        let project_root_path = request.project_root_path.clone().or(self.project_root.clone());

        let build = self.build_request_payload(
            &request.prompt,
            request.original_prompt.as_deref(),
            request.current_file_path.as_deref(),
            request.include_history,
            request.selected_history_ids.as_deref(),
            &blob_names,
        );
        let history_count = build.history_diag.loaded_count;
        let history_load_error = build.history_diag.load_error.clone();
        let history_fallback_used = build.history_diag.fallback_used;
        // 中文注释：返回给前端的"原始提示词"优先使用传入的 original_prompt
        let response_original_prompt = request.original_prompt.clone()
            .unwrap_or_else(|| request.prompt.clone());

        // 检查取消标志
        if let Some(flag) = &cancel_flag {
            if flag.load(Ordering::Relaxed) {
                let cancel_msg = "已取消增强请求".to_string();
                on_event(EnhanceStreamEvent::error(&request_id, &cancel_msg));
                return Ok(EnhanceResponse {
                    enhanced_prompt: String::new(),
                    original_prompt: response_original_prompt.clone(),
                    success: false,
                    error: Some(cancel_msg),
                    blob_count,
                    history_count,
                    history_load_error,
                    history_fallback_used,
                    project_root_path,
                    blob_source_root,
                    request_id: Some(request_id),
                });
            }
        }

        // 如果有 ChatClient 列表，使用 fallback 降级链（模拟流式事件）
        if !self.chat_clients.is_empty() {
            let mut last_error: Option<String> = None;
            for (idx, chat_client) in self.chat_clients.iter().enumerate() {
                log_important!(
                    info,
                    "enhance_stream fallback: 尝试候选 {}/{} — {:?}",
                    idx + 1,
                    self.chat_clients.len(),
                    chat_client.provider
                );
                match self.enhance_stream_via_chat_client(
                    chat_client,
                    &request.prompt,
                    &response_original_prompt,
                    blob_count,
                    history_count,
                    history_load_error.clone(),
                    history_fallback_used,
                    project_root_path.clone(),
                    blob_source_root.clone(),
                    request_id.clone(),
                    &mut on_event,
                ).await {
                    Ok(resp) if resp.success => return Ok(resp),
                    Ok(resp) => {
                        let err_msg = resp.error.unwrap_or_else(|| "未知错误".to_string());
                        log_important!(warn, "enhance_stream fallback: {:?} 失败 — {}", chat_client.provider, err_msg);
                        last_error = Some(err_msg);
                    }
                    Err(e) => {
                        log_important!(warn, "enhance_stream fallback: {:?} 异常 — {}", chat_client.provider, e);
                        last_error = Some(format!("{}", e));
                    }
                }
            }
            let final_err = format!(
                "所有 {} 个候选均失败，最后错误: {}",
                self.chat_clients.len(),
                last_error.unwrap_or_default()
            );
            on_event(EnhanceStreamEvent::error(&request_id, &final_err));
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt,
                success: false,
                error: Some(final_err),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 回退到旧的 Augment API 流式路径
        let payload = build.payload;
        let url = format!("{}/chat-stream", self.base_url);
        log_important!(info, "发送流式增强请求（Augment API）: url={}", url);

        let response = self.client
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let error_msg = format!("HTTP {} - {}", status, body);
            on_event(EnhanceStreamEvent::error(&request_id, &error_msg));
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt.clone(),
                success: false,
                error: Some(error_msg),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 处理 SSE 流式响应
        let mut accumulated_text = String::new();
        let mut stream = response.bytes_stream();
        let mut chunk_count = 0u32;
        let mut sse_buffer = String::new();
        let mut stream_failed = false;
        let mut stream_error: Option<String> = None;
        let mut cancelled = false;

        while let Some(chunk_result) = stream.next().await {
            if let Some(flag) = &cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }
            }

            match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // 使用缓冲拆行，避免 JSON 跨分片丢失
                    Self::drain_sse_lines(&mut sse_buffer, &text, |line| {
                        if let Some(json) = Self::parse_sse_json_line(line) {
                            if let Some(text_chunk) = json.get("text").and_then(|t| t.as_str()) {
                                if !text_chunk.is_empty() {
                                    accumulated_text.push_str(text_chunk);
                                    chunk_count += 1;

                                    // 估算进度（基于常见响应长度）
                                    let progress = std::cmp::min(90, (chunk_count * 2) as u8);

                                    on_event(EnhanceStreamEvent::chunk(
                                        &request_id,
                                        text_chunk,
                                        &accumulated_text,
                                        progress,
                                    ));
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    log_debug!("读取流式响应失败: {}", e);
                    // 读取失败时通知前端并终止流
                    let error_msg = format!("读取流式响应失败: {}", e);
                    on_event(EnhanceStreamEvent::error(&request_id, &error_msg));
                    stream_failed = true;
                    stream_error = Some(error_msg);
                    break;
                }
            }
        }
        // 中文注释：请求被取消时，停止后续解析与完成事件
        if cancelled {
            let cancel_msg = "已取消增强请求".to_string();
            on_event(EnhanceStreamEvent::error(&request_id, &cancel_msg));
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt.clone(),
                success: false,
                error: Some(cancel_msg),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }
        if stream_failed {
            return Ok(EnhanceResponse {
                enhanced_prompt: String::new(),
                original_prompt: response_original_prompt.clone(),
                success: false,
                error: stream_error.or_else(|| Some("读取流式响应失败".to_string())),
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }
        // 处理最后残留的未换行片段
        if !sse_buffer.trim().is_empty() {
            if let Some(json) = Self::parse_sse_json_line(&sse_buffer) {
                if let Some(text_chunk) = json.get("text").and_then(|t| t.as_str()) {
                    if !text_chunk.is_empty() {
                        accumulated_text.push_str(text_chunk);
                        chunk_count += 1;

                        let progress = std::cmp::min(90, (chunk_count * 2) as u8);
                        on_event(EnhanceStreamEvent::chunk(
                            &request_id,
                            text_chunk,
                            &accumulated_text,
                            progress,
                        ));
                    }
                }
            }
        }

        // 提取增强后的提示词
        let enhanced_prompt = Self::extract_enhanced_prompt(&accumulated_text)
            .unwrap_or_default();

        let success = !enhanced_prompt.is_empty();

        if success {
            on_event(EnhanceStreamEvent::complete(&request_id, &enhanced_prompt, &accumulated_text));
        } else {
            on_event(EnhanceStreamEvent::error(&request_id, "未能从响应中提取增强结果"));
        }

        Ok(EnhanceResponse {
            enhanced_prompt,
            original_prompt: response_original_prompt,
            success,
            error: if success { None } else { Some("未能从响应中提取增强结果".to_string()) },
            blob_count,
            history_count,
            history_load_error,
            history_fallback_used,
            project_root_path,
            blob_source_root,
            request_id: Some(request_id),
        })
    }

    /// 使用 ChatClient 进行流式增强（模拟流式事件）
    async fn enhance_stream_via_chat_client<F>(
        &self,
        chat_client: &ChatClient,
        prompt: &str,
        original_prompt: &str,
        blob_count: usize,
        history_count: usize,
        history_load_error: Option<String>,
        history_fallback_used: bool,
        project_root_path: Option<String>,
        blob_source_root: Option<String>,
        request_id: String,
        on_event: &mut F,
    ) -> Result<EnhanceResponse>
    where
        F: FnMut(EnhanceStreamEvent) + Send,
    {
        // 如果是规则引擎，使用本地规则增强（即时返回）
        if chat_client.provider == ChatProvider::RuleEngine {
            log_important!(info, "使用规则引擎进行流式增强（即时返回）");
            let rule_enhancer = RuleEnhancer::new_default();
            let context = EnhanceContext {
                current_file: None,
                project_root: project_root_path.clone(),
            };
            let enhanced = rule_enhancer.enhance(prompt, &context);

            // 模拟流式事件
            on_event(EnhanceStreamEvent::chunk(&request_id, &enhanced, &enhanced, 50));
            on_event(EnhanceStreamEvent::complete(&request_id, &enhanced, &enhanced));

            return Ok(EnhanceResponse {
                enhanced_prompt: enhanced,
                original_prompt: original_prompt.to_string(),
                success: true,
                error: None,
                blob_count,
                history_count,
                history_load_error,
                history_fallback_used,
                project_root_path,
                blob_source_root,
                request_id: Some(request_id),
            });
        }

        // 使用 ChatClient 调用 API（非流式，但模拟流式事件）
        log_important!(
            info,
            "使用 ChatClient 进行流式增强: provider={:?}, model={}",
            chat_client.provider,
            chat_client.model
        );

        // 发送"开始"事件
        on_event(EnhanceStreamEvent::chunk(&request_id, "", "", 10));

        // 构建 Chat 消息
        let messages = vec![
            Message::system(ENHANCE_SYSTEM_PROMPT),
            Message::user(prompt),
        ];

        match chat_client.chat_with_retry(&messages).await {
            Ok(response_text) => {
                // 模拟进度
                on_event(EnhanceStreamEvent::chunk(&request_id, &response_text, &response_text, 80));

                // 提取增强后的提示词
                let enhanced_prompt = Self::extract_enhanced_prompt(&response_text)
                    .unwrap_or_else(|| {
                        // 如果没有找到标签，尝试使用整个响应
                        response_text.trim().to_string()
                    });

                let success = !enhanced_prompt.is_empty();

                if success {
                    on_event(EnhanceStreamEvent::complete(&request_id, &enhanced_prompt, &response_text));
                } else {
                    on_event(EnhanceStreamEvent::error(&request_id, "未能从响应中提取增强结果"));
                }

                Ok(EnhanceResponse {
                    enhanced_prompt,
                    original_prompt: original_prompt.to_string(),
                    success,
                    error: if success { None } else { Some("未能从响应中提取增强结果".to_string()) },
                    blob_count,
                    history_count,
                    history_load_error,
                    history_fallback_used,
                    project_root_path,
                    blob_source_root,
                    request_id: Some(request_id),
                })
            }
            Err(e) => {
                let error_msg = format!("ChatClient 调用失败: {}", e);
                log_important!(warn, "{}", error_msg);
                on_event(EnhanceStreamEvent::error(&request_id, &error_msg));
                Ok(EnhanceResponse {
                    enhanced_prompt: String::new(),
                    original_prompt: original_prompt.to_string(),
                    success: false,
                    error: Some(error_msg),
                    blob_count,
                    history_count,
                    history_load_error,
                    history_fallback_used,
                    project_root_path,
                    blob_source_root,
                    request_id: Some(request_id),
                })
            }
        }
    }
}

/// 规范化 URL
fn normalize_base_url(input: &str) -> String {
    let mut url = input.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        url = format!("https://{}", url);
    }
    while url.ends_with('/') {
        url.pop();
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // normalize_base_url — 正常路径
    // =========================================================================

    #[test]
    fn test_normalize_base_url_adds_https_prefix() {
        assert_eq!(normalize_base_url("example.com"), "https://example.com");
    }

    #[test]
    fn test_normalize_base_url_preserves_http() {
        assert_eq!(normalize_base_url("http://example.com/"), "http://example.com");
    }

    #[test]
    fn test_normalize_base_url_strips_trailing_slashes() {
        assert_eq!(normalize_base_url("https://example.com///"), "https://example.com");
    }

    // normalize_base_url — 边界条件
    #[test]
    fn test_normalize_base_url_empty_string() {
        // 空字符串经 trim 后仍为空，加前缀 "https://" 再去尾斜杠 → "https:"
        assert_eq!(normalize_base_url(""), "https:");
    }

    #[test]
    fn test_normalize_base_url_whitespace_only() {
        // 纯空白经 trim 后为空，同上
        assert_eq!(normalize_base_url("   "), "https:");
    }

    #[test]
    fn test_normalize_base_url_with_leading_trailing_spaces() {
        assert_eq!(normalize_base_url("  http://api.example.com/  "), "http://api.example.com");
    }

    #[test]
    fn test_normalize_base_url_with_port() {
        assert_eq!(normalize_base_url("http://localhost:11434"), "http://localhost:11434");
    }

    #[test]
    fn test_normalize_base_url_with_path() {
        assert_eq!(normalize_base_url("https://api.example.com/v1"), "https://api.example.com/v1");
    }

    // =========================================================================
    // extract_enhanced_prompt — 正常路径
    // =========================================================================

    #[test]
    fn test_extract_enhanced_prompt_basic() {
        let text = "Here is the result:\n<augment-enhanced-prompt>增强后的提示词</augment-enhanced-prompt>\nDone.";
        let result = PromptEnhancer::extract_enhanced_prompt(text);
        assert_eq!(result, Some("增强后的提示词".to_string()));
    }

    #[test]
    fn test_extract_enhanced_prompt_multiline() {
        let text = "<augment-enhanced-prompt>\n第一行\n第二行\n第三行\n</augment-enhanced-prompt>";
        let result = PromptEnhancer::extract_enhanced_prompt(text);
        assert_eq!(result, Some("第一行\n第二行\n第三行".to_string()));
    }

    #[test]
    fn test_extract_enhanced_prompt_with_code_block() {
        let text = "<augment-enhanced-prompt>请修复以下代码：\n```rust\nfn main() {}\n```</augment-enhanced-prompt>";
        let result = PromptEnhancer::extract_enhanced_prompt(text);
        assert!(result.is_some());
        assert!(result.unwrap().contains("```rust"));
    }

    // extract_enhanced_prompt — 边界条件
    #[test]
    fn test_extract_enhanced_prompt_empty_tags() {
        let text = "<augment-enhanced-prompt></augment-enhanced-prompt>";
        let result = PromptEnhancer::extract_enhanced_prompt(text);
        // 空标签内容 trim 后为空字符串
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_extract_enhanced_prompt_whitespace_only_content() {
        let text = "<augment-enhanced-prompt>   \n  \n   </augment-enhanced-prompt>";
        let result = PromptEnhancer::extract_enhanced_prompt(text);
        assert_eq!(result, Some("".to_string()));
    }

    // extract_enhanced_prompt — 异常路径
    #[test]
    fn test_extract_enhanced_prompt_no_tags() {
        let result = PromptEnhancer::extract_enhanced_prompt("这是普通文本，没有标签");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_enhanced_prompt_incomplete_open_tag() {
        let result = PromptEnhancer::extract_enhanced_prompt("<augment-enhanced-prompt>没有关闭标签");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_enhanced_prompt_empty_input() {
        let result = PromptEnhancer::extract_enhanced_prompt("");
        assert!(result.is_none());
    }

    // =========================================================================
    // parse_sse_json_line — 正常路径
    // =========================================================================

    #[test]
    fn test_parse_sse_json_line_with_data_prefix() {
        let line = r#"data: {"text": "hello"}"#;
        let result = PromptEnhancer::parse_sse_json_line(line);
        assert!(result.is_some());
        let json = result.unwrap();
        assert_eq!(json["text"].as_str(), Some("hello"));
    }

    #[test]
    fn test_parse_sse_json_line_without_prefix() {
        let line = r#"{"text": "world"}"#;
        let result = PromptEnhancer::parse_sse_json_line(line);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["text"].as_str(), Some("world"));
    }

    // parse_sse_json_line — 边界条件
    #[test]
    fn test_parse_sse_json_line_empty() {
        assert!(PromptEnhancer::parse_sse_json_line("").is_none());
    }

    #[test]
    fn test_parse_sse_json_line_whitespace_only() {
        assert!(PromptEnhancer::parse_sse_json_line("   ").is_none());
    }

    #[test]
    fn test_parse_sse_json_line_data_prefix_only() {
        // "data:" 后面没有有效 JSON
        assert!(PromptEnhancer::parse_sse_json_line("data:").is_none());
    }

    // parse_sse_json_line — 异常路径
    #[test]
    fn test_parse_sse_json_line_invalid_json() {
        assert!(PromptEnhancer::parse_sse_json_line("data: {invalid}").is_none());
    }

    #[test]
    fn test_parse_sse_json_line_non_json_text() {
        assert!(PromptEnhancer::parse_sse_json_line("data: hello world").is_none());
    }

    // =========================================================================
    // drain_sse_lines — 正常路径
    // =========================================================================

    #[test]
    fn test_drain_sse_lines_single_complete_line() {
        let mut buffer = String::new();
        let mut lines = Vec::new();
        PromptEnhancer::drain_sse_lines(&mut buffer, "line1\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["line1"]);
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_drain_sse_lines_multiple_lines() {
        let mut buffer = String::new();
        let mut lines = Vec::new();
        PromptEnhancer::drain_sse_lines(&mut buffer, "a\nb\nc\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["a", "b", "c"]);
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_drain_sse_lines_cross_chunk() {
        // 模拟跨分片场景：第一个 chunk 以不完整行结尾
        let mut buffer = String::new();
        let mut lines = Vec::new();

        PromptEnhancer::drain_sse_lines(&mut buffer, "hel", |l| lines.push(l.to_string()));
        assert!(lines.is_empty());
        assert_eq!(buffer, "hel");

        PromptEnhancer::drain_sse_lines(&mut buffer, "lo\nworld\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["hello", "world"]);
        assert_eq!(buffer, "");
    }

    // drain_sse_lines — 边界条件
    #[test]
    fn test_drain_sse_lines_empty_chunk() {
        let mut buffer = String::new();
        let mut lines = Vec::new();
        PromptEnhancer::drain_sse_lines(&mut buffer, "", |l| lines.push(l.to_string()));
        assert!(lines.is_empty());
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_drain_sse_lines_crlf_handling() {
        let mut buffer = String::new();
        let mut lines = Vec::new();
        PromptEnhancer::drain_sse_lines(&mut buffer, "line1\r\nline2\r\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["line1", "line2"]);
    }

    #[test]
    fn test_drain_sse_lines_no_trailing_newline() {
        let mut buffer = String::new();
        let mut lines = Vec::new();
        PromptEnhancer::drain_sse_lines(&mut buffer, "complete\nincomplete", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["complete"]);
        assert_eq!(buffer, "incomplete");
    }

    // =========================================================================
    // clean_path_prefix_and_slashes — 正常路径
    // =========================================================================

    #[test]
    fn test_clean_path_unix_style() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("/home/user/project");
        assert_eq!(result, "/home/user/project");
    }

    #[test]
    fn test_clean_path_windows_backslash() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("C:\\Users\\test\\project");
        assert_eq!(result, "C:/Users/test/project");
    }

    #[test]
    fn test_clean_path_windows_extended_prefix() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("\\\\?\\C:\\Users\\test");
        assert_eq!(result, "C:/Users/test");
    }

    #[test]
    fn test_clean_path_windows_unc_prefix() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("\\\\?\\UNC\\server\\share\\path");
        assert_eq!(result, "//server/share/path");
    }

    // clean_path_prefix_and_slashes — 边界条件
    #[test]
    fn test_clean_path_trailing_slash_removed() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("/home/user/project/");
        assert_eq!(result, "/home/user/project");
    }

    #[test]
    fn test_clean_path_empty_string() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_clean_path_whitespace_trimmed() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("  /home/user  ");
        assert_eq!(result, "/home/user");
    }

    #[test]
    fn test_clean_path_forward_slash_unc() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("//?/UNC/server/share/path");
        assert_eq!(result, "//server/share/path");
    }

    #[test]
    fn test_clean_path_forward_slash_extended() {
        let result = PromptEnhancer::clean_path_prefix_and_slashes("//?/C:/Users/test");
        assert_eq!(result, "C:/Users/test");
    }

    // =========================================================================
    // truncate_text — 正常路径
    // =========================================================================

    #[test]
    fn test_truncate_text_short_text() {
        let result = PromptEnhancer::truncate_text("短文本", 100);
        assert_eq!(result, "短文本");
    }

    #[test]
    fn test_truncate_text_exact_limit() {
        let result = PromptEnhancer::truncate_text("abc", 3);
        assert_eq!(result, "abc");
    }

    #[test]
    fn test_truncate_text_exceeds_limit() {
        let result = PromptEnhancer::truncate_text("abcdefghij", 5);
        assert_eq!(result, "abcde...");
    }

    // truncate_text — 边界条件
    #[test]
    fn test_truncate_text_empty() {
        let result = PromptEnhancer::truncate_text("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_text_newlines_replaced() {
        let result = PromptEnhancer::truncate_text("line1\nline2\r\nline3", 100);
        assert_eq!(result, "line1 line2  line3");
    }

    #[test]
    fn test_truncate_text_chinese_chars() {
        // 中文字符按 char 计数，不按字节
        let result = PromptEnhancer::truncate_text("你好世界测试文本", 4);
        assert_eq!(result, "你好世界...");
    }

    #[test]
    fn test_truncate_text_zero_limit() {
        let result = PromptEnhancer::truncate_text("abc", 0);
        assert_eq!(result, "...");
    }

    // =========================================================================
    // 规则引擎集成（通过 core 调用）
    // =========================================================================

    #[test]
    fn test_rule_engine_via_core_fix_keyword() {
        let enhancer = RuleEnhancer::new_default();
        let context = EnhanceContext {
            current_file: None,
            project_root: None,
        };
        let result = enhancer.enhance("fix the login bug", &context);
        // 规则引擎应追加诊断步骤
        assert!(result.len() > "fix the login bug".len());
    }

    #[test]
    fn test_rule_engine_via_core_no_match() {
        let enhancer = RuleEnhancer::new_default();
        let context = EnhanceContext {
            current_file: None,
            project_root: None,
        };
        let result = enhancer.enhance("hello world", &context);
        // 无匹配规则时返回原文
        assert_eq!(result, "hello world");
    }
}
