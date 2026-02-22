//! 记忆管理工具模块
//!
//! 提供全局记忆管理功能，用于存储和管理重要的开发规范、用户偏好和最佳实践
//!
//! ## 模块结构
//! - `types` - 数据类型定义（MemoryEntry, MemoryStore, MemoryConfig）
//! - `similarity` - 文本相似度算法
//! - `dedup` - 去重检测器
//! - `migration` - 旧格式迁移
//! - `manager` - 核心管理器
//! - `mcp` - MCP 接口
//! - `write_guard` - 写入守卫（P0 新增）
//! - `registry` - 全局管理器池（P0 新增）
//! - `uri_path` - URI 路径解析验证（P1 新增）
//! - `vitality` - 活力衰减引擎（P1 新增）
//! - `fts_index` - FTS5 Sidecar 索引（P2 新增）
//! - `summary` - 摘要自动生成（P2 新增）
//! - `observation_store` - 会话观察存储（P3 新增）

pub mod types;
pub mod similarity;
pub mod dedup;
pub mod migration;
pub mod manager;
pub mod mcp;
pub mod write_guard;   // P0 新增
pub mod registry;      // P0 新增
pub mod uri_path;      // P1 新增
pub mod vitality;      // P1 新增
pub mod fts_index;     // P2 新增
pub mod fts_actor;     // P2 新增：FTS5 Actor 模式
pub mod consistency;   // T6 新增：FTS 与 JSON 一致性验证
pub mod summary;       // P2 新增
pub mod summary_service;  // P2 新增：摘要生成服务（Provider 链 + 超时保护）
pub mod observation_store;  // P3 新增

#[cfg(test)]
mod mcp_integration_test;  // Task 2 集成测试

// 重新导出主要类型和功能
pub use manager::MemoryManager;
pub use manager::SharedMemoryManager;
pub use types::{MemoryEntry, MemoryCategory, MemoryMetadata, MemoryStore, MemoryConfig, DomainInfo};
pub use mcp::MemoryTool;
// T5: 导出搜索相关 DTO 类型
pub use mcp::{SearchMode, SearchResult, FtsSearchItem};
pub use similarity::TextSimilarity;
pub use dedup::{MemoryDeduplicator, DuplicateInfo, DedupResult};
pub use migration::{MemoryMigrator, MigrationResult};
pub use write_guard::{WriteGuard, WriteGuardAction, WriteGuardResult};
pub use registry::REGISTRY;
