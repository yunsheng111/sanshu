# UI 模块 (ui)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **ui**

---

## 模块职责

UI 模块，负责窗口管理、音频播放、字体管理、自动更新和退出处理。提供完整的桌面应用 UI 控制能力。

---

## 入口与启动

### 核心结构
```rust
// 窗口管理
pub struct WindowManager;

// 音频管理
pub struct AudioAssetManager;

// 更新管理
pub struct UpdateManager;
```

---

## 对外接口

### Tauri 命令

#### 窗口管理
```rust
#[tauri::command]
async fn apply_window_constraints(state: State<'_, AppState>, app: AppHandle) -> Result<(), String>

#[tauri::command]
async fn update_window_size(size_update: WindowSizeUpdate, state: State<'_, AppState>, app: AppHandle) -> Result<(), String>
```

#### 音频播放
```rust
#[tauri::command]
async fn play_audio(url: String) -> Result<(), String>

#[tauri::command]
async fn stop_audio() -> Result<(), String>
```

#### 字体管理
```rust
#[tauri::command]
async fn apply_font_settings(state: State<'_, AppState>, app: AppHandle) -> Result<(), String>
```

#### 自动更新
```rust
#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<UpdateInfo, String>

#[tauri::command]
async fn download_and_install_update(app: AppHandle) -> Result<(), String>
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
tauri-plugin-updater = "2.0"
rodio = "0.19"
rust-embed = "8.0"
reqwest = { version = "0.11", features = [ "stream" ] }
```

---

## 核心功能

### 1. 窗口管理 (`window.rs`)

#### 窗口约束应用
```rust
#[tauri::command]
pub async fn apply_window_constraints(
    state: State<'_, AppState>,
    app: AppHandle
) -> Result<(), String> {
    let (window_config, always_on_top) = {
        let config = state.config.lock()
            .map_err(|e| format!("获取配置失败: {}", e))?;
        (config.ui_config.window_config.clone(), config.ui_config.always_on_top)
    };

    if let Some(window) = app.get_webview_window("main") {
        // 设置最小尺寸
        window.set_min_size(Some(LogicalSize::new(
            window_config.min_width,
            window_config.min_height,
        )))?;

        // 设置最大尺寸
        window.set_max_size(Some(LogicalSize::new(
            window_config.max_width,
            window_config.max_height,
        )))?;

        // 自动调整大小
        if window_config.auto_resize {
            let initial_width = window_config.min_width;
            let initial_height = (window_config.min_height + window_config.max_height) / 2.0;
            window.set_size(LogicalSize::new(initial_width, initial_height))?;
        }

        // 确保置顶状态
        window.set_always_on_top(always_on_top)?;
    }

    Ok(())
}
```

#### 窗口尺寸更新
```rust
#[tauri::command]
pub async fn update_window_size(
    size_update: WindowSizeUpdate,
    state: State<'_, AppState>,
    app: AppHandle
) -> Result<(), String> {
    // 更新配置
    {
        let mut config = state.config.lock()
            .map_err(|e| format!("获取配置失败: {}", e))?;

        config.ui_config.window_config.fixed = size_update.fixed;
        config.ui_config.window_config.update_current_size(
            size_update.width,
            size_update.height
        );

        if size_update.fixed {
            // 固定模式：最大最小尺寸相同
            config.ui_config.window_config.max_width = size_update.width;
            config.ui_config.window_config.max_height = size_update.height;
            config.ui_config.window_config.min_width = size_update.width;
            config.ui_config.window_config.min_height = size_update.height;
            config.ui_config.window_config.auto_resize = false;
        } else {
            // 自由拉伸模式
            config.ui_config.window_config.min_width = window::MIN_WIDTH;
            config.ui_config.window_config.min_height = window::MIN_HEIGHT;
            config.ui_config.window_config.max_width = window::MAX_WIDTH;
            config.ui_config.window_config.max_height = window::MAX_HEIGHT;
            config.ui_config.window_config.auto_resize = window::DEFAULT_AUTO_RESIZE;
        }
    }

    // 应用窗口约束
    apply_window_constraints(state.clone(), app.clone()).await?;

    // 保存配置
    save_config(state, app).await?;

    Ok(())
}
```

### 2. 音频播放 (`audio.rs`)

#### 音频播放器
```rust
use rodio::{Decoder, OutputStream, Sink};
use std::sync::Mutex;

static AUDIO_SINK: Lazy<Mutex<Option<Sink>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
pub async fn play_audio(url: String) -> Result<(), String> {
    // 停止当前播放
    stop_audio().await?;

    // 创建音频流
    let (_stream, stream_handle) = OutputStream::try_default()
        .map_err(|e| format!("创建音频流失败: {}", e))?;

    let sink = Sink::try_new(&stream_handle)
        .map_err(|e| format!("创建音频 Sink 失败: {}", e))?;

    // 加载音频文件
    if url.starts_with("http://") || url.starts_with("https://") {
        // 网络音频
        let response = reqwest::get(&url).await
            .map_err(|e| format!("下载音频失败: {}", e))?;
        let bytes = response.bytes().await
            .map_err(|e| format!("读取音频数据失败: {}", e))?;
        let cursor = Cursor::new(bytes.to_vec());
        let source = Decoder::new(cursor)
            .map_err(|e| format!("解码音频失败: {}", e))?;
        sink.append(source);
    } else {
        // 本地音频
        let file = std::fs::File::open(&url)
            .map_err(|e| format!("打开音频文件失败: {}", e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("解码音频失败: {}", e))?;
        sink.append(source);
    }

    // 播放
    sink.play();

    // 保存 Sink
    *AUDIO_SINK.lock().unwrap() = Some(sink);

    Ok(())
}

#[tauri::command]
pub async fn stop_audio() -> Result<(), String> {
    if let Some(sink) = AUDIO_SINK.lock().unwrap().take() {
        sink.stop();
    }
    Ok(())
}
```

### 3. 音频资源管理 (`audio_assets.rs`)

#### 嵌入式音频
```rust
#[derive(RustEmbed)]
#[folder = "assets/audio"]
struct AudioAssets;

pub struct AudioAssetManager {
    assets: HashMap<String, Vec<u8>>,
}

impl AudioAssetManager {
    pub fn new() -> Self {
        let mut assets = HashMap::new();

        // 加载所有嵌入式音频
        for file in AudioAssets::iter() {
            if let Some(data) = AudioAssets::get(&file) {
                assets.insert(file.to_string(), data.data.to_vec());
            }
        }

        Self { assets }
    }

    pub fn get_asset(&self, name: &str) -> Option<&Vec<u8>> {
        self.assets.get(name)
    }

    pub fn list_assets(&self) -> Vec<String> {
        self.assets.keys().cloned().collect()
    }
}

pub fn initialize_audio_asset_manager(app_handle: &AppHandle) -> Result<()> {
    let manager = AudioAssetManager::new();
    app_handle.manage(manager);
    Ok(())
}
```

### 4. 自动更新 (`updater.rs`)

#### 检查更新
```rust
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateInfo, String> {
    let updater = app.updater_builder().build()
        .map_err(|e| format!("创建更新器失败: {}", e))?;

    let update = updater.check().await
        .map_err(|e| format!("检查更新失败: {}", e))?;

    if let Some(update) = update {
        Ok(UpdateInfo {
            available: true,
            version: update.version,
            date: update.date,
            body: update.body,
        })
    } else {
        Ok(UpdateInfo {
            available: false,
            version: env!("CARGO_PKG_VERSION").to_string(),
            date: None,
            body: None,
        })
    }
}
```

#### 下载并安装更新
```rust
#[tauri::command]
pub async fn download_and_install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater_builder().build()
        .map_err(|e| format!("创建更新器失败: {}", e))?;

    let update = updater.check().await
        .map_err(|e| format!("检查更新失败: {}", e))?;

    if let Some(update) = update {
        // 下载更新
        update.download_and_install().await
            .map_err(|e| format!("下载更新失败: {}", e))?;

        // 重启应用
        app.restart();
    }

    Ok(())
}
```

### 5. 窗口事件监听 (`window_events.rs`)

#### 事件监听器
```rust
pub fn setup_window_event_listeners(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        // 窗口关闭事件
        window.on_window_event(|event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // 阻止默认关闭行为
                    api.prevent_close();

                    // 触发退出处理
                    handle_exit_request();
                }
                WindowEvent::Focused(focused) => {
                    if *focused {
                        log_debug!("窗口获得焦点");
                    } else {
                        log_debug!("窗口失去焦点");
                    }
                }
                _ => {}
            }
        });
    }
}
```

### 6. 退出处理 (`exit.rs` & `exit_handler.rs`)

#### 退出流程
```rust
#[tauri::command]
pub async fn exit_app() -> Result<(), String> {
    // 1. 停止音频
    stop_audio().await?;

    // 2. 保存配置
    // (由调用方负责)

    // 3. 清理资源
    cleanup_resources().await?;

    // 4. 退出应用
    std::process::exit(0);
}

async fn cleanup_resources() -> Result<(), String> {
    // 清理临时文件
    // 关闭数据库连接
    // 停止后台任务
    Ok(())
}
```

#### 退出处理器
```rust
pub fn setup_exit_handlers(app_handle: &AppHandle) -> Result<()> {
    // 注册 Ctrl+C 处理器
    let app_handle_clone = app_handle.clone();
    ctrlc::set_handler(move || {
        log_important!(info, "收到 Ctrl+C 信号，准备退出");
        app_handle_clone.exit(0);
    })?;

    Ok(())
}
```

### 7. 字体管理 (`font_commands.rs`)

#### 字体应用
```rust
#[tauri::command]
pub async fn apply_font_settings(
    state: State<'_, AppState>,
    app: AppHandle
) -> Result<(), String> {
    let font_config = {
        let config = state.config.lock()
            .map_err(|e| format!("获取配置失败: {}", e))?;
        config.ui_config.font_config.clone()
    };

    // 发送事件到前端
    app.emit("font_settings_changed", font_config)?;

    Ok(())
}
```

---

## 数据流程

### 窗口管理流程
```
配置更新 → 锁定配置 → 更新窗口约束 → 应用到窗口 → 保存配置
```

### 音频播放流程
```
播放请求 → 停止当前播放 → 创建音频流 → 加载音频 → 解码 → 播放
```

### 更新流程
```
检查更新 → 获取更新信息 → 用户确认 → 下载更新 → 安装 → 重启应用
```

---

## 常见问题 (FAQ)

### Q: 如何自定义窗口尺寸？
A: 在设置页面调整窗口尺寸，或编辑配置文件

### Q: 支持哪些音频格式？
A: MP3, WAV, OGG, FLAC（由 rodio 支持）

### Q: 如何禁用自动更新？
A: 在 `tauri.conf.json` 中设置 `plugins.updater.active: false`

### Q: 窗口置顶失效怎么办？
A: 检查操作系统权限设置

### Q: 如何添加自定义音效？
A: 将音频文件放入 `assets/audio/` 目录并重新编译

---

## 相关文件清单

### 核心文件
- `window.rs` - 窗口管理
- `audio.rs` - 音频播放
- `audio_assets.rs` - 音频资源管理
- `updater.rs` - 自动更新
- `exit.rs` - 退出处理
- `exit_handler.rs` - 退出处理器
- `window_events.rs` - 窗口事件监听
- `font_commands.rs` - 字体管理
- `commands.rs` - Tauri 命令
- `mod.rs` - 模块导出

### 资源文件
- `assets/audio/` - 嵌入式音频文件

---

## 使用示例

### 应用窗口约束
```rust
apply_window_constraints(state, app).await?;
```

### 播放音频
```rust
play_audio("https://example.com/notification.mp3".to_string()).await?;
```

### 检查更新
```rust
let update_info = check_for_updates(app).await?;
if update_info.available {
    println!("发现新版本: {}", update_info.version);
}
```

### 更新窗口尺寸
```rust
let size_update = WindowSizeUpdate {
    width: 800.0,
    height: 600.0,
    fixed: false,
};
update_window_size(size_update, state, app).await?;
```

---

**最后更新**: 2026-02-18
