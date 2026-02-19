//! SC-6: 搜索缓存模块
//!
//! 提供内存级搜索结果缓存，支持 LRU 淘汰和 TTL 过期。
//! SC-8: 支持磁盘级持久化缓存（.sanshu-index/cache/）
//! 用于减少重复查询的 API 调用。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use ring::digest::{Context, SHA256};

/// 默认缓存 TTL（5 分钟）
const DEFAULT_CACHE_TTL_SECS: u64 = 5 * 60;

/// 最大缓存条目数
const MAX_CACHE_ENTRIES: usize = 100;

/// 缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    /// 缓存值
    pub value: T,
    /// 创建时间
    pub created_at: Instant,
    /// 最后访问时间（用于 LRU）
    pub last_accessed: Instant,
}

impl<T: Clone> CacheEntry<T> {
    pub fn new(value: T) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    /// 更新最后访问时间
    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// 搜索结果缓存
#[derive(Debug)]
pub struct SearchCache {
    /// 缓存存储（query -> 结果）
    entries: HashMap<String, CacheEntry<String>>,
    /// TTL 配置
    ttl: Duration,
    /// 最大条目数
    max_entries: usize,
}

impl Default for SearchCache {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
            MAX_CACHE_ENTRIES,
        )
    }
}

impl SearchCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            max_entries,
        }
    }

    /// 生成缓存键
    fn make_key(project_root: &str, query: &str) -> String {
        format!("{}:{}", project_root, query)
    }

    /// 获取缓存结果
    pub fn get(&mut self, project_root: &str, query: &str) -> Option<String> {
        let key = Self::make_key(project_root, query);

        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.is_expired(self.ttl) {
                self.entries.remove(&key);
                return None;
            }
            entry.touch();
            return Some(entry.value.clone());
        }
        None
    }

    /// 存入缓存
    pub fn put(&mut self, project_root: &str, query: &str, result: String) {
        let key = Self::make_key(project_root, query);

        // LRU 淘汰：超过最大条目数时，移除最久未访问的条目
        if self.entries.len() >= self.max_entries {
            self.evict_lru();
        }

        self.entries.insert(key, CacheEntry::new(result));
    }

    /// LRU 淘汰策略
    fn evict_lru(&mut self) {
        // 先清理过期条目
        let ttl = self.ttl;
        self.entries.retain(|_, entry| !entry.is_expired(ttl));

        // 如果仍超限，移除最久未访问的
        if self.entries.len() >= self.max_entries {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest_key);
            }
        }
    }

    /// 清除指定项目的缓存（文件变更时调用）
    pub fn invalidate_project(&mut self, project_root: &str) {
        let prefix = format!("{}:", project_root);
        self.entries.retain(|k, _| !k.starts_with(&prefix));
    }

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 获取缓存统计
    pub fn stats(&self) -> CacheStats {
        let mut expired_count = 0;
        for entry in self.entries.values() {
            if entry.is_expired(self.ttl) {
                expired_count += 1;
            }
        }

        CacheStats {
            total_entries: self.entries.len(),
            expired_entries: expired_count,
            max_entries: self.max_entries,
            ttl_secs: self.ttl.as_secs(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub max_entries: usize,
    pub ttl_secs: u64,
}

/// 全局搜索缓存实例
static SEARCH_CACHE: Lazy<RwLock<SearchCache>> = Lazy::new(|| RwLock::new(SearchCache::default()));

/// 从全局缓存获取搜索结果
pub fn get_cached_search(project_root: &str, query: &str) -> Option<String> {
    SEARCH_CACHE
        .write()
        .ok()
        .and_then(|mut cache| cache.get(project_root, query))
}

/// 将搜索结果存入全局缓存
pub fn put_cached_search(project_root: &str, query: &str, result: String) {
    if let Ok(mut cache) = SEARCH_CACHE.write() {
        cache.put(project_root, query, result);
    }
}

/// 清除指定项目的缓存
pub fn invalidate_project_cache(project_root: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.write() {
        cache.invalidate_project(project_root);
    }
}

/// 获取缓存统计
pub fn get_cache_stats() -> Option<CacheStats> {
    SEARCH_CACHE.read().ok().map(|cache| cache.stats())
}

// ============================================================================
// SC-8: 磁盘级缓存
// ============================================================================

/// 磁盘缓存 TTL（默认 24 小时）
const DISK_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// 磁盘缓存条目
#[derive(serde::Serialize, serde::Deserialize)]
struct DiskCacheEntry {
    result: String,
    created_at: i64, // Unix 时间戳
}

/// 生成缓存文件名（SHA-256 hash）
fn make_disk_cache_key(project_root: &str, query: &str) -> String {
    let mut context = Context::new(&SHA256);
    context.update(project_root.as_bytes());
    context.update(b"|");
    context.update(query.as_bytes());
    let digest = context.finish();
    hex::encode(digest.as_ref())
}

/// 获取磁盘缓存目录
fn get_disk_cache_dir(project_root: &str) -> PathBuf {
    Path::new(project_root).join(".sanshu-index").join("cache")
}

/// SC-8: 从磁盘缓存获取搜索结果
pub fn get_disk_cached_search(project_root: &str, query: &str) -> Option<String> {
    let cache_dir = get_disk_cache_dir(project_root);
    let key = make_disk_cache_key(project_root, query);
    let cache_file = cache_dir.join(format!("{}.json", key));

    if !cache_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&cache_file).ok()?;
    let entry: DiskCacheEntry = serde_json::from_str(&content).ok()?;

    // 检查 TTL
    let now = chrono::Utc::now().timestamp();
    if now - entry.created_at > DISK_CACHE_TTL_SECS as i64 {
        // 过期，删除缓存文件
        let _ = fs::remove_file(&cache_file);
        return None;
    }

    Some(entry.result)
}

/// SC-8: 将搜索结果存入磁盘缓存
pub fn put_disk_cached_search(project_root: &str, query: &str, result: &str) {
    let cache_dir = get_disk_cache_dir(project_root);

    // 确保缓存目录存在
    if fs::create_dir_all(&cache_dir).is_err() {
        return;
    }

    let key = make_disk_cache_key(project_root, query);
    let cache_file = cache_dir.join(format!("{}.json", key));

    let entry = DiskCacheEntry {
        result: result.to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };

    if let Ok(json) = serde_json::to_string(&entry) {
        let _ = fs::write(&cache_file, json);
    }
}

/// SC-8: 三级缓存查询（内存 -> 磁盘 -> None）
pub fn get_cached_search_with_disk(project_root: &str, query: &str) -> Option<String> {
    // 1. 先查内存缓存
    if let Some(result) = get_cached_search(project_root, query) {
        return Some(result);
    }

    // 2. 再查磁盘缓存
    if let Some(result) = get_disk_cached_search(project_root, query) {
        // 将磁盘缓存提升到内存
        put_cached_search(project_root, query, result.clone());
        return Some(result);
    }

    None
}

/// SC-8: 存入缓存（内存 + 磁盘）
pub fn put_cached_search_with_disk(project_root: &str, query: &str, result: String) {
    // 存入内存缓存
    put_cached_search(project_root, query, result.clone());
    // 存入磁盘缓存
    put_disk_cached_search(project_root, query, &result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_get() {
        let mut cache = SearchCache::default();
        cache.put("/project", "test query", "result".to_string());

        let result = cache.get("/project", "test query");
        assert_eq!(result, Some("result".to_string()));
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = SearchCache::default();
        let result = cache.get("/project", "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidate_project() {
        let mut cache = SearchCache::default();
        cache.put("/project1", "query1", "result1".to_string());
        cache.put("/project1", "query2", "result2".to_string());
        cache.put("/project2", "query1", "result3".to_string());

        cache.invalidate_project("/project1");

        assert!(cache.get("/project1", "query1").is_none());
        assert!(cache.get("/project1", "query2").is_none());
        assert!(cache.get("/project2", "query1").is_some());
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = SearchCache::new(Duration::from_secs(3600), 3);

        cache.put("/p", "q1", "r1".to_string());
        cache.put("/p", "q2", "r2".to_string());
        cache.put("/p", "q3", "r3".to_string());

        // 访问 q1 和 q2，使 q3 成为最久未访问的
        cache.get("/p", "q1");
        cache.get("/p", "q2");

        // 添加新条目，触发 LRU 淘汰
        cache.put("/p", "q4", "r4".to_string());

        // q3 应该被淘汰
        assert!(cache.get("/p", "q3").is_none());
        assert!(cache.get("/p", "q1").is_some());
    }
}
