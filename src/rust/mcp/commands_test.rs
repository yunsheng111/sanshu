//! 集成测试：search_memories 和 detect_project_root
//!
//! 验证 W1（前缀搜索）、W2（意图识别）和 W9（路径检测）修复

#[cfg(test)]
mod integration_tests {
    use crate::mcp::commands::{search_memories, calculate_intent_boost, SearchMemoryResultDto};
    use crate::mcp::tools::memory::{MemoryCategory, SharedMemoryManager};
    use crate::ui::commands::detect_project_root;
    use tempfile::TempDir;
    use std::fs;

    /// 创建测试用的临时项目目录
    fn create_test_project() -> (TempDir, String) {
        let temp_dir = TempDir::new().expect("创建临时目录失败");
        let project_path = temp_dir.path().to_str().unwrap().to_string();
        fs::create_dir_all(temp_dir.path().join(".git")).expect("创建 .git 目录失败");
        (temp_dir, project_path)
    }

    #[tokio::test]
    async fn test_search_memories_basic() {
        // 基础搜索测试
        let (_temp_dir, project_path) = create_test_project();
        let manager = SharedMemoryManager::new(&project_path).unwrap();

        // 添加测试记忆
        manager.add_memory("前端规范：使用 Vue 3", MemoryCategory::Rule).unwrap();
        manager.add_memory("后端规范：使用 Rust", MemoryCategory::Rule).unwrap();

        // 搜索
        let results: Vec<SearchMemoryResultDto> = search_memories(
            project_path,
            "规范".to_string(),
            None,
            None,
            None,
        ).await.unwrap();

        // 应该返回包含"规范"的记忆
        assert!(results.len() >= 2);
    }

    #[tokio::test]
    async fn test_search_memories_with_category() {
        // 分类过滤测试
        let (_temp_dir, project_path) = create_test_project();
        let manager = SharedMemoryManager::new(&project_path).unwrap();

        manager.add_memory("规范内容", MemoryCategory::Rule).unwrap();
        manager.add_memory("偏好内容", MemoryCategory::Preference).unwrap();

        // 只搜索规范类别
        let results: Vec<SearchMemoryResultDto> = search_memories(
            project_path,
            "内容".to_string(),
            Some("规范".to_string()),
            None,
            None,
        ).await.unwrap();

        // 应该只返回规范类别
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, "规范");
    }

    #[test]
    fn test_detect_project_root_with_git() {
        // W9 修复验证：递归查找 .git
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let sub_dir = project_root.join("src").join("components");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::create_dir_all(project_root.join(".git")).unwrap();

        let found_root = detect_project_root(&sub_dir);
        assert!(found_root.is_some());
        assert_eq!(found_root.unwrap(), project_root);
    }

    #[test]
    fn test_detect_project_root_with_package_json() {
        // W9 修复验证：递归查找 package.json
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let sub_dir = project_root.join("src");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(project_root.join("package.json"), "{}").unwrap();

        let found_root = detect_project_root(&sub_dir);
        assert!(found_root.is_some());
        assert_eq!(found_root.unwrap(), project_root);
    }

    #[test]
    fn test_detect_project_root_with_cargo_toml() {
        // W9 修复验证：递归查找 Cargo.toml
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let sub_dir = project_root.join("src");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(project_root.join("Cargo.toml"), "[package]").unwrap();

        let found_root = detect_project_root(&sub_dir);
        assert!(found_root.is_some());
        assert_eq!(found_root.unwrap(), project_root);
    }

    #[test]
    fn test_detect_project_root_not_found() {
        // W9 修复验证：找不到项目标识时返回 None
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("src");
        fs::create_dir_all(&sub_dir).unwrap();

        let found_root = detect_project_root(&sub_dir);
        assert!(found_root.is_none());
    }

    #[test]
    fn test_calculate_intent_boost_rule() {
        // W2 修复验证：规范类意图识别
        let query = "项目规范要求";
        let content = "必须遵循代码规范";
        let category = MemoryCategory::Rule;

        let boost = calculate_intent_boost(query, content, &category);
        assert!(boost > 0.0);
        assert!(boost <= 0.2);
    }

    #[test]
    fn test_calculate_intent_boost_preference() {
        // W2 修复验证：偏好类意图识别
        let query = "我喜欢使用";
        let content = "个人偏好选择";
        let category = MemoryCategory::Preference;

        let boost = calculate_intent_boost(query, content, &category);
        assert!(boost > 0.0);
        assert!(boost <= 0.2);
    }
}
