//! SC-7: Enhance 缓存模块
//!
//! 提供提示词增强结果的内存缓存，支持 LRU 淘汰和 TTL 过期。
//! 用于减少重复增强请求的 API 调用。

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;

/// 默认缓存 TTL（10 分钟）
const DEFAULT_CACHE_TTL_SECS: u64 = 10 * 60;

/// 最大缓存条目数
const MAX_CACHE_ENTRIES: usize = 50;

/// 缓存条目
#[derive(Debug, Clone)]
pub struct EnhanceCacheEntry {
    /// 增强后的提示词
    pub enhanced_prompt: String,
    /// 创建时间
    pub created_at: Instant,
    /// 最后访问时间（用于 LRU）
    pub last_accessed: Instant,
}

impl EnhanceCacheEntry {
    pub fn new(enhanced_prompt: String) -> Self {
        let now = Instant::now();
        Self {
            enhanced_prompt,
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

/// Enhance 缓存
#[derive(Debug)]
pub struct EnhanceCache {
    /// 缓存存储（原始提示词 hash -> 增强结果）
    entries: HashMap<String, EnhanceCacheEntry>,
    /// TTL 配置
    ttl: Duration,
    /// 最大条目数
    max_entries: usize,
}

impl Default for EnhanceCache {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
            MAX_CACHE_ENTRIES,
        )
    }
}

impl EnhanceCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            max_entries,
        }
    }

    /// 生成缓存键（使用 SHA-256 hash）
    fn make_key(prompt: &str, project_path: Option<&str>) -> String {
        use ring::digest::{Context, SHA256};
        let mut context = Context::new(&SHA256);
        context.update(prompt.as_bytes());
        if let Some(path) = project_path {
            context.update(b"|");
            context.update(path.as_bytes());
        }
        let digest = context.finish();
        hex::encode(digest.as_ref())
    }

    /// 获取缓存结果
    pub fn get(&mut self, prompt: &str, project_path: Option<&str>) -> Option<String> {
        let key = Self::make_key(prompt, project_path);

        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.is_expired(self.ttl) {
                self.entries.remove(&key);
                return None;
            }
            entry.touch();
            return Some(entry.enhanced_prompt.clone());
        }
        None
    }

    /// 存入缓存
    pub fn put(&mut self, prompt: &str, project_path: Option<&str>, enhanced: String) {
        let key = Self::make_key(prompt, project_path);

        // LRU 淘汰：超过最大条目数时，移除最久未访问的条目
        if self.entries.len() >= self.max_entries {
            self.evict_lru();
        }

        self.entries.insert(key, EnhanceCacheEntry::new(enhanced));
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

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 获取缓存统计
    pub fn stats(&self) -> EnhanceCacheStats {
        let mut expired_count = 0;
        for entry in self.entries.values() {
            if entry.is_expired(self.ttl) {
                expired_count += 1;
            }
        }

        EnhanceCacheStats {
            total_entries: self.entries.len(),
            expired_entries: expired_count,
            max_entries: self.max_entries,
            ttl_secs: self.ttl.as_secs(),
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct EnhanceCacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub max_entries: usize,
    pub ttl_secs: u64,
}

/// 全局 Enhance 缓存实例
static ENHANCE_CACHE: Lazy<RwLock<EnhanceCache>> =
    Lazy::new(|| RwLock::new(EnhanceCache::default()));

/// 从全局缓存获取增强结果
pub fn get_cached_enhance(prompt: &str, project_path: Option<&str>) -> Option<String> {
    ENHANCE_CACHE
        .write()
        .ok()
        .and_then(|mut cache| cache.get(prompt, project_path))
}

/// 将增强结果存入全局缓存
pub fn put_cached_enhance(prompt: &str, project_path: Option<&str>, enhanced: String) {
    if let Ok(mut cache) = ENHANCE_CACHE.write() {
        cache.put(prompt, project_path, enhanced);
    }
}

/// 清除所有缓存
pub fn clear_enhance_cache() {
    if let Ok(mut cache) = ENHANCE_CACHE.write() {
        cache.clear();
    }
}

/// 获取缓存统计
pub fn get_enhance_cache_stats() -> Option<EnhanceCacheStats> {
    ENHANCE_CACHE.read().ok().map(|cache| cache.stats())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_cache_put_get() {
        let mut cache = EnhanceCache::default();
        cache.put("test prompt", Some("/project"), "enhanced".to_string());

        let result = cache.get("test prompt", Some("/project"));
        assert_eq!(result, Some("enhanced".to_string()));
    }

    #[test]
    fn test_enhance_cache_miss() {
        let mut cache = EnhanceCache::default();
        let result = cache.get("nonexistent", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_enhance_cache_different_project() {
        let mut cache = EnhanceCache::default();
        cache.put("prompt", Some("/p1"), "result1".to_string());
        cache.put("prompt", Some("/p2"), "result2".to_string());

        assert_eq!(cache.get("prompt", Some("/p1")), Some("result1".to_string()));
        assert_eq!(cache.get("prompt", Some("/p2")), Some("result2".to_string()));
    }

    #[test]
    fn test_enhance_cache_lru_eviction() {
        let mut cache = EnhanceCache::new(Duration::from_secs(3600), 3);

        cache.put("p1", None, "r1".to_string());
        cache.put("p2", None, "r2".to_string());
        cache.put("p3", None, "r3".to_string());

        // 访问 p1 和 p2，使 p3 成为最久未访问的
        cache.get("p1", None);
        cache.get("p2", None);

        // 添加新条目，触发 LRU 淘汰
        cache.put("p4", None, "r4".to_string());

        // p3 应该被淘汰
        assert!(cache.get("p3", None).is_none());
        assert!(cache.get("p1", None).is_some());
    }

    // ========================================================================
    // 性能基准测试
    // ========================================================================

    /// 性能基准：缓存写入吞吐量
    #[test]
    fn test_perf_cache_put_throughput() {
        let mut cache = EnhanceCache::new(Duration::from_secs(3600), 1000);

        let start = std::time::Instant::now();
        for i in 0..1000 {
            cache.put(
                &format!("prompt_{}", i),
                Some("/project"),
                format!("enhanced result for prompt {}", i),
            );
        }
        let elapsed = start.elapsed();
        println!("[Cache PUT] 1000 次写入 => {:?} (avg {:?}/op)", elapsed, elapsed / 1000);
        assert!(elapsed.as_millis() < 500, "缓存写入过慢: {:?}", elapsed);
    }

    /// 性能基准：缓存命中读取延迟
    #[test]
    fn test_perf_cache_get_hit() {
        let mut cache = EnhanceCache::new(Duration::from_secs(3600), 500);
        // 预填充
        for i in 0..500 {
            cache.put(&format!("p{}", i), None, format!("r{}", i));
        }

        let start = std::time::Instant::now();
        let mut hits = 0;
        for i in 0..500 {
            if cache.get(&format!("p{}", i), None).is_some() {
                hits += 1;
            }
        }
        let elapsed = start.elapsed();
        println!(
            "[Cache GET] 500 次读取 => {:?} (avg {:?}/op), 命中率={:.1}%",
            elapsed, elapsed / 500, (hits as f64 / 500.0) * 100.0
        );
        assert_eq!(hits, 500, "预填充后应全部命中");
        assert!(elapsed.as_millis() < 200, "缓存读取过慢: {:?}", elapsed);
    }

    /// 性能基准：LRU 淘汰压力测试
    #[test]
    fn test_perf_cache_lru_pressure() {
        let mut cache = EnhanceCache::new(Duration::from_secs(3600), 50);

        // 写入 200 条，触发 150 次 LRU 淘汰
        let start = std::time::Instant::now();
        for i in 0..200 {
            cache.put(&format!("p{}", i), None, format!("r{}", i));
        }
        let elapsed = start.elapsed();
        let stats = cache.stats();
        println!(
            "[Cache LRU] 200 次写入(max=50) => {:?}, 最终条目={}, 淘汰次数≈150",
            elapsed, stats.total_entries
        );
        assert_eq!(stats.total_entries, 50, "应保持最大条目数");
        assert!(elapsed.as_millis() < 1000, "LRU 淘汰过慢: {:?}", elapsed);
    }
}
