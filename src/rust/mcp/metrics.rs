//! SC-14: MCP 可观测性指标模块
//!
//! 提供工具调用的性能指标收集和统计。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use once_cell::sync::Lazy;

/// 最大延迟样本数
const MAX_LATENCY_SAMPLES: usize = 1000;

/// MCP 指标收集器
pub struct McpMetrics {
    /// 工具调用总数
    pub tool_calls: AtomicU64,
    /// 缓存命中数
    pub cache_hits: AtomicU64,
    /// 缓存未命中数
    pub cache_misses: AtomicU64,
    /// API 错误数
    pub api_errors: AtomicU64,
    /// 延迟样本（毫秒）
    pub latency_samples: RwLock<Vec<u64>>,
    /// 每个工具的调用计数
    pub tool_call_counts: RwLock<HashMap<String, u64>>,
    /// 每个工具的错误计数
    pub tool_error_counts: RwLock<HashMap<String, u64>>,
}

impl Default for McpMetrics {
    fn default() -> Self {
        Self {
            tool_calls: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            api_errors: AtomicU64::new(0),
            latency_samples: RwLock::new(Vec::with_capacity(MAX_LATENCY_SAMPLES)),
            tool_call_counts: RwLock::new(HashMap::new()),
            tool_error_counts: RwLock::new(HashMap::new()),
        }
    }
}

impl McpMetrics {
    /// 记录工具调用
    pub fn record_call(&self, tool: &str, latency_ms: u64) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);

        // 更新工具调用计数
        if let Ok(mut counts) = self.tool_call_counts.write() {
            *counts.entry(tool.to_string()).or_insert(0) += 1;
        }

        // 记录延迟样本
        if let Ok(mut samples) = self.latency_samples.write() {
            if samples.len() >= MAX_LATENCY_SAMPLES {
                samples.remove(0);
            }
            samples.push(latency_ms);
        }
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 API 错误
    pub fn record_api_error(&self, tool: &str) {
        self.api_errors.fetch_add(1, Ordering::Relaxed);

        if let Ok(mut counts) = self.tool_error_counts.write() {
            *counts.entry(tool.to_string()).or_insert(0) += 1;
        }
    }

    /// 获取指标摘要
    pub fn summary(&self) -> MetricsSummary {
        let total_calls = self.tool_calls.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        let api_errors = self.api_errors.load(Ordering::Relaxed);

        // 计算缓存命中率
        let total_cache_ops = cache_hits + cache_misses;
        let cache_hit_rate = if total_cache_ops > 0 {
            cache_hits as f64 / total_cache_ops as f64
        } else {
            0.0
        };

        // 计算延迟百分位数
        let (p50, p95, p99) = self.calculate_percentiles();

        // 获取工具调用统计
        let tool_stats = self
            .tool_call_counts
            .read()
            .map(|c| c.clone())
            .unwrap_or_default();

        MetricsSummary {
            total_calls,
            cache_hits,
            cache_misses,
            cache_hit_rate,
            api_errors,
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
            tool_call_counts: tool_stats,
        }
    }

    /// 计算延迟百分位数
    fn calculate_percentiles(&self) -> (u64, u64, u64) {
        let samples = match self.latency_samples.read() {
            Ok(s) => s.clone(),
            Err(_) => return (0, 0, 0),
        };

        if samples.is_empty() {
            return (0, 0, 0);
        }

        let mut sorted = samples;
        sorted.sort_unstable();

        let len = sorted.len();
        // 防止索引越界：使用 min(计算值, len - 1)
        let p50_idx = (len * 50 / 100).min(len - 1);
        let p95_idx = (len * 95 / 100).min(len - 1);
        let p99_idx = (len * 99 / 100).min(len - 1);

        (sorted[p50_idx], sorted[p95_idx], sorted[p99_idx])
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.tool_calls.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.api_errors.store(0, Ordering::Relaxed);

        if let Ok(mut samples) = self.latency_samples.write() {
            samples.clear();
        }

        if let Ok(mut counts) = self.tool_call_counts.write() {
            counts.clear();
        }

        if let Ok(mut counts) = self.tool_error_counts.write() {
            counts.clear();
        }
    }
}

/// 指标摘要
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// 总调用次数
    pub total_calls: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// API 错误次数
    pub api_errors: u64,
    /// P50 延迟（毫秒）
    pub latency_p50_ms: u64,
    /// P95 延迟（毫秒）
    pub latency_p95_ms: u64,
    /// P99 延迟（毫秒）
    pub latency_p99_ms: u64,
    /// 每个工具的调用次数
    pub tool_call_counts: HashMap<String, u64>,
}

/// 全局指标实例
pub static MCP_METRICS: Lazy<McpMetrics> = Lazy::new(McpMetrics::default);

/// 获取全局指标摘要
pub fn get_metrics_summary() -> MetricsSummary {
    MCP_METRICS.summary()
}

/// 重置全局指标
pub fn reset_metrics() {
    MCP_METRICS.reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = McpMetrics::default();

        metrics.record_call("zhi", 100);
        metrics.record_call("ji", 200);
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_api_error("enhance");

        let summary = metrics.summary();
        assert_eq!(summary.total_calls, 2);
        assert_eq!(summary.cache_hits, 1);
        assert_eq!(summary.cache_misses, 1);
        assert_eq!(summary.api_errors, 1);
    }

    #[test]
    fn test_cache_hit_rate() {
        let metrics = McpMetrics::default();

        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();

        let summary = metrics.summary();
        assert!((summary.cache_hit_rate - 0.666).abs() < 0.01);
    }
}
