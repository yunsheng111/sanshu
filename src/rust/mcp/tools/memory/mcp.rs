use anyhow::Result;
use rmcp::model::{ErrorData as McpError, CallToolResult, Content};

use super::MemoryCategory;
use crate::mcp::{JiyiRequest, utils::{validate_project_path, project_path_error}};
use crate::{log_debug, log_important};

/// 全局记忆管理工具
///
/// 用于存储和管理重要的开发规范、用户偏好和最佳实践
#[derive(Clone)]
pub struct MemoryTool;

impl MemoryTool {
    pub async fn jiyi(
        request: JiyiRequest,
    ) -> Result<CallToolResult, McpError> {
        log_important!(info, "[ji] 调用开始: action={}, project_path={}, content_len={}",
            request.action,
            request.project_path,
            request.content.len()
        );

        // 使用增强的路径验证功能
        if let Err(e) = validate_project_path(&request.project_path) {
            log_important!(warn, "[ji] 路径验证失败: {}", e);
            return Err(project_path_error(format!(
                "路径验证失败: {}\n原始路径: {}\n请检查路径格式是否正确，特别是 Windows 路径应使用正确的盘符格式（如 C:\\path）",
                e,
                request.project_path
            )).into());
        }

        // 创建记忆管理器（会自动执行迁移和启动时去重）
        // 支持非 Git 项目降级模式
        // 使用 SharedMemoryManager 提供并发安全访问
        let start = std::time::Instant::now();
        let manager = super::registry::REGISTRY.get_or_create(&request.project_path)
            .map_err(|e| {
                log_important!(error, "[ji] 创建记忆管理器失败: {}", e);
                McpError::internal_error(format!("创建记忆管理器失败: {}", e), None)
            })?;
        log_debug!("[ji] 记忆管理器创建完成: elapsed={}ms, is_non_git={}",
            start.elapsed().as_millis(), manager.is_non_git_project());

        // 非 Git 项目提示（仅在降级模式时显示）
        let non_git_hint = if manager.is_non_git_project() {
            "\n\n⚠️ 当前目录非 Git 仓库，记忆已存储在项目根目录 `.sanshu-memory` 文件夹中。\n💡 建议初始化 Git 以获得更好的项目记忆隔离：`git init`"
        } else {
            ""
        };

        // 检查 sou 工具是否启用，如果启用则尝试触发后台索引
        let mut index_hint = String::new();
        if is_sou_enabled() {
            if let Err(e) = try_trigger_background_index(&request.project_path).await {
                log_debug!("触发后台索引失败（不影响记忆操作）: {}", e);
            } else {
                index_hint = "\n\n💡 已为当前项目后台启动代码索引，以便后续 sou 工具使用。".to_string();
            }
        }

        let result = match request.action.as_str() {
            "记忆" => {
                if request.content.trim().is_empty() {
                    log_important!(warn, "[ji] 记忆操作失败: 内容为空");
                    return Err(McpError::invalid_params("缺少记忆内容".to_string(), None));
                }

                let category = MemoryCategory::from_str(&request.category);
                log_debug!("[ji] 执行记忆操作: category={:?}, content_len={}", category, request.content.len());

                match manager.add_memory_with_guard_result(&request.content, category) {
                    Ok((Some(id), guard_result)) => {
                        let action_label = match &guard_result.action {
                            super::write_guard::WriteGuardAction::Add => "新增",
                            super::write_guard::WriteGuardAction::Update { .. } => "合并更新",
                            super::write_guard::WriteGuardAction::Noop { .. } => "静默拒绝",
                        };
                        log_important!(info, "[ji] 记忆操作: id={}, action={}, similarity={:.1}%",
                            id, action_label, guard_result.max_similarity * 100.0);
                        format!(
                            "✅ 记忆已{}，ID: {}\n📝 内容: {}\n📂 分类: {}\n🛡️ Write Guard: {} (相似度: {:.1}%){}{}",
                            action_label, id, request.content, category.display_name(),
                            action_label, guard_result.max_similarity * 100.0,
                            index_hint, non_git_hint
                        )
                    }
                    Ok((None, guard_result)) => {
                        log_debug!("[ji] Write Guard NOOP: similarity={:.1}%", guard_result.max_similarity * 100.0);
                        format!(
                            "⚠️ 记忆被 Write Guard 拦截（相似度: {:.1}%，阈值: {:.0}%）\n📝 内容: {}\n📂 分类: {}\n💡 如需强制添加，可降低 write_guard_semantic_threshold{}{}",
                            guard_result.max_similarity * 100.0,
                            manager.config().map(|c| c.write_guard_semantic_threshold * 100.0).unwrap_or(80.0),
                            request.content, category.display_name(),
                            index_hint, non_git_hint
                        )
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 添加记忆失败: {}", e);
                        return Err(McpError::internal_error(format!("添加记忆失败: {}", e), None));
                    }
                }
            }
            "回忆" => {
                let verbose = request.verbose.unwrap_or(false);
                log_debug!("[ji] 执行回忆操作: verbose={}", verbose);

                // 获取记忆列表
                let memories = manager.get_all_memories()
                    .map_err(|e| McpError::internal_error(format!("获取记忆列表失败: {}", e), None))?;

                // Task 4: 回忆操作自动提升活力值
                let boost_start = std::time::Instant::now();
                let mut boosted_count = 0;
                let mut boost_errors = 0;
                for memory in &memories {
                    match manager.access_memory(&memory.id) {
                        Ok(_) => {
                            boosted_count += 1;
                            log_debug!("[ji] 活力值提升: id={}, old={:.2}, new={:.2}",
                                memory.id,
                                memory.vitality_score.unwrap_or(1.5),
                                (memory.vitality_score.unwrap_or(1.5) + manager.config().map(|c| c.vitality_access_boost).unwrap_or(0.5))
                                    .min(manager.config().map(|c| c.vitality_max_score).unwrap_or(3.0))
                            );
                        }
                        Err(e) => {
                            boost_errors += 1;
                            log_debug!("[ji] 活力值提升失败: id={}, error={}", memory.id, e);
                        }
                    }
                }
                log_important!(info, "[ji] 活力值批量提升完成: boosted={}, errors={}, elapsed={}ms",
                    boosted_count, boost_errors, boost_start.elapsed().as_millis());

                if verbose {
                    // 完整模式：返回所有内容
                    let info = manager.get_project_info()
                        .map_err(|e| McpError::internal_error(format!("获取项目信息失败: {}", e), None))?;
                    log_important!(info, "[ji] 回忆完成(verbose): info_len={}", info.len());
                    format!("{}{}{}", info, index_hint, non_git_hint)
                } else {
                    // 压缩模式（默认）：分类汇总 + 摘要
                    let stats = manager.get_stats()
                        .map_err(|e| McpError::internal_error(format!("获取统计失败: {}", e), None))?;

                    let entries_summary: Vec<serde_json::Value> = memories.iter().map(|m| {
                        let summary = m.summary.clone()
                            .unwrap_or_else(|| {
                                let preview: String = m.content.chars().take(80).collect();
                                if m.content.len() > 80 { format!("{}...", preview) } else { preview }
                            });
                        serde_json::json!({
                            "id": m.id,
                            "summary": summary,
                            "category": m.category.display_name()
                        })
                    }).collect();

                    let json_result = serde_json::json!({
                        "total": stats.total,
                        "by_category": {
                            "规范": stats.rules,
                            "偏好": stats.preferences,
                            "模式": stats.patterns,
                            "背景": stats.contexts
                        },
                        "entries": entries_summary,
                        "hint": "使用 verbose=true 获取完整内容"
                    });
                    log_important!(info, "[ji] 回忆完成(compact): total={}", stats.total);
                    format!("{}{}{}", serde_json::to_string_pretty(&json_result).unwrap_or_default(), index_hint, non_git_hint)
                }
            }
            // === 新增: 整理 (执行去重) ===
            "整理" => {
                log_debug!("[ji] 执行整理（去重）操作");
                match manager.deduplicate_with_stats() {
                    Ok(stats) => {
                        log_important!(info, "[ji] 去重完成: original={}, removed={}, remaining={}",
                            stats.original_count, stats.removed_count, stats.remaining_count);
                        // 返回 JSON 格式便于前端解析
                        let json_result = serde_json::json!({
                            "success": true,
                            "original_count": stats.original_count,
                            "removed_count": stats.removed_count,
                            "remaining_count": stats.remaining_count,
                            "removed_ids": stats.removed_ids
                        });
                        format!("✅ 去重整理完成\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 去重整理失败: {}", e);
                        return Err(McpError::internal_error(format!("去重整理失败: {}", e), None));
                    }
                }
            }
            // === 新增: 列表 (获取全部记忆) ===
            "列表" => {
                let page = request.page.unwrap_or(1).max(1);
                let page_size = request.page_size.unwrap_or(20).clamp(1, 100);
                let summary_only = request.summary_only.unwrap_or(false);

                log_debug!("[ji] 执行列表操作: page={}, page_size={}, summary_only={}", page, page_size, summary_only);
                let memories = manager.get_all_memories()
                    .map_err(|e| McpError::internal_error(format!("获取记忆列表失败: {}", e), None))?;

                let total = memories.len();
                let start = (page - 1) * page_size;
                let end = (start + page_size).min(total);
                let page_entries = if start < total { &memories[start..end] } else { &[] as &[_] };

                let entries: Vec<serde_json::Value> = page_entries.iter().map(|m| {
                    if summary_only {
                        let summary = m.summary.clone()
                            .unwrap_or_else(|| {
                                let preview: String = m.content.chars().take(80).collect();
                                if m.content.len() > 80 { format!("{}...", preview) } else { preview }
                            });
                        serde_json::json!({
                            "id": m.id,
                            "summary": summary,
                            "category": m.category.display_name()
                        })
                    } else {
                        serde_json::json!({
                            "id": m.id,
                            "content": m.content,
                            "category": m.category.display_name(),
                            "created_at": m.created_at.to_rfc3339()
                        })
                    }
                }).collect();

                let stats = manager.get_stats()
                    .map_err(|e| McpError::internal_error(format!("获取统计失败: {}", e), None))?;
                log_important!(info, "[ji] 列表完成: total={}, page={}/{}", stats.total, page, (total + page_size - 1) / page_size);
                let json_result = serde_json::json!({
                    "total": stats.total,
                    "page": page,
                    "page_size": page_size,
                    "total_pages": (total + page_size - 1) / page_size.max(1),
                    "by_category": {
                        "规范": stats.rules,
                        "偏好": stats.preferences,
                        "模式": stats.patterns,
                        "背景": stats.contexts
                    },
                    "entries": entries
                });
                serde_json::to_string_pretty(&json_result).unwrap_or_else(|_| "[]".to_string())
            }
            // === 新增: 预览相似 (检测相似度) ===
            "预览相似" => {
                if request.content.trim().is_empty() {
                    log_important!(warn, "[ji] 预览相似失败: 内容为空");
                    return Err(McpError::invalid_params("缺少待检测内容".to_string(), None));
                }
                
                log_debug!("[ji] 执行预览相似: content_len={}", request.content.len());
                let config = manager.config()
                    .map_err(|e| McpError::internal_error(format!("获取配置失败: {}", e), None))?;
                let dedup = super::dedup::MemoryDeduplicator::new(config.similarity_threshold);
                let all_memories = manager.get_all_memories()
                    .map_err(|e| McpError::internal_error(format!("获取记忆列表失败: {}", e), None))?;
                let dup_info = dedup.check_duplicate(&request.content, &all_memories);

                log_important!(info, "[ji] 相似度检测完成: is_dup={}, similarity={:.1}%",
                    dup_info.is_duplicate, dup_info.similarity * 100.0);

                let json_result = serde_json::json!({
                    "is_duplicate": dup_info.is_duplicate,
                    "similarity": format!("{:.1}%", dup_info.similarity * 100.0),
                    "similarity_value": dup_info.similarity,
                    "threshold": config.similarity_threshold,
                    "matched_id": dup_info.matched_id,
                    "matched_content": dup_info.matched_content
                });
                
                if dup_info.is_duplicate {
                    format!("⚠️ 检测到相似内容 (相似度: {:.1}%)\n{}", 
                        dup_info.similarity * 100.0,
                        serde_json::to_string_pretty(&json_result).unwrap_or_default())
                } else {
                    format!("✅ 未检测到相似内容 (最高相似度: {:.1}%)\n{}", 
                        dup_info.similarity * 100.0,
                        serde_json::to_string_pretty(&json_result).unwrap_or_default())
                }
            }
            // === 新增: 配置 (获取/更新配置) ===
            "配置" => {
                // 如果提供了 config 参数，则更新配置
                if let Some(config_req) = request.config {
                    log_debug!("[ji] 执行配置更新: {:?}", config_req);
                    let current_config = manager.config()
                        .map_err(|e| McpError::internal_error(format!("获取配置失败: {}", e), None))?;
                    let mut new_config = current_config.clone();

                    if let Some(threshold) = config_req.similarity_threshold {
                        new_config.similarity_threshold = threshold.clamp(0.5, 0.95);
                    }
                    if let Some(dedup_on_startup) = config_req.dedup_on_startup {
                        new_config.dedup_on_startup = dedup_on_startup;
                    }
                    if let Some(enable_dedup) = config_req.enable_dedup {
                        new_config.enable_dedup = enable_dedup;
                    }
                    // v2.2 新增：Write Guard 配置
                    if let Some(wg_semantic) = config_req.write_guard_semantic_threshold {
                        new_config.write_guard_semantic_threshold = wg_semantic.clamp(0.5, 0.95);
                    }
                    if let Some(wg_update) = config_req.write_guard_update_threshold {
                        new_config.write_guard_update_threshold = wg_update.clamp(0.3, 0.8);
                    }
                    // v2.2 新增：Vitality 配置
                    if let Some(half_life) = config_req.vitality_decay_half_life_days {
                        new_config.vitality_decay_half_life_days = half_life;
                    }
                    if let Some(cleanup_threshold) = config_req.vitality_cleanup_threshold {
                        new_config.vitality_cleanup_threshold = cleanup_threshold.clamp(0.0, 1.0);
                    }
                    if let Some(inactive_days) = config_req.vitality_cleanup_inactive_days {
                        new_config.vitality_cleanup_inactive_days = inactive_days;
                    }
                    if let Some(access_boost) = config_req.vitality_access_boost {
                        new_config.vitality_access_boost = access_boost.clamp(0.0, 1.0);
                    }
                    if let Some(max_score) = config_req.vitality_max_score {
                        new_config.vitality_max_score = max_score.clamp(1.0, 10.0);
                    }

                    manager.update_config(new_config.clone())
                        .map_err(|e| {
                            log_important!(error, "[ji] 更新配置失败: {}", e);
                            McpError::internal_error(format!("更新配置失败: {}", e), None)
                        })?;

                    log_important!(info, "[ji] 配置更新成功: threshold={}, wg_semantic={}, wg_update={}, dedup_on_startup={}, enable_dedup={}",
                        new_config.similarity_threshold,
                        new_config.write_guard_semantic_threshold,
                        new_config.write_guard_update_threshold,
                        new_config.dedup_on_startup, new_config.enable_dedup);

                    let json_result = serde_json::json!({
                        "success": true,
                        "message": "配置已更新",
                        "config": {
                            "similarity_threshold": new_config.similarity_threshold,
                            "dedup_on_startup": new_config.dedup_on_startup,
                            "enable_dedup": new_config.enable_dedup,
                            "write_guard_semantic_threshold": new_config.write_guard_semantic_threshold,
                            "write_guard_update_threshold": new_config.write_guard_update_threshold,
                            "vitality_decay_half_life_days": new_config.vitality_decay_half_life_days,
                            "vitality_cleanup_threshold": new_config.vitality_cleanup_threshold,
                            "vitality_cleanup_inactive_days": new_config.vitality_cleanup_inactive_days,
                            "vitality_access_boost": new_config.vitality_access_boost,
                            "vitality_max_score": new_config.vitality_max_score
                        }
                    });
                    format!("✅ 配置已更新\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                } else {
                    // 返回当前配置（包含 v2.2 新参数）
                    log_debug!("[ji] 获取当前配置");
                    let config = manager.config()
                        .map_err(|e| McpError::internal_error(format!("获取配置失败: {}", e), None))?;
                    let json_result = serde_json::json!({
                        "similarity_threshold": config.similarity_threshold,
                        "dedup_on_startup": config.dedup_on_startup,
                        "enable_dedup": config.enable_dedup,
                        "write_guard_semantic_threshold": config.write_guard_semantic_threshold,
                        "write_guard_update_threshold": config.write_guard_update_threshold,
                        "vitality_decay_half_life_days": config.vitality_decay_half_life_days,
                        "vitality_cleanup_threshold": config.vitality_cleanup_threshold,
                        "vitality_cleanup_inactive_days": config.vitality_cleanup_inactive_days,
                        "vitality_access_boost": config.vitality_access_boost,
                        "vitality_max_score": config.vitality_max_score
                    });
                    format!("📋 当前配置\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                }
            }
            // === 新增: 删除 (移除指定记忆) ===
            "删除" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 删除失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;
                
                log_debug!("[ji] 执行删除操作: memory_id={}", memory_id);
                match manager.delete_memory(memory_id) {
                    Ok(Some(content)) => {
                        log_important!(info, "[ji] 删除成功: id={}, content_len={}", memory_id, content.len());
                        format!("✅ 已删除记忆\n🆔 ID: {}\n📝 内容: {}", memory_id, content)
                    }
                    Ok(None) => {
                        log_debug!("[ji] 删除失败: 未找到记忆 id={}", memory_id);
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 删除记忆失败: {}", e);
                        return Err(McpError::internal_error(format!("删除记忆失败: {}", e), None));
                    }
                }
            }
            // === 新增: 更新 (修改指定记忆) ===
            "更新" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 更新失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;

                if request.content.trim().is_empty() {
                    log_important!(warn, "[ji] 更新失败: 内容为空");
                    return Err(McpError::invalid_params("缺少更新内容（content 参数）".to_string(), None));
                }

                // 解析更新模式：默认 replace，支持 append
                let append = matches!(request.update_mode.as_deref(), Some("append"));
                let mode_label = if append { "追加" } else { "替换" };

                log_debug!("[ji] 执行更新操作: memory_id={}, mode={}, content_len={}",
                    memory_id, mode_label, request.content.len());

                match manager.update_memory(memory_id, &request.content, append) {
                    Ok(Some(id)) => {
                        log_important!(info, "[ji] 更新成功: id={}, mode={}", id, mode_label);
                        format!(
                            "✅ 记忆已更新\n🆔 ID: {}\n📝 模式: {}\n📄 新内容: {}{}{}",
                            id,
                            mode_label,
                            request.content,
                            index_hint,
                            non_git_hint
                        )
                    }
                    Ok(None) => {
                        log_debug!("[ji] 更新失败: 未找到记忆 id={}", memory_id);
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 更新记忆失败: {}", e);
                        return Err(McpError::internal_error(format!("更新记忆失败: {}", e), None));
                    }
                }
            }
            // === P1 新增: 分类 (设置 URI 路径和标签) ===
            "分类" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 分类失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;

                log_debug!("[ji] 执行分类操作: memory_id={}, uri_path={:?}, tags={:?}",
                    memory_id, request.uri_path, request.tags);

                match manager.classify_memory(
                    memory_id,
                    request.uri_path.as_deref(),
                    request.tags.clone(),
                ) {
                    Ok(Some(id)) => {
                        log_important!(info, "[ji] 分类成功: id={}", id);
                        let json_result = serde_json::json!({
                            "success": true,
                            "memory_id": id,
                            "uri_path": request.uri_path,
                            "tags": request.tags
                        });
                        format!("✅ 记忆分类已更新\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Ok(None) => {
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 分类失败: {}", e);
                        return Err(McpError::internal_error(format!("分类失败: {}", e), None));
                    }
                }
            }
            // === P1 新增: 域列表 (获取所有域及统计) ===
            "域列表" => {
                log_debug!("[ji] 执行域列表操作");
                let domains = manager.get_domain_list()
                    .map_err(|e| McpError::internal_error(format!("获取域列表失败: {}", e), None))?;

                let domain_entries: Vec<serde_json::Value> = domains.iter().map(|(name, count)| {
                    serde_json::json!({
                        "domain": name,
                        "entry_count": count
                    })
                }).collect();

                let json_result = serde_json::json!({
                    "total_domains": domains.len(),
                    "domains": domain_entries
                });
                log_important!(info, "[ji] 域列表完成: total={}", domains.len());
                serde_json::to_string_pretty(&json_result).unwrap_or_else(|_| "[]".to_string())
            }
            // === P1 新增: 清理候选 (获取低活力清理候选) ===
            "清理候选" => {
                log_debug!("[ji] 执行清理候选操作");
                let candidates = manager.get_cleanup_candidates()
                    .map_err(|e| McpError::internal_error(format!("获取清理候选失败: {}", e), None))?;

                let candidate_entries: Vec<serde_json::Value> = candidates.iter().map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "content_preview": c.content_preview,
                        "vitality_score": format!("{:.2}", c.vitality_score),
                        "days_since_access": c.days_since_access,
                        "category": c.category
                    })
                }).collect();

                let json_result = serde_json::json!({
                    "total_candidates": candidates.len(),
                    "candidates": candidate_entries,
                    "hint": "使用 '执行清理' 操作并提供 cleanup_ids 参数来确认清理"
                });
                log_important!(info, "[ji] 清理候选完成: total={}", candidates.len());
                serde_json::to_string_pretty(&json_result).unwrap_or_else(|_| "[]".to_string())
            }
            // === P1 新增: 执行清理 (HC-15: 必须先获取候选再执行) ===
            "执行清理" => {
                let cleanup_ids = request.cleanup_ids.as_ref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 执行清理失败: 缺少 cleanup_ids");
                        McpError::invalid_params(
                            "缺少 cleanup_ids 参数。请先调用 '清理候选' 获取候选列表，再提供需要清理的 ID".to_string(),
                            None
                        )
                    })?;

                if cleanup_ids.is_empty() {
                    return Err(McpError::invalid_params("cleanup_ids 不能为空".to_string(), None));
                }

                log_debug!("[ji] 执行清理操作: ids={:?}", cleanup_ids);
                match manager.execute_cleanup(cleanup_ids) {
                    Ok(removed) => {
                        log_important!(info, "[ji] 清理完成: removed={}", removed);
                        let json_result = serde_json::json!({
                            "success": true,
                            "removed_count": removed,
                            "removed_ids": cleanup_ids
                        });
                        format!("✅ 清理完成，已移除 {} 条记忆\n{}", removed,
                            serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 执行清理失败: {}", e);
                        return Err(McpError::internal_error(format!("执行清理失败: {}", e), None));
                    }
                }
            }
            // === P2 新增: 获取快照 (版本历史) ===
            "获取快照" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| McpError::invalid_params("缺少 memory_id".to_string(), None))?;

                let memories = manager.get_all_memories()
                    .map_err(|e| McpError::internal_error(format!("获取记忆列表失败: {}", e), None))?;
                if let Some(entry) = memories.iter().find(|e| e.id == memory_id) {
                    let snapshots: Vec<serde_json::Value> = entry.snapshots.iter().map(|s| {
                        serde_json::json!({
                            "version": s.version,
                            "content": s.content,
                            "created_at": s.created_at.to_rfc3339()
                        })
                    }).collect();

                    serde_json::to_string_pretty(&serde_json::json!({
                        "memory_id": memory_id,
                        "current_version": entry.version,
                        "current_content": entry.content,
                        "snapshots": snapshots
                    })).unwrap_or_default()
                } else {
                    format!("未找到记忆: {}", memory_id)
                }
            }
            // === Task 2 新增: 活力趋势 (获取活力值历史趋势) ===
            "活力趋势" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 活力趋势失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;

                log_debug!("[ji] 执行活力趋势操作: memory_id={}", memory_id);
                match manager.get_vitality_trend(memory_id) {
                    Ok(Some(trend)) => {
                        log_important!(info, "[ji] 活力趋势完成: id={}, current={:.2}, points={}",
                            memory_id, trend.current_vitality, trend.trend_points.len());

                        let trend_points: Vec<serde_json::Value> = trend.trend_points.iter().map(|p| {
                            serde_json::json!({
                                "timestamp": p.timestamp.to_rfc3339(),
                                "vitality_score": format!("{:.2}", p.vitality_score),
                                "event": p.event
                            })
                        }).collect();

                        let json_result = serde_json::json!({
                            "memory_id": trend.memory_id,
                            "current_vitality": format!("{:.2}", trend.current_vitality),
                            "base_vitality": format!("{:.2}", trend.base_vitality),
                            "last_accessed_at": trend.last_accessed_at.to_rfc3339(),
                            "trend_points": trend_points
                        });
                        format!("📈 活力值趋势\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Ok(None) => {
                        log_debug!("[ji] 活力趋势失败: 未找到记忆 id={}", memory_id);
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 获取活力趋势失败: {}", e);
                        return Err(McpError::internal_error(format!("获取活力趋势失败: {}", e), None));
                    }
                }
            }
            // === Task 2 新增: 快照列表 (获取记忆快照列表) ===
            "快照列表" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 快照列表失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;

                log_debug!("[ji] 执行快照列表操作: memory_id={}", memory_id);
                match manager.get_memory_snapshots(memory_id) {
                    Ok(Some(snapshots)) => {
                        log_important!(info, "[ji] 快照列表完成: id={}, count={}", memory_id, snapshots.len());

                        let snapshot_entries: Vec<serde_json::Value> = snapshots.iter().map(|s| {
                            serde_json::json!({
                                "version": s.version,
                                "content": s.content,
                                "created_at": s.created_at.to_rfc3339()
                            })
                        }).collect();

                        let json_result = serde_json::json!({
                            "memory_id": memory_id,
                            "total_snapshots": snapshots.len(),
                            "snapshots": snapshot_entries,
                            "hint": "使用 '回滚快照' 操作并提供 target_version 参数来回滚到指定版本"
                        });
                        format!("📸 快照列表\n{}", serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Ok(None) => {
                        log_debug!("[ji] 快照列表失败: 未找到记忆 id={}", memory_id);
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 获取快照列表失败: {}", e);
                        return Err(McpError::internal_error(format!("获取快照列表失败: {}", e), None));
                    }
                }
            }
            // === Task 2 新增: 回滚快照 (回滚到指定快照版本) ===
            "回滚快照" => {
                let memory_id = request.memory_id.as_deref()
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 回滚快照失败: 缺少 memory_id");
                        McpError::invalid_params("缺少 memory_id 参数".to_string(), None)
                    })?;

                let target_version = request.target_version
                    .ok_or_else(|| {
                        log_important!(warn, "[ji] 回滚快照失败: 缺少 target_version");
                        McpError::invalid_params("缺少 target_version 参数".to_string(), None)
                    })?;

                log_debug!("[ji] 执行回滚快照操作: memory_id={}, target_version={}", memory_id, target_version);
                match manager.rollback_to_snapshot(memory_id, target_version) {
                    Ok(Some(restored_version)) => {
                        log_important!(info, "[ji] 回滚快照成功: id={}, version={}", memory_id, restored_version);
                        let json_result = serde_json::json!({
                            "success": true,
                            "memory_id": memory_id,
                            "restored_version": restored_version
                        });
                        format!("✅ 已回滚到版本 {}\n{}", restored_version,
                            serde_json::to_string_pretty(&json_result).unwrap_or_default())
                    }
                    Ok(None) => {
                        log_debug!("[ji] 回滚快照失败: 未找到记忆 id={}", memory_id);
                        format!("⚠️ 未找到指定 ID 的记忆: {}", memory_id)
                    }
                    Err(e) => {
                        log_important!(error, "[ji] 回滚快照失败: {}", e);
                        return Err(McpError::internal_error(format!("回滚快照失败: {}", e), None));
                    }
                }
            }
            _ => {
                log_important!(warn, "[ji] 未知操作类型: {}", request.action);
                return Err(McpError::invalid_params(
                    format!("未知的操作类型: {}。支持的操作: 记忆 | 回忆 | 整理 | 列表 | 预览相似 | 配置 | 删除 | 更新 | 分类 | 域列表 | 清理候选 | 执行清理 | 获取快照 | 活力趋势 | 快照列表 | 回滚快照", request.action),
                    None
                ));
            }
        };

        log_important!(info, "[ji] 调用完成: action={}, result_len={}", request.action, result.len());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

/// 检查 sou 工具是否启用
fn is_sou_enabled() -> bool {
    match crate::config::load_standalone_config() {
        Ok(config) => config.mcp_config.tools.get("sou").copied().unwrap_or(false),
        Err(_) => false,
    }
}

/// 尝试触发后台索引（仅在项目未初始化或索引失败时）
async fn try_trigger_background_index(project_root: &str) -> Result<()> {
    use super::super::acemcp::mcp::{get_initial_index_state, ensure_initial_index_background, InitialIndexState};

    // 获取 acemcp 配置：复用工具内部读取逻辑，避免字段新增/演进导致此处漏填
    let acemcp_config = super::super::acemcp::mcp::AcemcpTool::get_acemcp_config().await?;

    // 检查索引状态
    let initial_state = get_initial_index_state(project_root);

    // 仅在未初始化或失败时触发
    if matches!(initial_state, InitialIndexState::Missing | InitialIndexState::Idle | InitialIndexState::Failed) {
        ensure_initial_index_background(&acemcp_config, project_root).await?;
        Ok(())
    } else {
        // 已经完成或正在进行，无需操作
        Ok(())
    }
}
