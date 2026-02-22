use std::collections::HashMap;
use tauri::{AppHandle, State};

use crate::config::{AppState, save_config};
use crate::constants::mcp;
// use crate::mcp::tools::acemcp; // 已迁移到独立模块

/// MCP工具配置
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MCPToolConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub can_disable: bool,
    pub icon: String,
    pub icon_bg: String,
    pub dark_icon_bg: String,
    pub has_config: bool, // 是否有配置选项
}

/// 获取MCP工具配置列表
#[tauri::command]
pub async fn get_mcp_tools_config(state: State<'_, AppState>) -> Result<Vec<MCPToolConfig>, String> {
    let config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
    
    // 动态构建工具配置列表
    let mut tools = Vec::new();
    
    // 三术工具 - 始终存在，无配置选项
    tools.push(MCPToolConfig {
        id: mcp::TOOL_ZHI.to_string(),
        name: "三术".to_string(),
        description: "智能代码审查交互工具，支持预定义选项、自由文本输入和图片上传".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_ZHI).copied().unwrap_or(true),
        can_disable: false, // 三术工具是必需的
        icon: "i-carbon-chat text-lg text-blue-600 dark:text-blue-400".to_string(),
        icon_bg: "bg-blue-100 dark:bg-blue-900".to_string(),
        dark_icon_bg: "dark:bg-blue-800".to_string(),
        has_config: false, // 三术工具没有配置选项
    });
    
    // 记忆管理工具 - 始终存在，有配置选项
    tools.push(MCPToolConfig {
        id: mcp::TOOL_JI.to_string(),
        name: "记忆管理".to_string(),
        description: "全局记忆管理工具，用于存储和管理重要的开发规范、用户偏好和最佳实践".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_JI).copied().unwrap_or(true), // 修复：默认启用，与 default_mcp_tools() 保持一致
        can_disable: true,
        icon: "i-carbon-data-base text-lg text-purple-600 dark:text-purple-400".to_string(),
        icon_bg: "bg-green-100 dark:bg-green-900".to_string(),
        dark_icon_bg: "dark:bg-green-800".to_string(),
        has_config: true, // 记忆管理工具有配置选项
    });
    
    // 代码搜索工具 - 始终存在，有配置选项
    tools.push(MCPToolConfig {
        id: mcp::TOOL_SOU.to_string(),
        name: "代码搜索".to_string(),
        description: "基于查询在特定项目中搜索相关的代码上下文，支持语义搜索和增量索引".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_SOU).copied().unwrap_or(false),
        can_disable: true,
        icon: "i-carbon-search text-lg text-green-600 dark:text-green-400".to_string(),
        icon_bg: "bg-green-100 dark:bg-green-900".to_string(),
        dark_icon_bg: "dark:bg-green-800".to_string(),
        has_config: true, // 代码搜索工具有配置选项
    });

    // Context7 文档查询工具 - 始终存在，有配置选项
    tools.push(MCPToolConfig {
        id: mcp::TOOL_CONTEXT7.to_string(),
        name: "Context7 文档查询".to_string(),
        description: "查询最新的框架和库文档，支持 Next.js、React、Vue、Spring 等主流框架".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_CONTEXT7).copied().unwrap_or(true),
        can_disable: true,
        icon: "i-carbon-document text-lg text-orange-600 dark:text-orange-400".to_string(),
        icon_bg: "bg-orange-100 dark:bg-orange-900".to_string(),
        dark_icon_bg: "dark:bg-orange-800".to_string(),
        has_config: true, // Context7 工具有配置选项
    });

    // UI/UX Pro Max 工具
    tools.push(MCPToolConfig {
        id: mcp::TOOL_UIUX.to_string(),
        name: "UI/UX Pro Max".to_string(),
        description: "UI/UX 设计智能检索与设计系统生成工具".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_UIUX).copied().unwrap_or(true),
        can_disable: true,
        icon: "i-carbon-color-palette text-lg text-pink-600 dark:text-pink-400".to_string(),
        icon_bg: "bg-pink-100 dark:bg-pink-900".to_string(),
        dark_icon_bg: "dark:bg-pink-800".to_string(),
        has_config: false,
    });

    // 提示词增强工具 - 依赖 acemcp 配置
    tools.push(MCPToolConfig {
        id: mcp::TOOL_ENHANCE.to_string(),
        name: "提示词增强".to_string(),
        description: "将口语化提示词增强为结构化专业提示词，支持上下文与历史".to_string(),
        enabled: config.mcp_config.tools.get(mcp::TOOL_ENHANCE).copied().unwrap_or(false),
        can_disable: true,
        icon: "i-carbon-magic-wand text-lg text-indigo-600 dark:text-indigo-400".to_string(),
        icon_bg: "bg-indigo-100 dark:bg-indigo-900".to_string(),
        dark_icon_bg: "dark:bg-indigo-800".to_string(),
        has_config: true, // 提示词增强有独立配置面板
    });

    // 图标工坊工具 - UI 功能工具，始终存在，有配置选项
    tools.push(MCPToolConfig {
        id: "icon".to_string(),
        name: "图标工坊".to_string(),
        description: "搜索和管理 Iconfont 图标库，支持预览、复制 SVG 和下载到项目".to_string(),
        enabled: config.mcp_config.tools.get("icon").copied().unwrap_or(true),
        can_disable: true,
        icon: "i-carbon-image text-lg text-purple-600 dark:text-purple-400".to_string(),
        icon_bg: "bg-purple-100 dark:bg-purple-900".to_string(),
        dark_icon_bg: "dark:bg-purple-800".to_string(),
        has_config: true, // 图标工坊有配置选项
    });

    // 按启用状态排序，启用的在前
    tools.sort_by(|a, b| b.enabled.cmp(&a.enabled));
    
    Ok(tools)
}

/// 设置MCP工具启用状态
#[tauri::command]
pub async fn set_mcp_tool_enabled(
    tool_id: String,
    enabled: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
        
        // 检查工具是否可以禁用
        if tool_id == mcp::TOOL_ZHI && !enabled {
            return Err("三术工具是必需的，无法禁用".to_string());
        }
        
        // 更新工具状态
        config.mcp_config.tools.insert(tool_id.clone(), enabled);
    }
    
    // 保存配置
    save_config(&state, &app).await
        .map_err(|e| format!("保存配置失败: {}", e))?;

    // 使用日志记录状态变更（在 MCP 模式下会自动输出到文件）
    log::info!("MCP工具 {} 状态已更新为: {}", tool_id, enabled);

    Ok(())
}

/// 获取所有MCP工具状态
#[tauri::command]
pub async fn get_mcp_tools_status(state: State<'_, AppState>) -> Result<HashMap<String, bool>, String> {
    let config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
    Ok(config.mcp_config.tools.clone())
}

/// 重置MCP工具配置为默认值
#[tauri::command]
pub async fn reset_mcp_tools_config(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
        let default_config = mcp::get_default_mcp_config();
        config.mcp_config.tools.clear();
        for tool in &default_config.tools {
            config.mcp_config.tools.insert(tool.tool_id.clone(), tool.enabled);
        }
    }
    
    // 保存配置
    save_config(&state, &app).await
        .map_err(|e| format!("保存配置失败: {}", e))?;

    // 使用日志记录配置重置（在 MCP 模式下会自动输出到文件）
    log::info!("MCP工具配置已重置为默认值");
    Ok(())
}

// acemcp 相关命令已迁移

// 已移除 Python Web 服务相关函数，完全使用 Rust 实现
// 如需调试配置，请直接查看本地配置文件

// ============ 记忆管理相关命令 ============

use crate::mcp::tools::memory::{SharedMemoryManager, MemoryConfig};

/// 记忆条目 DTO（用于前端展示）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntryDto {
    pub id: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
}

/// 记忆配置 DTO（用于前端交互）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryConfigDto {
    pub similarity_threshold: f64,
    pub dedup_on_startup: bool,
    pub enable_dedup: bool,
}

/// 去重结果 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DedupResultDto {
    pub original_count: usize,
    pub removed_count: usize,
    pub remaining_count: usize,
    pub removed_ids: Vec<String>,
}

/// 记忆统计 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryStatsDto {
    pub total: usize,
    pub rules: usize,
    pub preferences: usize,
    pub patterns: usize,
    pub contexts: usize,
}

/// 相似度预览结果 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SimilarityPreviewDto {
    pub is_duplicate: bool,
    pub similarity: f64,
    pub matched_id: Option<String>,
    pub matched_content: Option<String>,
    pub threshold: f64,
}

/// 获取记忆列表
#[tauri::command]
pub async fn get_memory_list(project_path: String) -> Result<Vec<MemoryEntryDto>, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let memories = manager.get_all_memories()
        .map_err(|e| format!("获取记忆列表失败: {}", e))?;
    let entries: Vec<MemoryEntryDto> = memories.iter().map(|m| MemoryEntryDto {
        id: m.id.clone(),
        content: m.content.clone(),
        category: m.category.display_name().to_string(),
        created_at: m.created_at.to_rfc3339(),
    }).collect();
    
    Ok(entries)
}

/// 获取记忆统计
#[tauri::command]
pub async fn get_memory_stats(project_path: String) -> Result<MemoryStatsDto, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let stats = manager.get_stats()
        .map_err(|e| format!("获取统计失败: {}", e))?;
    Ok(MemoryStatsDto {
        total: stats.total,
        rules: stats.rules,
        preferences: stats.preferences,
        patterns: stats.patterns,
        contexts: stats.contexts,
    })
}

/// 获取记忆配置
#[tauri::command]
pub async fn get_memory_config(project_path: String) -> Result<MemoryConfigDto, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let config = manager.config()
        .map_err(|e| format!("获取配置失败: {}", e))?;
    Ok(MemoryConfigDto {
        similarity_threshold: config.similarity_threshold,
        dedup_on_startup: config.dedup_on_startup,
        enable_dedup: config.enable_dedup,
    })
}

/// 保存记忆配置
#[tauri::command]
pub async fn save_memory_config(project_path: String, config: MemoryConfigDto) -> Result<(), String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;
    
    let new_config = MemoryConfig {
        similarity_threshold: config.similarity_threshold.clamp(0.5, 0.95),
        dedup_on_startup: config.dedup_on_startup,
        enable_dedup: config.enable_dedup,
        max_entry_bytes: 10240, // 10KB 默认值
        max_entries: 1000,      // 1000 条默认值
        // v2.2 新增配置使用默认值
        write_guard_semantic_threshold: 0.80,
        write_guard_update_threshold: 0.60,
        vitality_decay_half_life_days: 30,
        vitality_cleanup_threshold: 0.35,
        vitality_cleanup_inactive_days: 14,
        vitality_access_boost: 0.5,
        vitality_max_score: 3.0,
        summary_length_threshold: 500,
    };
    
    manager.update_config(new_config)
        .map_err(|e| format!("保存配置失败: {}", e))?;
    
    log::info!("记忆配置已更新: {:?}", config);
    Ok(())
}

/// 执行去重整理
#[tauri::command]
pub async fn deduplicate_memories(project_path: String) -> Result<DedupResultDto, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;
    
    let stats = manager.deduplicate_with_stats()
        .map_err(|e| format!("去重失败: {}", e))?;
    
    Ok(DedupResultDto {
        original_count: stats.original_count,
        removed_count: stats.removed_count,
        remaining_count: stats.remaining_count,
        removed_ids: stats.removed_ids,
    })
}

/// 预览相似度
#[tauri::command]
pub async fn preview_similarity(project_path: String, content: String) -> Result<SimilarityPreviewDto, String> {
    use crate::mcp::tools::memory::dedup::MemoryDeduplicator;

    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let config = manager.config()
        .map_err(|e| format!("获取配置失败: {}", e))?;
    let threshold = config.similarity_threshold;
    let dedup = MemoryDeduplicator::new(threshold);
    let memories = manager.get_all_memories()
        .map_err(|e| format!("获取记忆列表失败: {}", e))?;
    let dup_info = dedup.check_duplicate(&content, &memories);
    
    Ok(SimilarityPreviewDto {
        is_duplicate: dup_info.is_duplicate,
        similarity: dup_info.similarity,
        matched_id: dup_info.matched_id,
        matched_content: dup_info.matched_content,
        threshold,
    })
}

/// 删除记忆
#[tauri::command]
pub async fn delete_memory(project_path: String, memory_id: String) -> Result<String, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    match manager.delete_memory(&memory_id) {
        Ok(Some(content)) => {
            log::info!("已删除记忆: {} - {}", memory_id, content);
            Ok(content)
        }
        Ok(None) => Err(format!("未找到指定 ID 的记忆: {}", memory_id)),
        Err(e) => Err(format!("删除记忆失败: {}", e)),
    }
}

/// 搜索记忆结果 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SearchMemoryResultDto {
    pub id: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub relevance: f64,
    pub highlight: String,
}

/// 搜索记忆（支持关键词模糊匹配）
#[tauri::command]
pub async fn search_memories(
    project_path: String,
    query: String,
    category: Option<String>,
    domain: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Vec<SearchMemoryResultDto>, String> {
    use crate::mcp::tools::memory::similarity::TextSimilarity;

    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let memories = manager.get_all_memories()
        .map_err(|e| format!("获取记忆列表失败: {}", e))?;

    let query_lower = query.to_lowercase();
    let query_normalized = TextSimilarity::normalize(&query);

    let mut results: Vec<SearchMemoryResultDto> = memories
        .iter()
        .filter_map(|m| {
            // 分类过滤
            if let Some(ref cat) = category {
                if m.category.display_name() != cat {
                    return None;
                }
            }

            // 域过滤（SC-17）
            if let Some(ref domain_filter) = domain {
                if let Some(ref entry_domain) = m.domain {
                    if !entry_domain.contains(domain_filter) {
                        return None;
                    }
                } else {
                    // 记忆没有域信息，不匹配域过滤
                    return None;
                }
            }

            // 标签过滤（SC-17）
            if let Some(ref tags_filter) = tags {
                if let Some(ref entry_tags) = m.tags {
                    // 要求所有过滤标签都存在于记忆的标签中
                    if !tags_filter.iter().all(|t| entry_tags.contains(t)) {
                        return None;
                    }
                } else {
                    // 记忆没有标签信息，不匹配标签过滤
                    return None;
                }
            }

            // 计算相关度（结合包含匹配、意图识别和相似度）
            let content_lower = m.content.to_lowercase();
            let mut relevance = 0.0;

            // 精确包含匹配（高权重）
            if content_lower.contains(&query_lower) {
                relevance += 0.5;
            }

            // 词级匹配
            let query_words: Vec<&str> = query_lower.split_whitespace().collect();
            let matched_words = query_words.iter()
                .filter(|w| content_lower.contains(*w))
                .count();
            if !query_words.is_empty() {
                relevance += 0.15 * (matched_words as f64 / query_words.len() as f64);
            }

            // 意图识别加权（SC-24）
            let intent_boost = calculate_intent_boost(&query_lower, &content_lower, &m.category);
            relevance += intent_boost;

            // 相似度计算
            let similarity = TextSimilarity::calculate(&query_normalized, &m.content_normalized);
            relevance += 0.15 * similarity;

            // 过滤低相关度结果
            if relevance < 0.1 {
                return None;
            }

            // 生成高亮片段（截取包含关键词的部分）
            let highlight = generate_highlight(&m.content, &query_lower, 100);

            Some(SearchMemoryResultDto {
                id: m.id.clone(),
                content: m.content.clone(),
                category: m.category.display_name().to_string(),
                created_at: m.created_at.to_rfc3339(),
                relevance,
                highlight,
            })
        })
        .collect();

    // 按相关度降序排序
    results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));

    Ok(results)
}

/// 意图识别加权（SC-24）
///
/// 根据查询关键词和记忆分类，识别用户意图并提升相关度
fn calculate_intent_boost(query: &str, content: &str, category: &crate::mcp::tools::memory::MemoryCategory) -> f64 {
    use crate::mcp::tools::memory::MemoryCategory;

    let mut boost = 0.0;

    // 规范类意图关键词
    let rule_keywords = ["规范", "规则", "约束", "标准", "要求", "必须", "禁止", "应该"];
    // 偏好类意图关键词
    let preference_keywords = ["偏好", "喜欢", "习惯", "风格", "倾向", "选择"];
    // 模式类意图关键词
    let pattern_keywords = ["模式", "实践", "方法", "技巧", "经验", "最佳", "优化"];
    // 背景类意图关键词
    let context_keywords = ["背景", "上下文", "项目", "架构", "技术栈", "依赖"];

    // 检查查询中是否包含分类相关关键词
    let has_rule_intent = rule_keywords.iter().any(|k| query.contains(k));
    let has_preference_intent = preference_keywords.iter().any(|k| query.contains(k));
    let has_pattern_intent = pattern_keywords.iter().any(|k| query.contains(k));
    let has_context_intent = context_keywords.iter().any(|k| query.contains(k));

    // 检查内容中是否包含分类相关关键词
    let content_has_rule = rule_keywords.iter().any(|k| content.contains(k));
    let content_has_preference = preference_keywords.iter().any(|k| content.contains(k));
    let content_has_pattern = pattern_keywords.iter().any(|k| content.contains(k));
    let content_has_context = context_keywords.iter().any(|k| content.contains(k));

    // 意图与分类匹配时提升权重
    match category {
        MemoryCategory::Rule => {
            if has_rule_intent || content_has_rule {
                boost += 0.2;
            }
        }
        MemoryCategory::Preference => {
            if has_preference_intent || content_has_preference {
                boost += 0.2;
            }
        }
        MemoryCategory::Pattern => {
            if has_pattern_intent || content_has_pattern {
                boost += 0.2;
            }
        }
        MemoryCategory::Context => {
            if has_context_intent || content_has_context {
                boost += 0.2;
            }
        }
    }

    boost
}

/// 生成高亮片段
fn generate_highlight(content: &str, query: &str, max_len: usize) -> String {
    let content_lower = content.to_lowercase();

    // 查找关键词位置
    if let Some(pos) = content_lower.find(query) {
        let start = pos.saturating_sub(20);
        let end = (pos + query.len() + max_len).min(content.len());

        let mut snippet = String::new();
        if start > 0 {
            snippet.push_str("...");
        }
        snippet.push_str(&content[start..end]);
        if end < content.len() {
            snippet.push_str("...");
        }
        snippet
    } else {
        // 未找到关键词，返回开头部分
        if content.len() > max_len {
            format!("{}...", &content[..max_len])
        } else {
            content.to_string()
        }
    }
}

/// 更新记忆 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UpdateMemoryDto {
    pub memory_id: String,
    pub content: String,
    pub append: bool,
}

/// 更新记忆内容（对接 SC-4）
#[tauri::command]
pub async fn update_memory(
    project_path: String,
    update: UpdateMemoryDto,
) -> Result<String, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    match manager.update_memory(&update.memory_id, &update.content, update.append) {
        Ok(Some(id)) => {
            log::info!(
                "已更新记忆: {} (append={})",
                id,
                update.append
            );
            Ok(id)
        }
        Ok(None) => Err(format!("未找到指定 ID 的记忆: {}", update.memory_id)),
        Err(e) => Err(format!("更新记忆失败: {}", e)),
    }
}

/// 导出记忆 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExportMemoriesDto {
    pub version: String,
    pub exported_at: String,
    pub project_path: String,
    pub total_count: usize,
    pub entries: Vec<MemoryEntryDto>,
}

/// 导出记忆为 JSON 格式
#[tauri::command]
pub async fn export_memories(project_path: String) -> Result<ExportMemoriesDto, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let memories = manager.get_all_memories()
        .map_err(|e| format!("获取记忆列表失败: {}", e))?;

    let entries: Vec<MemoryEntryDto> = memories
        .iter()
        .map(|m| MemoryEntryDto {
            id: m.id.clone(),
            content: m.content.clone(),
            category: m.category.display_name().to_string(),
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();

    Ok(ExportMemoriesDto {
        version: "2.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        project_path,
        total_count: entries.len(),
        entries,
    })
}

/// 域信息 DTO（供前端 DomainTree 使用）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DomainInfoDto {
    /// 域路径
    pub path: String,
    /// 该域下的记忆数量
    pub count: usize,
    /// 子域列表（当前实现为扁平结构，无子域）
    pub children: Vec<DomainInfoDto>,
}

/// 获取域列表（供前端 DomainTree 组件使用）
#[tauri::command]
pub async fn get_domain_list(project_path: String) -> Result<Vec<DomainInfoDto>, String> {
    let manager = SharedMemoryManager::new(&project_path)
        .map_err(|e| format!("创建记忆管理器失败: {}", e))?;

    let domain_list = manager.get_domain_list()
        .map_err(|e| format!("获取域列表失败: {}", e))?;

    // 将 (domain, count) 扁平列表转换为 DomainInfoDto
    let result: Vec<DomainInfoDto> = domain_list
        .into_iter()
        .map(|(path, count)| DomainInfoDto {
            path,
            count,
            children: Vec::new(),
        })
        .collect();

    Ok(result)
}

// ============ 提示词增强配置命令 ============

/// 提示词增强配置 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnhanceConfigDto {
    /// 当前提供者: "ollama" | "openai_compat" | "rule_engine"
    pub provider: String,
    /// Ollama 端点
    pub ollama_url: String,
    /// Ollama 模型
    pub ollama_model: String,
    /// OpenAI 兼容 API 端点
    pub base_url: String,
    /// API Key
    pub api_key: String,
    /// 模型名称
    pub model: String,
    /// 降级链通道顺序（可选）：如 ["ollama", "cloud", "rule_engine"]
    #[serde(default)]
    pub channel_order: Option<Vec<String>>,
}

/// 获取提示词增强配置
#[tauri::command]
pub async fn get_enhance_config(state: State<'_, AppState>) -> Result<EnhanceConfigDto, String> {
    let config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
    let mcp = &config.mcp_config;

    Ok(EnhanceConfigDto {
        provider: mcp.enhance_provider.clone().unwrap_or_else(|| "ollama".to_string()),
        ollama_url: mcp.enhance_ollama_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string()),
        ollama_model: mcp.enhance_ollama_model.clone().unwrap_or_else(|| "qwen2.5-coder:7b".to_string()),
        base_url: mcp.enhance_base_url.clone().unwrap_or_default(),
        api_key: mcp.enhance_api_key.clone().unwrap_or_default(),
        model: mcp.enhance_model.clone().unwrap_or_else(|| "Qwen/Qwen2.5-Coder-7B-Instruct".to_string()),
        channel_order: mcp.enhance_channel_order.clone(),
    })
}

/// 保存提示词增强配置
#[tauri::command]
pub async fn save_enhance_config(
    config_dto: EnhanceConfigDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
        let mcp = &mut config.mcp_config;

        mcp.enhance_provider = Some(config_dto.provider);
        mcp.enhance_ollama_url = Some(config_dto.ollama_url);
        mcp.enhance_ollama_model = Some(config_dto.ollama_model);
        mcp.enhance_base_url = if config_dto.base_url.is_empty() { None } else { Some(config_dto.base_url) };
        mcp.enhance_api_key = if config_dto.api_key.is_empty() { None } else { Some(config_dto.api_key) };
        mcp.enhance_model = Some(config_dto.model);
        mcp.enhance_channel_order = config_dto.channel_order;
    }

    save_config(&state, &app).await
        .map_err(|e| format!("保存配置失败: {}", e))?;

    log::info!("提示词增强配置已保存");
    Ok(())
}

// ============ 代码搜索（sou）配置命令 ============

/// 代码搜索配置 DTO
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SouConfigDto {
    /// 模式: "local" | "acemcp"
    pub mode: String,
    /// 嵌入提供者: "jina" | "siliconflow" | "ollama" 等
    pub embedding_provider: String,
    /// 嵌入 API 端点
    pub embedding_base_url: String,
    /// 嵌入 API Key
    pub embedding_api_key: String,
    /// 嵌入模型名称
    pub embedding_model: String,
    /// 索引存储路径
    pub index_path: String,
}

/// 获取代码搜索配置
#[tauri::command]
pub async fn get_sou_config(state: State<'_, AppState>) -> Result<SouConfigDto, String> {
    let config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
    let mcp = &config.mcp_config;

    Ok(SouConfigDto {
        mode: mcp.sou_mode.clone().unwrap_or_else(|| "acemcp".to_string()),
        embedding_provider: mcp.sou_embedding_provider.clone().unwrap_or_else(|| "jina".to_string()),
        embedding_base_url: mcp.sou_embedding_base_url.clone().unwrap_or_default(),
        embedding_api_key: mcp.sou_embedding_api_key.clone().unwrap_or_default(),
        embedding_model: mcp.sou_embedding_model.clone().unwrap_or_default(),
        index_path: mcp.sou_index_path.clone().unwrap_or_else(|| ".sanshu-index".to_string()),
    })
}

/// 保存代码搜索配置
#[tauri::command]
pub async fn save_sou_config(
    config_dto: SouConfigDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut config = state.config.lock().map_err(|e| format!("获取配置失败: {}", e))?;
        let mcp = &mut config.mcp_config;

        mcp.sou_mode = Some(config_dto.mode);
        mcp.sou_embedding_provider = Some(config_dto.embedding_provider);
        mcp.sou_embedding_base_url = if config_dto.embedding_base_url.is_empty() { None } else { Some(config_dto.embedding_base_url) };
        mcp.sou_embedding_api_key = if config_dto.embedding_api_key.is_empty() { None } else { Some(config_dto.embedding_api_key) };
        mcp.sou_embedding_model = if config_dto.embedding_model.is_empty() { None } else { Some(config_dto.embedding_model) };
        mcp.sou_index_path = Some(config_dto.index_path);
    }

    save_config(&state, &app).await
        .map_err(|e| format!("保存配置失败: {}", e))?;

    log::info!("代码搜索配置已保存");
    Ok(())
}

// 集成测试模块
#[cfg(test)]
#[path = "commands_test.rs"]
mod commands_test;
