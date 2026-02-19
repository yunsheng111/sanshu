//! SC-13: MCP 配置热更新模块
//!
//! 提供配置文件监听和缓存更新机制，避免每次调用都读取文件。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use once_cell::sync::Lazy;

use crate::config::load_standalone_config;
use crate::log_debug;

/// 配置缓存刷新间隔（秒）
const CACHE_REFRESH_INTERVAL_SECS: u64 = 5;

/// 热更新配置缓存
struct HotReloadCache {
    /// 工具启用状态
    tools: HashMap<String, bool>,
    /// 最后更新时间
    last_updated: Instant,
    /// 配置文件路径（用于监听）
    config_path: Option<PathBuf>,
}

impl Default for HotReloadCache {
    fn default() -> Self {
        let (tools, config_path) = match load_standalone_config() {
            Ok(config) => (config.mcp_config.tools, None), // TODO: 添加配置文件路径
            Err(_) => (crate::config::default_mcp_tools(), None),
        };

        Self {
            tools,
            last_updated: Instant::now(),
            config_path,
        }
    }
}

/// 全局配置缓存
static CONFIG_CACHE: Lazy<Arc<RwLock<HotReloadCache>>> =
    Lazy::new(|| Arc::new(RwLock::new(HotReloadCache::default())));

/// SC-13: 检查工具是否启用（带缓存和热更新）
///
/// 使用缓存机制，每 5 秒检查配置文件是否更新
pub fn is_tool_enabled_cached(tool_name: &str) -> bool {
    let cache = CONFIG_CACHE.read().ok();

    if let Some(cache) = cache {
        // 检查缓存是否过期
        if cache.last_updated.elapsed().as_secs() < CACHE_REFRESH_INTERVAL_SECS {
            return cache.tools.get(tool_name).copied().unwrap_or(true);
        }
    }

    // 缓存过期或不可用，重新加载配置
    refresh_config_cache();

    CONFIG_CACHE
        .read()
        .ok()
        .and_then(|c| c.tools.get(tool_name).copied())
        .unwrap_or(true)
}

/// SC-13: 刷新配置缓存
pub fn refresh_config_cache() {
    if let Ok(mut cache) = CONFIG_CACHE.write() {
        match load_standalone_config() {
            Ok(config) => {
                cache.tools = config.mcp_config.tools;
                cache.last_updated = Instant::now();
                log_debug!("MCP 配置缓存已刷新");
            }
            Err(e) => {
                log_debug!("刷新配置缓存失败: {}", e);
                // 保留旧缓存，仅更新时间戳避免频繁重试
                cache.last_updated = Instant::now();
            }
        }
    }
}

/// SC-13: 获取当前配置快照
pub fn get_current_tools_config() -> HashMap<String, bool> {
    CONFIG_CACHE
        .read()
        .ok()
        .map(|c| c.tools.clone())
        .unwrap_or_else(crate::config::default_mcp_tools)
}

/// SC-13: 强制更新工具配置（从外部调用）
pub fn update_tool_enabled(tool_name: &str, enabled: bool) {
    if let Ok(mut cache) = CONFIG_CACHE.write() {
        cache.tools.insert(tool_name.to_string(), enabled);
        log_debug!("工具 {} 状态已更新为: {}", tool_name, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_cache_default() {
        // 默认情况下工具应该启用
        assert!(is_tool_enabled_cached("nonexistent_tool"));
    }

    #[test]
    fn test_update_tool_enabled() {
        update_tool_enabled("test_tool", false);
        // 注意：由于全局状态，这个测试可能受其他测试影响
    }
}
