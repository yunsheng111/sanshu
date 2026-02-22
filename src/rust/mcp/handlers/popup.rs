use anyhow::Result;
use std::process::Command;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::mcp::types::PopupRequest;
use crate::mcp::utils::safe_truncate_clean;
use crate::{log_important, log_debug};

/// 创建 Tauri 弹窗
///
/// 优先调用与 MCP 服务器同目录的 UI 命令，找不到时使用全局版本
pub fn create_tauri_popup(request: &PopupRequest) -> Result<String> {
    let start = Instant::now();

    // 创建临时请求文件 - 跨平台适配
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("mcp_request_{}.json", request.id));
    let request_json = serde_json::to_string_pretty(request)?;
    fs::write(&temp_file, request_json)?;

    log_important!(
        info,
        "[popup] 已写入MCP请求文件: request_id={}, file={}, message_len={}, message_preview={}, options_len={}, project={:?}, markdown={}",
        request.id,
        temp_file.display(),
        request.message.len(),
        safe_truncate_clean(&request.message, 200),
        request.predefined_options.as_ref().map(|v| v.len()).unwrap_or(0),
        request.project_root_path.as_deref(),
        request.is_markdown
    );

    // 尝试找到等一下命令的路径
    let command_path = find_ui_command()?;

    log_debug!(
        "[popup] 准备调用GUI进程: request_id={}, command_path={}",
        request.id,
        command_path
    );

    // 调用等一下命令
    let output = Command::new(&command_path)
        .arg("--mcp-request")
        .arg(temp_file.to_string_lossy().to_string())
        .output()?;

    // 清理临时文件
    let _ = fs::remove_file(&temp_file);

    let elapsed_ms = start.elapsed().as_millis();
    let exit_code = output.status.code();
    let stdout_len = output.stdout.len();
    let stderr_len = output.stderr.len();

    if output.status.success() {
        let response = String::from_utf8_lossy(&output.stdout);
        let response = response.trim();

        log_important!(
            info,
            "[popup] GUI执行成功: request_id={}, exit_code={:?}, stdout_len={}, stderr_len={}, elapsed_ms={}",
            request.id,
            exit_code,
            stdout_len,
            stderr_len,
            elapsed_ms
        );
        if response.is_empty() {
            Ok("用户取消了操作".to_string())
        } else {
            Ok(response.to_string())
        }
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        log_important!(
            error,
            "[popup] GUI执行失败: request_id={}, exit_code={:?}, stdout_len={}, stderr_len={}, stderr_preview={}, elapsed_ms={}",
            request.id,
            exit_code,
            stdout_len,
            stderr_len,
            safe_truncate_clean(&error, 200),
            elapsed_ms
        );
        anyhow::bail!("UI进程失败: {}", error);
    }
}

/// 查找等一下 UI 命令的路径
///
/// 按优先级查找：同目录 -> 全局版本 -> 开发环境
fn find_ui_command() -> Result<String> {
    // 1. Prefer GUI executable in the same directory as MCP server.
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            #[cfg(windows)]
            let local_candidates = ["sanshu-gui.exe", "sanshu-gui"];

            #[cfg(not(windows))]
            let local_candidates = ["sanshu-gui"];

            for candidate in local_candidates {
                let local_ui_path = exe_dir.join(candidate);
                if local_ui_path.exists() && is_executable(&local_ui_path) {
                    return Ok(local_ui_path.to_string_lossy().to_string());
                }
            }
        }
    }

    // 2. Fallback to global command.
    if test_command_available("sanshu-gui") {
        return Ok("sanshu-gui".to_string());
    }

    // 3. Development environment detection (target/debug or target/release)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            // Check if we're in target/debug or target/release
            let exe_dir_str = exe_dir.to_string_lossy();
            if exe_dir_str.contains("target/debug") || exe_dir_str.contains("target\\debug")
                || exe_dir_str.contains("target/release") || exe_dir_str.contains("target\\release") {

                // Try to find GUI in the same target directory
                #[cfg(windows)]
                let dev_candidates = ["sanshu.exe", "等一下.exe"];

                #[cfg(not(windows))]
                let dev_candidates = ["sanshu", "等一下"];

                for candidate in dev_candidates {
                    let dev_ui_path = exe_dir.join(candidate);
                    if dev_ui_path.exists() && is_executable(&dev_ui_path) {
                        log_important!(
                            info,
                            "[popup] 开发环境检测：使用同目录 GUI 可执行文件: {}",
                            dev_ui_path.display()
                        );
                        return Ok(dev_ui_path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // 4. Return detailed error when command cannot be found.
    anyhow::bail!(
        "UI command not found (tried sanshu-gui). Please ensure:\n\
         1. Build is done: cargo build --release\n\
         2. Or install script has run: ./install.sh\n\
         3. Or the executable is in the same directory as MCP server"
    )
}


fn test_command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// 检查文件是否可执行
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        if !path.is_file() {
            return false;
        }

        // Prefer .exe on Windows, and allow extensionless executable names.
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("exe"))
            .unwrap_or(true)
    }
}

