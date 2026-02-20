//! MemoryManagerRegistry 全局管理器池
//!
//! HC-13: 使用 Weak<RwLock<MemoryManager>> 引用 + TTL 回收 + 池大小上限
//! SC-22: 懒加载，首次请求时创建管理器并缓存

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};
use anyhow::Result;
use once_cell::sync::Lazy;

use super::manager::{MemoryManager, SharedMemoryManager};
use crate::log_debug;

/// 全局 Registry 单例
pub static REGISTRY: Lazy<MemoryManagerRegistry> = Lazy::new(MemoryManagerRegistry::new);

/// TTL 默认值：30 分钟
const DEFAULT_TTL_SECS: u64 = 30 * 60;

/// 池大小上限：16
const DEFAULT_POOL_SIZE: usize = 16;

/// 清理间隔：5 分钟
const CLEANUP_INTERVAL_SECS: u64 = 5 * 60;

/// 弱引用条目
struct WeakEntry {
    /// 管理器的弱引用
    weak: Weak<RwLock<MemoryManager>>,
    /// 最后访问时间
    last_access: Instant,
}

/// 全局管理器池
pub struct MemoryManagerRegistry {
    /// 项目路径 -> 弱引用条目
    pool: RwLock<HashMap<String, WeakEntry>>,
    /// TTL 持续时间
    ttl: Duration,
    /// 池大小上限
    max_size: usize,
    /// 上次清理时间
    last_cleanup: RwLock<Instant>,
}

impl MemoryManagerRegistry {
    /// 创建新的 Registry
    fn new() -> Self {
        Self {
            pool: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            max_size: DEFAULT_POOL_SIZE,
            last_cleanup: RwLock::new(Instant::now()),
        }
    }

    /// SC-22: 获取或创建 SharedMemoryManager
    ///
    /// 1. 规范化 project_path
    /// 2. 检查缓存中是否有有效的管理器
    /// 3. 如果有且 Weak::upgrade 成功，更新 last_access 并返回
    /// 4. 如果没有或已失效，创建新的 SharedMemoryManager 并缓存
    pub fn get_or_create(&self, project_path: &str) -> Result<SharedMemoryManager> {
        let canonical = Self::canonical_path(project_path);

        // 先尝试读锁查找
        {
            let pool = self.pool.read()
                .map_err(|e| anyhow::anyhow!("Registry 读锁失败: {}", e))?;

            if let Some(entry) = pool.get(&canonical) {
                if let Some(arc) = entry.weak.upgrade() {
                    // 缓存命中
                    log_debug!("[Registry] 缓存命中: {}", canonical);
                    drop(pool);
                    // 更新 last_access
                    self.touch(&canonical);
                    return Ok(SharedMemoryManager::from_arc(arc));
                }
            }
        }

        // 缓存未命中或已失效，需要创建新的
        self.maybe_cleanup();

        let manager = SharedMemoryManager::new(project_path)?;
        let arc = manager.inner_arc();

        let mut pool = self.pool.write()
            .map_err(|e| anyhow::anyhow!("Registry 写锁失败: {}", e))?;

        // HC-13: 池大小上限检查
        if pool.len() >= self.max_size {
            // 移除最久未访问的条目
            Self::evict_oldest(&mut pool);
        }

        pool.insert(canonical.clone(), WeakEntry {
            weak: Arc::downgrade(&arc),
            last_access: Instant::now(),
        });

        log_debug!("[Registry] 创建新管理器: {}, pool_size={}", canonical, pool.len());
        Ok(manager)
    }

    /// 更新条目的最后访问时间
    fn touch(&self, canonical: &str) {
        if let Ok(mut pool) = self.pool.write() {
            if let Some(entry) = pool.get_mut(canonical) {
                entry.last_access = Instant::now();
            }
        }
    }

    /// 定期清理过期条目
    fn maybe_cleanup(&self) {
        let should_cleanup = {
            let last = self.last_cleanup.read().ok();
            last.map_or(true, |t| t.elapsed() > Duration::from_secs(CLEANUP_INTERVAL_SECS))
        };

        if !should_cleanup {
            return;
        }

        if let Ok(mut pool) = self.pool.write() {
            let before = pool.len();
            pool.retain(|path, entry| {
                // 移除：Weak 已失效 或 TTL 过期
                let alive = entry.weak.upgrade().is_some();
                let fresh = entry.last_access.elapsed() < self.ttl;
                let keep = alive && fresh;
                if !keep {
                    log_debug!("[Registry] 清理过期条目: {} (alive={}, fresh={})", path, alive, fresh);
                }
                keep
            });
            let after = pool.len();
            if before != after {
                log_debug!("[Registry] 清理完成: {} -> {} 条目", before, after);
            }
        }

        if let Ok(mut last) = self.last_cleanup.write() {
            *last = Instant::now();
        }
    }

    /// 移除最久未访问的条目
    fn evict_oldest(pool: &mut HashMap<String, WeakEntry>) {
        if let Some(oldest_key) = pool.iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone())
        {
            log_debug!("[Registry] 驱逐最久未访问: {}", oldest_key);
            pool.remove(&oldest_key);
        }
    }

    /// 规范化项目路径为 canonical key
    fn canonical_path(project_path: &str) -> String {
        // 统一路径分隔符为 /，去除尾部 /，转小写（Windows 不区分大小写）
        let normalized = project_path
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase();
        normalized
    }

    /// 获取当前池大小（用于监控）
    pub fn pool_size(&self) -> usize {
        self.pool.read().map(|p| p.len()).unwrap_or(0)
    }

    /// 仅暴露给测试：规范化路径（用于单元测试验证）
    #[cfg(test)]
    pub(crate) fn canonical_path_for_test(path: &str) -> String {
        Self::canonical_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- 正常路径：canonical_path 规范化 ---

    #[test]
    fn test_canonical_path_forward_slash() {
        // 正斜杠路径保持不变（仅转小写和去尾部斜杠）
        let result = MemoryManagerRegistry::canonical_path_for_test("C:/Users/test/project");
        assert_eq!(result, "c:/users/test/project");
    }

    #[test]
    fn test_canonical_path_backslash_to_forward() {
        // Windows 反斜杠应转为正斜杠
        let result = MemoryManagerRegistry::canonical_path_for_test(r"C:\Users\test\project");
        assert_eq!(result, "c:/users/test/project");
    }

    #[test]
    fn test_canonical_path_trailing_slash_removed() {
        // 尾部斜杠应被去除
        let result = MemoryManagerRegistry::canonical_path_for_test("C:/Users/test/project/");
        assert_eq!(result, "c:/users/test/project");
    }

    #[test]
    fn test_canonical_path_trailing_backslash_removed() {
        // 尾部反斜杠也应被去除（先转为正斜杠再去除）
        let result = MemoryManagerRegistry::canonical_path_for_test(r"C:\Users\test\project\");
        assert_eq!(result, "c:/users/test/project");
    }

    #[test]
    fn test_canonical_path_case_insensitive() {
        // Windows 不区分大小写，统一转小写
        let upper = MemoryManagerRegistry::canonical_path_for_test(r"C:\Users\TEST\Project");
        let lower = MemoryManagerRegistry::canonical_path_for_test(r"c:\users\test\project");
        assert_eq!(upper, lower);
    }

    #[test]
    fn test_canonical_path_mixed_separators() {
        // 混合分隔符
        let result = MemoryManagerRegistry::canonical_path_for_test(r"C:\Users/test\project/");
        assert_eq!(result, "c:/users/test/project");
    }

    // --- 边界条件 ---

    #[test]
    fn test_canonical_path_empty_string() {
        let result = MemoryManagerRegistry::canonical_path_for_test("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_canonical_path_single_char() {
        let result = MemoryManagerRegistry::canonical_path_for_test("A");
        assert_eq!(result, "a");
    }

    #[test]
    fn test_canonical_path_only_slashes() {
        // 纯斜杠路径
        let result = MemoryManagerRegistry::canonical_path_for_test("///");
        assert_eq!(result, "");
    }

    // --- pool_size 功能测试（不依赖文件系统） ---

    #[test]
    fn test_new_registry_pool_empty() {
        // 新创建的 Registry 池为空
        let registry = MemoryManagerRegistry::new();
        assert_eq!(registry.pool_size(), 0);
    }
}
