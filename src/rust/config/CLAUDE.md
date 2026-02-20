# 配置管理模块 (config)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **config**

---

## 模块职责

配置管理模块，负责应用配置的加载、保存、验证和持久化。支持多平台配置路径、默认值、热重载和 API 密钥安全存储（系统凭据管理器）。

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

## 核心功能

### 1. 配置结构 (`settings.rs`)

主配置 `AppConfig` 包含 8 个子配置（UI、音频、回复、MCP、Telegram、自定义提示词、快捷键、代理），全部支持 serde 默认值。

### 2. 配置加载与保存 (`storage.rs`)

```
加载流程: 获取路径 → 检查文件 → 读取 JSON → 合并默认快捷键 → 合并默认提示词 → 返回
保存流程: 序列化 JSON → 写入文件 → sync_all() 强制刷新
```

**损坏恢复**：保存时使用 `sync_all()` 确保文件系统缓存刷新，降低写入中断导致损坏的风险。

### 3. 密钥安全存储 (`keyring.rs`) - P2 新增

HC-9 实现。使用系统凭据管理器安全存储敏感 API 密钥，避免明文存储在配置文件中。

```rust
pub enum ApiKeyType {
    AcemcpToken,       // Acemcp API Token
    Context7ApiKey,    // Context7 API Key
    EnhanceApiKey,     // Enhance API Key
    SouEmbeddingApiKey,// Sou Embedding API Key
}

pub struct SecureKeyStore;

impl SecureKeyStore {
    pub fn store(key_type: ApiKeyType, value: &str) -> Result<()>
    pub fn get(key_type: ApiKeyType) -> Result<String>
    pub fn delete(key_type: ApiKeyType) -> Result<()>
    pub fn exists(key_type: ApiKeyType) -> bool
}
```

**支持的后端**：
- **Windows**: Windows Credential Manager
- **macOS**: macOS Keychain
- **Linux**: Secret Service (GNOME Keyring / KDE Wallet)

**服务标识**: `sanshu`（在凭据管理器中显示的服务名）

### 4. 配置文件位置

- **Windows**: `%APPDATA%\sanshu\config.json`
- **macOS**: `~/Library/Application Support/sanshu/config.json`
- **Linux**: `~/.config/sanshu/config.json`

### 5. 独立配置加载

```rust
/// MCP 服务器独立运行时使用（不依赖 Tauri AppHandle）
pub fn load_standalone_config() -> Result<AppConfig>
pub fn load_standalone_telegram_config() -> Result<TelegramConfig>
```

---

## 数据流程

### 加载流程
```
应用启动 → 获取配置路径 → 检查文件 → 读取 JSON → 合并默认值 → 验证 → 返回
```

### 保存流程
```
配置更新 → 序列化 JSON → 写入文件 → sync_all() 刷新 → 发送重载事件
```

### 密钥流程（P2 新增）
```
存储: 用户输入 API Key → SecureKeyStore::store() → 系统凭据管理器
读取: 工具初始化 → SecureKeyStore::get() → 返回密钥（或 Error）
```

---

## 常见问题 (FAQ)

### Q: 配置文件在哪里？
A: 调用 `get_config_file_path` 命令查看

### Q: 如何重置配置？
A: 删除配置文件，重启应用会自动创建默认配置

### Q: API 密钥存储在哪里？
A: 通过 `SecureKeyStore` 存储在系统凭据管理器中，不会写入 config.json

### Q: 配置文件损坏怎么办？
A: 删除配置文件，重启后自动创建默认配置。保存机制使用 `sync_all()` 降低损坏概率

---

## 相关文件清单

### 核心文件
- `settings.rs` - 配置结构定义（AppConfig + 8 个子配置）
- `storage.rs` - 配置加载与保存（含 sync_all 强制刷新）
- `keyring.rs` - P2 HC-9 密钥安全存储（系统凭据管理器）
- `mod.rs` - 模块导出

### 配置文件
- `config.json` - 应用配置（不含敏感密钥）
- 系统凭据管理器 - API 密钥存储

---

**最后更新**: 2026-02-19
