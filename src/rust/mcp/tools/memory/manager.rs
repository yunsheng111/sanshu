//! 记忆管理器
//!
//! 核心记忆管理功能，包括：
//! - 记忆的添加、查询
//! - 启动时自动迁移和去重
//! - JSON 格式存储
//! - 并发安全的 SharedMemoryManager 包装

use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::types::{MemoryEntry, MemoryCategory, MemoryStore, MemoryConfig, MemorySnapshot};
use super::similarity::TextSimilarity;
use super::dedup::MemoryDeduplicator;
use super::migration::MemoryMigrator;
use crate::log_debug;

/// 记忆管理器
pub struct MemoryManager {
    /// 记忆目录路径
    memory_dir: PathBuf,
    /// 项目路径
    project_path: String,
    /// 存储数据
    store: MemoryStore,
    /// 是否为非 Git 项目（降级模式）
    is_non_git_project: bool,
}

/// 路径规范化结果
struct NormalizeResult {
    /// 规范化后的路径
    path: PathBuf,
    /// 是否为非 Git 项目
    is_non_git: bool,
}

impl MemoryManager {
    /// 存储文件名
    const STORE_FILE: &'static str = "memories.json";

    /// 创建新的记忆管理器
    ///
    /// 自动执行：
    /// 1. 路径规范化和验证（支持非 Git 项目降级）
    /// 2. 旧格式迁移（如果需要）
    /// 3. 启动时去重（如果配置启用）
    pub fn new(project_path: &str) -> Result<Self> {
        // 规范化项目路径（支持非 Git 项目降级）
        let normalize_result = Self::normalize_project_path(project_path)?;
        let memory_dir = normalize_result.path.join(".sanshu-memory");

        // 创建记忆目录
        fs::create_dir_all(&memory_dir)
            .map_err(|e| anyhow::anyhow!(
                "无法创建记忆目录: {}\n错误: {}\n这可能是因为项目目录没有写入权限。",
                Self::clean_display_path(&memory_dir),
                e
            ))?;

        let project_path_str = Self::clean_display_path(&normalize_result.path);

        // 检查是否需要迁移
        if MemoryMigrator::needs_migration(&memory_dir) {
            log_debug!("检测到旧版记忆格式，开始迁移...");
            match MemoryMigrator::migrate(&memory_dir, &project_path_str) {
                Ok(result) => {
                    log_debug!(
                        "迁移完成: 读取 {} 条，去重后 {} 条，移除 {} 条重复",
                        result.md_entries_count,
                        result.deduped_entries_count,
                        result.removed_duplicates
                    );
                }
                Err(e) => {
                    log_debug!("迁移失败（将使用空存储）: {}", e);
                }
            }
        }

        // 加载或创建存储
        let store_path = memory_dir.join(Self::STORE_FILE);
        let mut store = if store_path.exists() {
            let content = fs::read_to_string(&store_path)?;
            let mut loaded_store: MemoryStore = serde_json::from_str(&content).unwrap_or_else(|e| {
                log_debug!("解析存储文件失败，使用默认值: {}", e);
                MemoryStore {
                    project_path: project_path_str.clone(),
                    ..Default::default()
                }
            });

            // SC-5: 版本兼容性检查和升级
            let (is_compatible, needs_upgrade) = loaded_store.check_version_compatibility();
            if !is_compatible {
                log_debug!(
                    "存储版本不兼容: {}，将使用默认值",
                    loaded_store.version
                );
                MemoryStore {
                    project_path: project_path_str.clone(),
                    ..Default::default()
                }
            } else if needs_upgrade {
                log_debug!(
                    "检测到旧版存储格式 {}，升级到 {}",
                    loaded_store.version,
                    MemoryStore::CURRENT_VERSION
                );
                if let Err(e) = loaded_store.upgrade_to_current() {
                    log_debug!("存储升级失败: {}", e);
                }
                loaded_store
            } else {
                loaded_store
            }
        } else {
            MemoryStore {
                project_path: project_path_str.clone(),
                ..Default::default()
            }
        };

        // 如果配置启用了启动时去重，执行去重
        if store.config.dedup_on_startup && !store.entries.is_empty() {
            let dedup = MemoryDeduplicator::new(store.config.similarity_threshold);
            let entries = std::mem::take(&mut store.entries);
            let (deduped, stats) = dedup.deduplicate(entries);

            if stats.removed_count > 0 {
                log_debug!(
                    "启动时去重: 移除 {} 条重复记忆，保留 {} 条",
                    stats.removed_count,
                    stats.remaining_count
                );
                store.last_dedup_at = Utc::now();
            }
            store.entries = deduped;
        }

        let manager = Self {
            memory_dir,
            project_path: project_path_str,
            store,
            is_non_git_project: normalize_result.is_non_git,
        };

        // 保存存储
        manager.save_store()?;

        Ok(manager)
    }

    /// 检查是否为非 Git 项目（降级模式）
    pub fn is_non_git_project(&self) -> bool {
        self.is_non_git_project
    }

    /// 添加记忆条目
    ///
    /// 如果启用了去重检测，会检查是否与现有记忆重复
    /// 重复时静默拒绝，返回 None
    pub fn add_memory(&mut self, content: &str, category: MemoryCategory) -> Result<Option<String>> {
        let content = content.trim();
        if content.is_empty() {
            return Err(anyhow::anyhow!("记忆内容不能为空"));
        }

        // HC-10: 记忆内容大小限制
        if content.len() > self.store.config.max_entry_bytes {
            return Err(anyhow::anyhow!(
                "记忆内容超过大小限制: {} 字节 > {} 字节上限",
                content.len(),
                self.store.config.max_entry_bytes
            ));
        }

        // HC-10: 记忆条目数量限制
        if self.store.entries.len() >= self.store.config.max_entries {
            return Err(anyhow::anyhow!(
                "记忆条目数已达上限: {} / {}",
                self.store.entries.len(),
                self.store.config.max_entries
            ));
        }

        // 如果启用去重检测，检查是否重复
        if self.store.config.enable_dedup {
            let dedup = MemoryDeduplicator::new(self.store.config.similarity_threshold);
            let dup_info = dedup.check_duplicate(content, &self.store.entries);

            if dup_info.is_duplicate {
                log_debug!(
                    "记忆去重: 新内容与现有记忆相似度 {:.1}%，静默拒绝。匹配内容: {:?}",
                    dup_info.similarity * 100.0,
                    dup_info.matched_content
                );
                return Ok(None); // 静默拒绝，不报错
            }
        }

        // 创建新记忆条目
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let entry = MemoryEntry {
            id: id.clone(),
            content: content.to_string(),
            content_normalized: TextSimilarity::normalize(content),
            category,
            created_at: now,
            updated_at: now,
            version: 1,
            snapshots: Vec::new(),
        };

        self.store.entries.push(entry);
        self.save_store()?;

        log_debug!("已添加记忆: {} ({:?})", id, category);
        Ok(Some(id))
    }

    /// 获取所有记忆
    pub fn get_all_memories(&self) -> Vec<&MemoryEntry> {
        self.store.entries.iter().collect()
    }

    /// 获取指定分类的记忆
    pub fn get_memories_by_category(&self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.store.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// 手动执行去重
    ///
    /// 返回移除的记忆数量
    pub fn deduplicate(&mut self) -> Result<usize> {
        let dedup = MemoryDeduplicator::new(self.store.config.similarity_threshold);
        let (deduped, stats) = dedup.deduplicate(std::mem::take(&mut self.store.entries));

        self.store.entries = deduped;
        self.store.last_dedup_at = Utc::now();
        self.save_store()?;

        log_debug!("手动去重完成: 移除 {} 条重复记忆", stats.removed_count);
        Ok(stats.removed_count)
    }

    /// 执行去重并返回详细统计结果
    /// 用于前端可视化展示
    pub fn deduplicate_with_stats(&mut self) -> Result<super::dedup::DedupResult> {
        let dedup = MemoryDeduplicator::new(self.store.config.similarity_threshold);
        let (deduped, stats) = dedup.deduplicate(std::mem::take(&mut self.store.entries));

        self.store.entries = deduped;
        self.store.last_dedup_at = Utc::now();
        self.save_store()?;

        log_debug!("手动去重完成: 移除 {} 条重复记忆", stats.removed_count);
        Ok(stats)
    }

    /// 删除指定 ID 的记忆条目
    /// 返回被删除的记忆内容（用于确认）
    pub fn delete_memory(&mut self, memory_id: &str) -> Result<Option<String>> {
        let original_count = self.store.entries.len();
        let mut deleted_content = None;

        self.store.entries.retain(|entry| {
            if entry.id == memory_id {
                deleted_content = Some(entry.content.clone());
                false // 移除该条目
            } else {
                true
            }
        });

        if self.store.entries.len() < original_count {
            self.save_store()?;
            log_debug!("已删除记忆: {}", memory_id);
            Ok(deleted_content)
        } else {
            Ok(None) // 未找到该 ID
        }
    }

    /// SC-4: 更新指定 ID 的记忆内容
    ///
    /// 支持两种模式：
    /// - Patch: 完全替换内容
    /// - Append: 追加内容
    ///
    /// SC-6: 更新前自动创建快照用于版本回滚
    ///
    /// 返回更新后的记忆 ID（如果找到并更新）
    pub fn update_memory(
        &mut self,
        memory_id: &str,
        new_content: &str,
        append: bool,
    ) -> Result<Option<String>> {
        let new_content = new_content.trim();
        if new_content.is_empty() && !append {
            return Err(anyhow::anyhow!("更新内容不能为空"));
        }

        // 查找记忆条目索引（避免借用冲突）
        let entry_idx = self.store.entries.iter().position(|e| e.id == memory_id);

        if let Some(idx) = entry_idx {
            // SC-6: 创建快照（保留最多 5 个历史版本）
            let entry = &self.store.entries[idx];
            let snapshot = MemorySnapshot {
                version: entry.version,
                content: entry.content.clone(),
                created_at: Utc::now(),
            };

            // 更新内容
            let updated_content = if append {
                format!("{}\n{}", entry.content, new_content)
            } else {
                new_content.to_string()
            };

            // HC-10: 内容大小检查
            if updated_content.len() > self.store.config.max_entry_bytes {
                return Err(anyhow::anyhow!(
                    "更新后内容超过大小限制: {} 字节 > {} 字节上限",
                    updated_content.len(),
                    self.store.config.max_entry_bytes
                ));
            }

            // 执行更新（使用索引访问避免借用冲突）
            let entry = &mut self.store.entries[idx];
            entry.snapshots.push(snapshot);
            if entry.snapshots.len() > 5 {
                entry.snapshots.remove(0); // 移除最旧的快照
            }
            entry.content = updated_content.clone();
            entry.content_normalized = TextSimilarity::normalize(&updated_content);
            entry.updated_at = Utc::now();
            entry.version += 1; // SC-6: 递增版本号

            let new_version = entry.version; // 提取版本号，结束借用

            self.save_store()?;
            log_debug!("已更新记忆: {} (append={}, version={})", memory_id, append, new_version);
            Ok(Some(memory_id.to_string()))
        } else {
            Ok(None) // 未找到该 ID
        }
    }

    /// SC-6: 回滚记忆到指定版本
    ///
    /// 如果 target_version 为 None，回滚到上一个版本
    pub fn rollback_memory(
        &mut self,
        memory_id: &str,
        target_version: Option<u32>,
    ) -> Result<Option<u32>> {
        // 查找记忆条目索引（避免借用冲突）
        let entry_idx = self.store.entries.iter().position(|e| e.id == memory_id);

        if let Some(idx) = entry_idx {
            let entry = &self.store.entries[idx];
            if entry.snapshots.is_empty() {
                return Err(anyhow::anyhow!("记忆 {} 没有可回滚的历史版本", memory_id));
            }

            // 确定目标快照索引
            let snapshot_idx = match target_version {
                Some(ver) => entry
                    .snapshots
                    .iter()
                    .position(|s| s.version == ver)
                    .ok_or_else(|| anyhow::anyhow!("未找到版本 {} 的快照", ver))?,
                None => entry.snapshots.len() - 1, // 最近的快照
            };

            let snapshot = entry.snapshots[snapshot_idx].clone();
            let restored_version = snapshot.version;

            // 执行恢复（使用索引访问避免借用冲突）
            let entry = &mut self.store.entries[idx];
            entry.content = snapshot.content;
            entry.content_normalized = TextSimilarity::normalize(&entry.content);
            entry.updated_at = Utc::now();
            // 版本号不变，但移除已回滚的快照及之后的快照
            entry.snapshots.truncate(snapshot_idx);

            self.save_store()?;
            log_debug!(
                "已回滚记忆: {} 到版本 {}",
                memory_id,
                restored_version
            );
            Ok(Some(restored_version))
        } else {
            Ok(None)
        }
    }


    /// 获取记忆统计信息
    pub fn get_stats(&self) -> MemoryStats {
        let mut stats = MemoryStats::default();
        stats.total = self.store.entries.len();

        for entry in &self.store.entries {
            match entry.category {
                MemoryCategory::Rule => stats.rules += 1,
                MemoryCategory::Preference => stats.preferences += 1,
                MemoryCategory::Pattern => stats.patterns += 1,
                MemoryCategory::Context => stats.contexts += 1,
            }
        }

        stats
    }

    /// 获取项目信息供MCP调用方分析 - 压缩简化版本
    pub fn get_project_info(&self) -> String {
        if self.store.entries.is_empty() {
            return "📭 暂无项目记忆".to_string();
        }

        let mut compressed_info = Vec::new();

        // 按分类压缩汇总
        let categories = [
            (MemoryCategory::Rule, "规范"),
            (MemoryCategory::Preference, "偏好"),
            (MemoryCategory::Pattern, "模式"),
            (MemoryCategory::Context, "背景"),
        ];

        for (category, title) in categories.iter() {
            let memories: Vec<_> = self.get_memories_by_category(*category);
            if !memories.is_empty() {
                let items: Vec<String> = memories
                    .iter()
                    .map(|m| {
                        // 去除多余空格和换行，压缩内容
                        m.content
                            .split_whitespace()
                            .collect::<Vec<&str>>()
                            .join(" ")
                    })
                    .filter(|s| !s.is_empty())
                    .collect();

                if !items.is_empty() {
                    compressed_info.push(format!("**{}**: {}", title, items.join("; ")));
                }
            }
        }

        if compressed_info.is_empty() {
            "📭 暂无有效项目记忆".to_string()
        } else {
            format!("📚 项目记忆总览: {}", compressed_info.join(" | "))
        }
    }

    /// 获取去重配置
    pub fn config(&self) -> &MemoryConfig {
        &self.store.config
    }

    /// 更新去重配置
    pub fn update_config(&mut self, config: MemoryConfig) -> Result<()> {
        self.store.config = config;
        self.save_store()
    }

    /// 保存存储到文件（原子写入：先写临时文件，再 rename）
    fn save_store(&self) -> Result<()> {
        let store_path = self.memory_dir.join(Self::STORE_FILE);
        let json = serde_json::to_string_pretty(&self.store)?;

        // 原子写入：先写临时文件，再 rename，避免写入中断导致数据损坏
        let tmp_path = store_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, &store_path)?;
        Ok(())
    }

    // ========================================================================
    // 以下是路径处理辅助方法
    // ========================================================================

    /// 清理 Windows 扩展路径前缀用于显示
    /// 
    /// Windows 的 `canonicalize()` 会返回 `\\?\C:\...` 格式的路径，
    /// 这在错误消息和日志中显示不友好，需要清理前缀。
    fn clean_display_path(path: &Path) -> String {
        let path_str = path.to_string_lossy();
        // 处理 \\?\ 格式（Windows 扩展路径语法）
        if path_str.starts_with(r"\\?\") {
            return path_str[4..].to_string();
        }
        // 处理 //?/ 格式（canonicalize 在某些情况下返回）
        if path_str.starts_with("//?/") {
            return path_str[4..].to_string();
        }
        path_str.to_string()
    }

    /// 规范化项目路径
    /// 
    /// 支持非 Git 项目降级：
    /// - 如果检测到 Git 仓库，使用 Git 根目录
    /// - 如果未检测到 Git 仓库，使用当前目录并标记为降级模式
    fn normalize_project_path(project_path: &str) -> Result<NormalizeResult> {
        // 使用增强的路径解码和规范化功能
        let normalized_path_str = crate::mcp::utils::decode_and_normalize_path(project_path)
            .map_err(|e| anyhow::anyhow!("路径格式错误: {}", e))?;

        let path = Path::new(&normalized_path_str);

        // 转换为绝对路径
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };

        // 规范化路径（解析 . 和 .. 等）
        let canonical_path = absolute_path.canonicalize()
            .unwrap_or_else(|_| {
                // 如果 canonicalize 失败，尝试手动规范化
                Self::manual_canonicalize(&absolute_path).unwrap_or(absolute_path)
            });

        // 验证路径是否存在且为目录
        if !canonical_path.exists() {
            return Err(anyhow::anyhow!(
                "项目路径不存在: {}\n原始输入: {}\n规范化后: {}",
                Self::clean_display_path(&canonical_path),
                project_path,
                normalized_path_str
            ));
        }

        if !canonical_path.is_dir() {
            return Err(anyhow::anyhow!(
                "项目路径不是目录: {}",
                Self::clean_display_path(&canonical_path)
            ));
        }

        // 优先使用 git 根目录，否则降级使用当前目录
        if let Some(git_root) = Self::find_git_root(&canonical_path) {
            Ok(NormalizeResult {
                path: git_root,
                is_non_git: false,
            })
        } else {
            // 非 Git 项目降级：使用当前目录
            log_debug!(
                "未检测到 Git 仓库，使用项目目录作为记忆存储位置: {}",
                Self::clean_display_path(&canonical_path)
            );
            Ok(NormalizeResult {
                path: canonical_path,
                is_non_git: true,
            })
        }
    }

    /// 手动规范化路径
    fn manual_canonicalize(path: &Path) -> Result<PathBuf> {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if !components.is_empty() {
                        components.pop();
                    }
                }
                _ => {
                    components.push(component);
                }
            }
        }

        let mut result = PathBuf::new();
        for component in components {
            result.push(component);
        }

        Ok(result)
    }

    /// 查找 git 根目录
    fn find_git_root(start_path: &Path) -> Option<PathBuf> {
        let mut current_path = start_path;

        loop {
            let git_path = current_path.join(".git");
            if git_path.exists() {
                return Some(current_path.to_path_buf());
            }

            match current_path.parent() {
                Some(parent) => current_path = parent,
                None => break,
            }
        }

        None
    }
}

/// 记忆统计信息
#[derive(Debug, Default)]
pub struct MemoryStats {
    pub total: usize,
    pub rules: usize,
    pub preferences: usize,
    pub patterns: usize,
    pub contexts: usize,
}

/// 并发安全的记忆管理器包装
///
/// 使用 Arc<RwLock<MemoryManager>> 提供线程安全的读写访问，
/// 支持多 MCP 客户端同时访问同一个 MemoryManager 实例。
pub struct SharedMemoryManager {
    inner: Arc<RwLock<MemoryManager>>,
}

impl Clone for SharedMemoryManager {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl SharedMemoryManager {
    /// 创建并发安全的记忆管理器
    pub fn new(project_path: &str) -> Result<Self> {
        let manager = MemoryManager::new(project_path)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(manager)),
        })
    }

    /// 检查是否为非 Git 项目（降级模式）
    pub fn is_non_git_project(&self) -> bool {
        self.inner.read()
            .map(|m| m.is_non_git_project())
            .unwrap_or(false)
    }

    /// 添加记忆条目（写锁）
    pub fn add_memory(&self, content: &str, category: MemoryCategory) -> Result<Option<String>> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.add_memory(content, category)
    }

    /// 获取所有记忆（读锁，返回克隆数据）
    pub fn get_all_memories(&self) -> Result<Vec<MemoryEntry>> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.get_all_memories().into_iter().cloned().collect())
    }

    /// 获取指定分类的记忆（读锁）
    pub fn get_memories_by_category(&self, category: MemoryCategory) -> Result<Vec<MemoryEntry>> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.get_memories_by_category(category).into_iter().cloned().collect())
    }

    /// 手动执行去重（写锁）
    pub fn deduplicate(&self) -> Result<usize> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.deduplicate()
    }

    /// 执行去重并返回详细统计结果（写锁）
    pub fn deduplicate_with_stats(&self) -> Result<super::dedup::DedupResult> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.deduplicate_with_stats()
    }

    /// 删除指定 ID 的记忆条目（写锁）
    pub fn delete_memory(&self, memory_id: &str) -> Result<Option<String>> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.delete_memory(memory_id)
    }

    /// SC-4: 更新指定 ID 的记忆内容（写锁）
    pub fn update_memory(
        &self,
        memory_id: &str,
        new_content: &str,
        append: bool,
    ) -> Result<Option<String>> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.update_memory(memory_id, new_content, append)
    }

    /// 获取记忆统计信息（读锁）
    pub fn get_stats(&self) -> Result<MemoryStats> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.get_stats())
    }

    /// 获取项目信息供 MCP 调用方分析（读锁）
    pub fn get_project_info(&self) -> Result<String> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.get_project_info())
    }

    /// 获取去重配置（读锁）
    pub fn config(&self) -> Result<MemoryConfig> {
        let manager = self.inner.read()
            .map_err(|e| anyhow::anyhow!("获取读锁失败: {}", e))?;
        Ok(manager.config().clone())
    }

    /// 更新去重配置（写锁）
    pub fn update_config(&self, config: MemoryConfig) -> Result<()> {
        let mut manager = self.inner.write()
            .map_err(|e| anyhow::anyhow!("获取写锁失败: {}", e))?;
        manager.update_config(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 辅助函数：创建一个基于临时目录的 MemoryManager 实例
    ///
    /// 在临时目录中创建 `.git` 目录，模拟 Git 仓库根目录，
    /// 防止 `find_git_root` 向上遍历到真实项目目录导致加载已有记忆。
    fn create_test_manager() -> (TempDir, MemoryManager) {
        let temp_dir = TempDir::new().expect("创建临时目录失败");
        // 创建 .git 目录使 find_git_root 在此停止
        fs::create_dir_all(temp_dir.path().join(".git")).expect("创建 .git 目录失败");
        let manager = MemoryManager::new(temp_dir.path().to_str().unwrap())
            .expect("创建 MemoryManager 失败");
        (temp_dir, manager)
    }

    /// 辅助函数：创建管理器并预置一条记忆，返回 (TempDir, MemoryManager, memory_id)
    fn create_test_manager_with_entry() -> (TempDir, MemoryManager, String) {
        let (temp_dir, mut manager) = create_test_manager();
        let id = manager
            .add_memory("初始内容", MemoryCategory::Rule)
            .expect("添加记忆失败")
            .expect("记忆不应被去重拒绝");
        (temp_dir, manager, id)
    }

    // --- 正常路径 ---

    #[test]
    fn test_update_memory_replace() {
        // Arrange
        let (_dir, mut manager, id) = create_test_manager_with_entry();

        // Act: replace 模式完全替换内容
        let result = manager.update_memory(&id, "替换后的内容", false);

        // Assert
        assert!(result.is_ok());
        let returned_id = result.unwrap();
        assert_eq!(returned_id, Some(id.clone()));

        // 验证内容已被替换
        let all = manager.get_all_memories();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, "替换后的内容");
        // SC-6: 验证版本递增（初始版本 1 -> 更新后 2）
        assert_eq!(all[0].version, 2);
        // SC-6: 验证快照已创建（保存了旧内容）
        assert_eq!(all[0].snapshots.len(), 1);
        assert_eq!(all[0].snapshots[0].content, "初始内容");
        assert_eq!(all[0].snapshots[0].version, 1);
    }

    #[test]
    fn test_update_memory_append() {
        // Arrange
        let (_dir, mut manager, id) = create_test_manager_with_entry();

        // Act: append 模式追加内容
        let result = manager.update_memory(&id, "追加的内容", true);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(id.clone()));

        let all = manager.get_all_memories();
        assert_eq!(all.len(), 1);
        // append 模式下内容格式为 "{旧内容}\n{新内容}"
        assert_eq!(all[0].content, "初始内容\n追加的内容");
        assert_eq!(all[0].version, 2);
        assert_eq!(all[0].snapshots.len(), 1);
        assert_eq!(all[0].snapshots[0].content, "初始内容");
    }

    // --- 异常路径 ---

    #[test]
    fn test_update_memory_not_found() {
        // Arrange
        let (_dir, mut manager) = create_test_manager();

        // Act: 更新一个不存在的 memory_id
        let result = manager.update_memory("nonexistent-id", "新内容", false);

        // Assert: 未找到返回 Ok(None)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    // --- 边界条件 ---

    #[test]
    fn test_update_memory_empty_content_replace() {
        // Arrange
        let (_dir, mut manager, id) = create_test_manager_with_entry();

        // Act: replace 模式下空内容应报错
        let result = manager.update_memory(&id, "", false);

        // Assert
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("更新内容不能为空"), "错误消息应包含'更新内容不能为空'，实际: {}", err_msg);

        // 验证原始内容未被修改
        let all = manager.get_all_memories();
        assert_eq!(all[0].content, "初始内容");
        assert_eq!(all[0].version, 1);
    }

    #[test]
    fn test_update_memory_exceeds_size_limit() {
        // Arrange
        let (_dir, mut manager, id) = create_test_manager_with_entry();
        let max_bytes = manager.config().max_entry_bytes;
        // 构造一个超过 max_entry_bytes 的内容
        let oversized_content: String = "A".repeat(max_bytes + 1);

        // Act: 超大内容应报错
        let result = manager.update_memory(&id, &oversized_content, false);

        // Assert
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("大小限制"), "错误消息应包含'大小限制'，实际: {}", err_msg);

        // 验证原始内容未被修改
        let all = manager.get_all_memories();
        assert_eq!(all[0].content, "初始内容");
        assert_eq!(all[0].version, 1);
    }
}
