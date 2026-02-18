// 提示词增强模块
// 支持 Ollama / OpenAI 兼容 / 规则引擎三级降级链

pub mod types;
pub mod core;
pub mod history;
pub mod commands;
pub mod mcp;
pub mod utils;
pub mod chat_client;
pub mod rule_engine;
pub mod provider_factory;

// 重新导出工具以便访问
pub use mcp::EnhanceTool;
pub use types::*;
pub use core::*;
pub use history::ChatHistoryManager;
pub use utils::mask_api_key;
pub use chat_client::{ChatClient, ChatProvider, Message};
pub use rule_engine::{RuleEnhancer, EnhanceContext};
pub use provider_factory::{build_enhance_client, build_enhance_client_async};
