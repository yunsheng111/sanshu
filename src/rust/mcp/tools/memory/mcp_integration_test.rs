//! Task 2 MCP 接口集成测试
//!
//! 验证 5 个新增操作的 MCP 接口完整性：
//! 1. 域列表 (get_domain_list)
//! 2. 清理候选 (get_cleanup_candidates)
//! 3. 活力趋势 (get_vitality_trend)
//! 4. 快照列表 (get_memory_snapshots)
//! 5. 回滚快照 (rollback_to_snapshot)

#[cfg(test)]
mod tests {
    use super::super::mcp::MemoryTool;
    use crate::mcp::JiyiRequest;
    use tempfile::TempDir;
    use std::fs;

    /// 辅助函数：从 Content 中提取文本
    fn extract_text(content: &rmcp::model::Content) -> &str {
        match &content.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("返回内容不是文本类型"),
        }
    }

    /// 辅助函数：创建测试用的临时 Git 项目目录
    fn create_test_project() -> TempDir {
        let temp_dir = TempDir::new().expect("创建临时目录失败");
        // 创建 .git 目录使 find_git_root 在此停止
        fs::create_dir_all(temp_dir.path().join(".git")).expect("创建 .git 目录失败");
        temp_dir
    }

    /// 辅助函数：创建 JiyiRequest
    fn make_request(action: &str, project_path: &str) -> JiyiRequest {
        JiyiRequest {
            action: action.to_string(),
            project_path: project_path.to_string(),
            content: String::new(),
            category: "context".to_string(),
            config: None,
            memory_id: None,
            update_mode: None,
            uri_path: None,
            tags: None,
            cleanup_ids: None,
            verbose: None,
            page: None,
            page_size: None,
            summary_only: None,
            target_version: None,
        }
    }

    #[tokio::test]
    async fn test_domain_list_operation() {
        // Arrange: 创建项目并添加记忆
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // 添加记忆
        let mut add_req = make_request("记忆", project_path);
        add_req.content = "核心架构".to_string();
        add_req.category = "rule".to_string();
        let add_result = MemoryTool::jiyi(add_req).await;
        assert!(add_result.is_ok(), "添加记忆失败");

        // Act: 调用域列表操作
        let list_req = make_request("域列表", project_path);
        let result = MemoryTool::jiyi(list_req).await;

        // Assert: 验证返回结果
        assert!(result.is_ok(), "域列表操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        // 验证 JSON 格式
        let content = &call_result.content[0];
        let text = extract_text(content);
        let json: serde_json::Value = serde_json::from_str(text).expect("解析 JSON 失败");
        assert!(json.get("total_domains").is_some(), "缺少 total_domains 字段");
        assert!(json.get("domains").is_some(), "缺少 domains 字段");
    }

    #[tokio::test]
    async fn test_cleanup_candidates_operation() {
        // Arrange: 创建项目
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // Act: 调用清理候选操作（空项目应返回空列表）
        let req = make_request("清理候选", project_path);
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回结果
        assert!(result.is_ok(), "清理候选操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        // 验证 JSON 格式
        let content = &call_result.content[0];
        let text = extract_text(content);
        let json: serde_json::Value = serde_json::from_str(text).expect("解析 JSON 失败");
        assert!(json.get("total_candidates").is_some(), "缺少 total_candidates 字段");
        assert!(json.get("candidates").is_some(), "缺少 candidates 字段");
        assert!(json.get("hint").is_some(), "缺少 hint 字段");
    }

    #[tokio::test]
    async fn test_vitality_trend_operation_not_found() {
        // Arrange: 创建项目
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // Act: 调用活力趋势操作（不存在的 ID）
        let mut req = make_request("活力趋势", project_path);
        req.memory_id = Some("nonexistent-id".to_string());
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回结果（应返回未找到提示）
        assert!(result.is_ok(), "活力趋势操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        let content = &call_result.content[0];
        let text = extract_text(content);
        assert!(text.contains("未找到"), "应返回未找到提示");
    }

    #[tokio::test]
    async fn test_vitality_trend_operation_with_memory() {
        // Arrange: 创建项目并添加记忆
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // 添加记忆
        let mut add_req = make_request("记忆", project_path);
        add_req.content = "测试记忆".to_string();
        let add_result = MemoryTool::jiyi(add_req).await;
        assert!(add_result.is_ok(), "添加记忆失败");

        // 提取记忆 ID（从返回文本中解析）
        let add_content = &add_result.unwrap().content[0];
        let memory_id = {
            let text = extract_text(add_content);
            // 从 "✅ 记忆已新增，ID: <uuid>" 中提取 ID
            text.split("ID: ").nth(1)
                .and_then(|s| s.split('\n').next())
                .expect("无法提取记忆 ID")
                .to_string()
        };

        // Act: 调用活力趋势操作
        let mut req = make_request("活力趋势", project_path);
        req.memory_id = Some(memory_id.clone());
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回结果
        assert!(result.is_ok(), "活力趋势操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        let content = &call_result.content[0];
        let text = extract_text(content);
        let json: serde_json::Value = serde_json::from_str(
            text.split('\n').skip(1).collect::<Vec<_>>().join("\n").as_str()
        ).expect("解析 JSON 失败");
        assert_eq!(json.get("memory_id").and_then(|v| v.as_str()), Some(memory_id.as_str()));
        assert!(json.get("current_vitality").is_some(), "缺少 current_vitality 字段");
        assert!(json.get("trend_points").is_some(), "缺少 trend_points 字段");
    }

    #[tokio::test]
    async fn test_snapshot_list_operation_no_updates() {
        // Arrange: 创建项目并添加记忆（不更新）
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // 添加记忆
        let mut add_req = make_request("记忆", project_path);
        add_req.content = "初始内容".to_string();
        let add_result = MemoryTool::jiyi(add_req).await;
        assert!(add_result.is_ok(), "添加记忆失败");

        // 提取记忆 ID
        let add_content = &add_result.unwrap().content[0];
        let memory_id = {
            let text = extract_text(add_content);
            text.split("ID: ").nth(1)
                .and_then(|s: &str| s.split('\n').next())
                .expect("无法提取记忆 ID")
                .to_string()
        };

        // Act: 调用快照列表操作
        let mut req = make_request("快照列表", project_path);
        req.memory_id = Some(memory_id);
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回结果（应返回空快照列表）
        assert!(result.is_ok(), "快照列表操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        let content = &call_result.content[0];
        let text = extract_text(content);
        let json: serde_json::Value = serde_json::from_str(
            text.split('\n').skip(1).collect::<Vec<_>>().join("\n").as_str()
        ).expect("解析 JSON 失败");
        assert_eq!(json.get("total_snapshots").and_then(|v| v.as_u64()), Some(0));
    }

    #[tokio::test]
    async fn test_rollback_snapshot_operation_no_snapshots() {
        // Arrange: 创建项目并添加记忆（不更新）
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // 添加记忆
        let mut add_req = make_request("记忆", project_path);
        add_req.content = "初始内容".to_string();
        let add_result = MemoryTool::jiyi(add_req).await;
        assert!(add_result.is_ok(), "添加记忆失败");

        // 提取记忆 ID
        let add_content = &add_result.unwrap().content[0];
        let memory_id = {
            let text = extract_text(add_content);
            text.split("ID: ").nth(1)
                .and_then(|s: &str| s.split('\n').next())
                .expect("无法提取记忆 ID")
                .to_string()
        };

        // Act: 调用回滚快照操作（应失败，因为没有快照）
        let mut req = make_request("回滚快照", project_path);
        req.memory_id = Some(memory_id);
        req.target_version = Some(1);
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回错误
        assert!(result.is_err(), "回滚快照应失败（没有快照）");
    }

    #[tokio::test]
    async fn test_rollback_snapshot_operation_success() {
        // Arrange: 创建项目并添加记忆，然后更新（创建快照）
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // 添加记忆
        let mut add_req = make_request("记忆", project_path);
        add_req.content = "版本1".to_string();
        let add_result = MemoryTool::jiyi(add_req).await;
        assert!(add_result.is_ok(), "添加记忆失败");

        // 提取记忆 ID
        let add_content = &add_result.unwrap().content[0];
        let memory_id = {
            let text = extract_text(add_content);
            text.split("ID: ").nth(1)
                .and_then(|s: &str| s.split('\n').next())
                .expect("无法提取记忆 ID")
                .to_string()
        };

        // 更新记忆（创建快照）
        let mut update_req = make_request("更新", project_path);
        update_req.memory_id = Some(memory_id.clone());
        update_req.content = "版本2".to_string();
        let update_result = MemoryTool::jiyi(update_req).await;
        assert!(update_result.is_ok(), "更新记忆失败");

        // Act: 调用回滚快照操作
        let mut req = make_request("回滚快照", project_path);
        req.memory_id = Some(memory_id.clone());
        req.target_version = Some(1);
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回结果
        assert!(result.is_ok(), "回滚快照操作失败");
        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);

        let content = &call_result.content[0];
        let text = extract_text(content);
        assert!(text.contains("已回滚到版本 1"), "应返回回滚成功提示");
        let json: serde_json::Value = serde_json::from_str(
            text.split('\n').skip(1).collect::<Vec<_>>().join("\n").as_str()
        ).expect("解析 JSON 失败");
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(json.get("restored_version").and_then(|v| v.as_u64()), Some(1));
    }

    #[tokio::test]
    async fn test_missing_memory_id_parameter() {
        // Arrange: 创建项目
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // Act: 调用活力趋势操作（缺少 memory_id）
        let req = make_request("活力趋势", project_path);
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回错误
        assert!(result.is_err(), "缺少 memory_id 应返回错误");
    }

    #[tokio::test]
    async fn test_missing_target_version_parameter() {
        // Arrange: 创建项目
        let temp_dir = create_test_project();
        let project_path = temp_dir.path().to_str().unwrap();

        // Act: 调用回滚快照操作（缺少 target_version）
        let mut req = make_request("回滚快照", project_path);
        req.memory_id = Some("test-id".to_string());
        let result = MemoryTool::jiyi(req).await;

        // Assert: 验证返回错误
        assert!(result.is_err(), "缺少 target_version 应返回错误");
    }
}
