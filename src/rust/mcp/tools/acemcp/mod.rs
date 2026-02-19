// Acemcp工具模块
// 用于代码库索引和语义搜索的MCP工具

pub mod mcp;
pub mod types;
pub mod commands;
pub mod watcher;
pub mod embedding_client;
pub mod local_index;
pub mod hybrid_search;
pub mod cache;

// 重新导出工具以便访问
pub use mcp::AcemcpTool;
pub use watcher::get_watcher_manager;
pub use embedding_client::{EmbeddingClient, EmbeddingProvider};
pub use local_index::{LocalIndexManager, SearchResult};
pub use hybrid_search::{HybridSearcher, HybridResult};
pub use cache::{get_cached_search, put_cached_search, invalidate_project_cache, get_cache_stats};
