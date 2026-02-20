# 应用模块 (app)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **app**

---

## 模块职责

Tauri 应用构建与 CLI 处理，负责应用初始化、命令行参数解析、GUI 启动和 MCP 请求模式切换。

---

## 入口与启动

### 主入口
- **文件**: `main.rs`
- **二进制名**: `等一下`
- **职责**: 处理命令行参数，启动 GUI 或 MCP 请求模式

```rust
fn main() -> Result<()> {
    // 初始化日志系统
    auto_init_logger()?;

    // 处理命令行参数
    handle_cli_args()
}
```

### 启动模式
| 模式 | 触发条件 | 说明 |
|------|----------|------|
| GUI 模式 | 无参数 | 启动 Tauri 窗口应用 |
| MCP 请求模式 | `--mcp-request <file>` | 读取请求文件，弹窗交互 |
| CLI 交互模式 | `--cli` | 解析参数，启动 GUI 交互 |
| 图标搜索模式 | `--icon-search` | 启动图标工坊弹窗 |
| 帮助/版本 | `--help` / `--version` | 显示帮助或版本信息 |

---

## 对外接口

### Tauri 命令 (`commands.rs`)

#### 配置管理
```rust
#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String>

#[tauri::command]
async fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn get_config_file_path() -> Result<String, String>
```

#### MCP 响应
```rust
#[tauri::command]
async fn send_mcp_response(response: String) -> Result<(), String>
```

#### 应用控制
```rust
#[tauri::command]
async fn exit_app() -> Result<(), String>
```

---

## 关键依赖与配置

### 核心依赖
```toml
tauri = { version = "2.0", features = [
  "tray-icon",
  "image-ico",
  "image-png"
] }
tauri-plugin-shell = "2.0"
tauri-plugin-updater = "2.0"
anyhow = "1.0"
```

### 应用配置
- **文件**: `tauri.conf.json`
- **窗口配置**:
  - 默认尺寸: 600x800
  - 最小尺寸: 600x400
  - 最大尺寸: 4096x4096
  - 置顶: true
  - 无边框标题栏: true

---

## 核心功能

### 1. CLI 参数处理 (`cli.rs`)

#### 参数解析
```rust
pub fn handle_cli_args() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.len() {
        1 => run_tauri_app(),  // 无参数：GUI 模式
        2 => match args[1].as_str() {
            "--help" | "-h" => print_help(),
            "--version" | "-v" => print_version(),
            _ => {
                eprintln!("未知参数: {}", args[1]);
                std::process::exit(1);
            }
        },
        _ => handle_multi_args(&args),
    }
}
```

#### MCP 请求模式
```rust
fn handle_mcp_request(request_file: &str) -> Result<()> {
    // 1. 读取请求文件
    let request_json = std::fs::read_to_string(request_file)?;
    let popup_request: PopupRequest = serde_json::from_str(&request_json)?;

    // 2. 设置环境变量
    std::env::set_var("MCP_REQUEST", serde_json::to_string(&popup_request)?);

    // 3. 启动 GUI
    run_tauri_app();

    Ok(())
}
```

#### CLI 交互模式
```rust
fn handle_cli_mode(args: &[String]) -> Result<()> {
    // 解析参数
    let mut message: Option<String> = None;
    let mut options: Vec<String> = Vec::new();
    let mut project_root: Option<String> = None;

    // 参数解析逻辑
    // --message / -m: 消息内容
    // --option / -o: 预定义选项
    // --project / -p: 项目路径
    // --markdown / --no-markdown: Markdown 开关

    // 构建请求并启动 GUI
    let popup_request = PopupRequest { /* ... */ };
    std::env::set_var("MCP_REQUEST", serde_json::to_string(&popup_request)?);
    run_tauri_app();

    Ok(())
}
```

#### 图标搜索模式
```rust
fn handle_icon_search(args: &[String]) -> Result<()> {
    // 解析参数
    // --query / -q: 搜索关键词
    // --style / -s: 图标风格
    // --save-path: 保存路径
    // --project: 项目根路径

    // 构建请求并启动 GUI
    std::env::set_var("ICON_SEARCH_MODE", "true");
    std::env::set_var("ICON_SEARCH_PARAMS", serde_json::to_string(&params)?);
    run_tauri_app();

    Ok(())
}
```

### 2. 应用构建 (`builder.rs`)

#### Tauri 应用构建
```rust
pub fn run_tauri_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // 配置管理
            get_config,
            save_config,
            get_config_file_path,

            // MCP 响应
            send_mcp_response,

            // 应用控制
            exit_app,

            // 窗口管理
            apply_window_constraints,
            update_window_size,

            // 音频播放
            play_audio,
            stop_audio,

            // ... 更多命令
        ])
        .setup(|app| {
            // 应用初始化
            tauri::async_runtime::block_on(async {
                setup_application(app.handle()).await
            })?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. 应用设置 (`setup.rs`)

#### 初始化流程
```rust
pub async fn setup_application(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();

    // 1. 加载配置并应用窗口设置
    load_config_and_apply_window_settings(&state, app_handle).await?;

    // 2. 初始化音频资源管理器
    initialize_audio_asset_manager(app_handle)?;

    // 3. 设置窗口事件监听器
    setup_window_event_listeners(app_handle);

    // 4. 设置退出处理器
    setup_exit_handlers(app_handle)?;

    Ok(())
}
```

---

## 数据流程

### GUI 启动流程
```
main() → handle_cli_args() → run_tauri_app() → setup_application() → 显示窗口
```

### MCP 请求流程
```
main() → handle_mcp_request() → 读取请求文件 → 设置环境变量 → 启动 GUI → 弹窗交互 → 返回响应
```

### CLI 交互流程
```
main() → handle_cli_mode() → 解析参数 → 构建请求 → 启动 GUI → 弹窗交互 → 返回响应
```

---

## 常见问题 (FAQ)

### Q: 如何添加新的 Tauri 命令？
A:
1. 在 `commands.rs` 添加命令函数（标注 `#[tauri::command]`）
2. 在 `builder.rs` 的 `invoke_handler` 中注册
3. 在前端使用 `invoke('command_name', { args })` 调用

### Q: 如何修改窗口默认配置？
A: 编辑 `tauri.conf.json` 中的 `app.windows` 配置

### Q: 如何调试 CLI 参数？
A: 设置环境变量 `RUST_LOG=debug` 并运行 `cargo run -- <args>`

### Q: 如何添加新的启动模式？
A: 在 `cli.rs` 的 `handle_multi_args()` 中添加新的参数处理逻辑

---

## 相关文件清单

### 核心文件
- `main.rs` - 主入口
- `builder.rs` - Tauri 应用构建
- `cli.rs` - CLI 参数处理
- `setup.rs` - 应用初始化
- `commands.rs` - Tauri 命令
- `mod.rs` - 模块导出

### 配置文件
- `tauri.conf.json` - Tauri 配置
- `Cargo.toml` - Rust 依赖配置

---

## 使用示例

### GUI 模式
```bash
# 直接启动
./等一下

# 或
cargo run
```

### MCP 请求模式
```bash
# 从文件读取请求
./等一下 --mcp-request request.json

# request.json 格式
{
  "id": "req-123",
  "message": "请选择操作",
  "predefined_options": ["选项1", "选项2"],
  "is_markdown": true,
  "project_root_path": "/path/to/project"
}
```

### CLI 交互模式
```bash
# 基础交互
./等一下 --cli --message "是否继续？" --option "是" --option "否"

# 带项目路径
./等一下 --cli -m "选择操作" -o "提交" -o "取消" -p /path/to/project

# Markdown 消息
./等一下 --cli -m "## 审查结果\n- ✅ 通过" --markdown
```

### 图标搜索模式
```bash
# 搜索图标
./等一下 --icon-search --query "用户" --style "线性" --save-path ./icons --project /path/to/project
```

---

**最后更新**: 2026-02-18
