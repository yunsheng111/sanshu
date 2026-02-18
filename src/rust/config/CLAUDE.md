# 配置管理模块 (config)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **config**

---

## 模块职责

配置管理模块，负责应用配置的加载、保存、验证和持久化。支持多平台配置路径、默认值和热重载。

---

## 入口与启动

### 核心结构
```rust
pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
}

pub struct AppConfig {
    pub ui_config: UiConfig,
    pub audio_config: AudioConfig,
    pub reply_config: ReplyConfig,
    pub mcp_config: McpConfig,
    pub telegram_config: TelegramConfig,
    pub custom_prompt_config: CustomPromptConfig,
    pub shortcut_config: ShortcutConfig,
    pub proxy_config: ProxyConfig,
}
```

---

## 对外接口

### Tauri 命令
```rust
#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String>

#[tauri::command]
async fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn get_config_file_path() -> Result<String, String>

#[tauri::command]
async fn reload_config(state: State<'_, AppState>) -> Result<AppConfig, String>
```

---

## 关键依赖与配置

### 核心依赖
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dirs = "5.0"
anyhow = "1.0"
```

### 配置文件位置
- **Windows**: `%APPDATA%\sanshu\config.json`
- **macOS**: `~/Library/Application Support/sanshu/config.json`
- **Linux**: `~/.config/sanshu/config.json`

---

## 核心功能

### 1. 配置结构 (`settings.rs`)

#### 主配置
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_ui_config")]
    pub ui_config: UiConfig,

    #[serde(default = "default_audio_config")]
    pub audio_config: AudioConfig,

    #[serde(default = "default_reply_config")]
    pub reply_config: ReplyConfig,

    #[serde(default = "default_mcp_config")]
    pub mcp_config: McpConfig,

    #[serde(default = "default_telegram_config")]
    pub telegram_config: TelegramConfig,

    #[serde(default = "default_custom_prompt_config")]
    pub custom_prompt_config: CustomPromptConfig,

    #[serde(default = "default_shortcut_config")]
    pub shortcut_config: ShortcutConfig,

    #[serde(default = "default_proxy_config")]
    pub proxy_config: ProxyConfig,
}
```

#### UI 配置
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    /// 主题（"light" | "dark"）
    #[serde(default = "default_theme")]
    pub theme: String,

    /// 字体配置
    #[serde(default = "default_font_config")]
    pub font_config: FontConfig,

    /// 窗口配置
    #[serde(default = "default_window_config")]
    pub window_config: WindowConfig,

    /// 置顶设置
    #[serde(default = "default_always_on_top")]
    pub always_on_top: bool,
}
```

#### MCP 配置
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpConfig {
    /// 工具启用状态
    pub tools: HashMap<String, bool>,

    /// 继续回复配置
    #[serde(default = "default_continue_reply_enabled")]
    pub continue_reply_enabled: bool,

    #[serde(default = "default_auto_continue_threshold")]
    pub auto_continue_threshold: u32,

    #[serde(default = "default_continue_prompt")]
    pub continue_prompt: String,

    /// Acemcp 配置
    pub acemcp_config: Option<AcemcpConfig>,

    /// Context7 配置
    pub context7_api_key: Option<String>,

    /// Enhance 配置
    pub enhance_config: Option<EnhanceConfig>,

    /// 技能 Python 路径
    pub skill_python_path: Option<String>,
}
```

### 2. 配置加载 (`storage.rs`)

#### 加载流程
```rust
pub fn load_config() -> Result<AppConfig> {
    let config_path = get_config_path()?;

    // 如果配置文件不存在，创建默认配置
    if !config_path.exists() {
        let default_config = AppConfig::default();
        save_config(&default_config)?;
        return Ok(default_config);
    }

    // 读取配置文件
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow!("读取配置文件失败: {}", e))?;

    // 解析 JSON
    let config: AppConfig = serde_json::from_str(&content)
        .map_err(|e| anyhow!("解析配置文件失败: {}", e))?;

    Ok(config)
}
```

#### 配置路径获取
```rust
pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("无法获取配置目录"))?;

    let app_config_dir = config_dir.join("sanshu");

    // 创建配置目录
    std::fs::create_dir_all(&app_config_dir)
        .map_err(|e| anyhow!("创建配置目录失败: {}", e))?;

    Ok(app_config_dir.join("config.json"))
}
```

### 3. 配置保存

#### 保存流程
```rust
pub fn save_config(config: &AppConfig) -> Result<()> {
    let config_path = get_config_path()?;

    // 序列化为 JSON（格式化输出）
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| anyhow!("序列化配置失败: {}", e))?;

    // 写入文件
    std::fs::write(&config_path, content)
        .map_err(|e| anyhow!("写入配置文件失败: {}", e))?;

    Ok(())
}
```

### 4. 默认值

#### 默认配置函数
```rust
fn default_ui_config() -> UiConfig {
    UiConfig {
        theme: default_theme(),
        font_config: default_font_config(),
        window_config: default_window_config(),
        always_on_top: default_always_on_top(),
    }
}

fn default_theme() -> String {
    "light".to_string()
}

fn default_always_on_top() -> bool {
    true
}

fn default_mcp_config() -> McpConfig {
    McpConfig {
        tools: default_mcp_tools(),
        continue_reply_enabled: true,
        auto_continue_threshold: 1000,
        continue_prompt: "请按照最佳实践继续".to_string(),
        acemcp_config: None,
        context7_api_key: None,
        enhance_config: None,
        skill_python_path: None,
    }
}

pub fn default_mcp_tools() -> HashMap<String, bool> {
    let mut tools = HashMap::new();
    tools.insert("zhi".to_string(), true);
    tools.insert("ji".to_string(), true);
    tools.insert("sou".to_string(), false);
    tools.insert("context7".to_string(), true);
    tools.insert("uiux".to_string(), true);
    tools.insert("enhance".to_string(), false);
    tools
}
```

### 5. 配置验证

#### 验证流程
```rust
impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        // 验证主题
        if !["light", "dark"].contains(&self.ui_config.theme.as_str()) {
            return Err(anyhow!("无效的主题: {}", self.ui_config.theme));
        }

        // 验证窗口尺寸
        if self.ui_config.window_config.min_width < 400.0 {
            return Err(anyhow!("窗口最小宽度不能小于 400"));
        }

        // 验证 Telegram 配置
        if self.telegram_config.enabled {
            if self.telegram_config.bot_token.is_empty() {
                return Err(anyhow!("Telegram Bot Token 不能为空"));
            }
            if self.telegram_config.chat_id.is_empty() {
                return Err(anyhow!("Telegram Chat ID 不能为空"));
            }
        }

        Ok(())
    }
}
```

### 6. 配置热重载

#### 重载流程
```rust
pub async fn reload_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    // 从文件加载配置
    let new_config = load_config()
        .map_err(|e| format!("加载配置失败: {}", e))?;

    // 验证配置
    new_config.validate()
        .map_err(|e| format!("配置验证失败: {}", e))?;

    // 更新状态
    {
        let mut config = state.config.lock()
            .map_err(|e| format!("锁定配置失败: {}", e))?;
        *config = new_config.clone();
    }

    Ok(new_config)
}
```

### 7. 独立配置加载

#### 用于 MCP 服务器
```rust
pub fn load_standalone_config() -> Result<AppConfig> {
    // MCP 服务器独立运行时使用
    load_config()
}

pub fn load_standalone_telegram_config() -> Result<TelegramConfig> {
    let config = load_standalone_config()?;
    Ok(config.telegram_config)
}
```

---

## 配置迁移

### 版本升级
```rust
pub fn migrate_config(old_version: &str, config: &mut AppConfig) -> Result<()> {
    match old_version {
        "0.4.0" => {
            // 添加新字段的默认值
            if config.mcp_config.skill_python_path.is_none() {
                config.mcp_config.skill_python_path = Some("python".to_string());
            }
        }
        _ => {}
    }
    Ok(())
}
```

---

## 数据流程

### 加载流程
```
应用启动 → 获取配置路径 → 检查文件存在 → 读取文件 → 解析 JSON → 验证配置 → 返回配置
```

### 保存流程
```
配置更新 → 验证配置 → 序列化 JSON → 写入文件 → 发送重载事件
```

### 热重载流程
```
文件变更 → 监听器触发 → 重新加载 → 验证 → 更新状态 → 通知前端
```

---

## 常见问题 (FAQ)

### Q: 配置文件在哪里？
A: 调用 `get_config_file_path` 命令查看

### Q: 如何重置配置？
A: 删除配置文件，重启应用会自动创建默认配置

### Q: 配置文件损坏怎么办？
A: 删除配置文件，或手动修复 JSON 格式

### Q: 如何备份配置？
A: 复制配置文件到安全位置

### Q: 支持配置导入导出吗？
A: 可以手动复制配置文件实现

---

## 相关文件清单

### 核心文件
- `settings.rs` - 配置结构定义
- `storage.rs` - 配置加载与保存
- `mod.rs` - 模块导出

### 配置文件
- `config.json` - 应用配置

---

## 使用示例

### 加载配置
```rust
let config = load_config()?;
println!("当前主题: {}", config.ui_config.theme);
```

### 保存配置
```rust
let mut config = load_config()?;
config.ui_config.theme = "dark".to_string();
save_config(&config)?;
```

### 获取配置路径
```rust
let path = get_config_path()?;
println!("配置文件路径: {}", path.display());
```

### 验证配置
```rust
let config = load_config()?;
config.validate()?;
```

### 热重载配置
```rust
let new_config = reload_config(state).await?;
println!("配置已重载");
```

---

**最后更新**: 2026-02-18
