# 网络模块 (network)

[根目录](../../../CLAUDE.md) > [rust](../CLAUDE.md) > **network**

---

## 模块职责

网络模块，提供代理检测、地理位置检测和 HTTP 客户端构建功能。支持自动检测本地代理、SOCKS5/HTTP 代理和智能降级。

---

## 入口与启动

### 核心结构
```rust
pub struct ProxyDetector;
pub struct ProxyInfo {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
}

pub enum ProxyType {
    Http,
    Socks5,
}
```

---

## 对外接口

### Tauri 命令
```rust
#[tauri::command]
async fn detect_proxy() -> Result<Option<ProxyInfo>, String>

#[tauri::command]
async fn detect_geo_location() -> Result<GeoLocation, String>

#[tauri::command]
async fn test_proxy_connection(proxy: ProxyInfo) -> Result<bool, String>
```

---

## 关键依赖与配置

### 核心依赖
```toml
reqwest = { version = "0.11", features = ["socks", "json"] }
tokio = { version = "1.0", features = ["net"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## 核心功能

### 1. 代理检测 (`proxy.rs`)

#### 自动检测
```rust
impl ProxyDetector {
    /// 常用代理端口列表（按优先级排序）
    const COMMON_PORTS: &'static [(u16, ProxyType)] = &[
        (7890, ProxyType::Http),    // Clash 混合端口
        (7891, ProxyType::Http),    // Clash HTTP 端口
        (10808, ProxyType::Http),   // V2Ray HTTP 端口
        (10809, ProxyType::Socks5), // V2Ray SOCKS5 端口
        (1080, ProxyType::Socks5),  // 通用 SOCKS5 端口
        (8080, ProxyType::Http),    // 通用 HTTP 端口
    ];

    /// 检测本地可用的代理
    pub async fn detect_available_proxy() -> Option<ProxyInfo> {
        for (port, proxy_type) in Self::COMMON_PORTS {
            if Self::test_port("127.0.0.1", *port).await {
                return Some(ProxyInfo::new(
                    proxy_type.clone(),
                    "127.0.0.1".to_string(),
                    *port
                ));
            }
        }
        None
    }

    /// 测试端口是否可用
    async fn test_port(host: &str, port: u16) -> bool {
        tokio::net::TcpStream::connect((host, port))
            .await
            .is_ok()
    }
}
```

#### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_proxy() {
        let proxy = ProxyDetector::detect_available_proxy().await;
        // 如果本地有代理，应该能检测到
        if let Some(info) = proxy {
            assert!(info.port > 0);
            assert!(!info.host.is_empty());
        }
    }

    #[tokio::test]
    async fn test_port_detection() {
        // 测试常见端口
        let result = ProxyDetector::test_port("127.0.0.1", 7890).await;
        // 结果取决于本地是否运行代理
        println!("Port 7890 available: {}", result);
    }
}
```

### 2. 地理位置检测 (`geo.rs`)

#### 检测流程
```rust
pub async fn detect_geo_location() -> Result<GeoLocation> {
    // 1. 创建 HTTP 客户端
    let client = create_http_client()?;

    // 2. 调用 IP 地理位置 API
    let response = client
        .get("https://ipapi.co/json/")
        .send()
        .await?;

    // 3. 解析响应
    let geo: GeoLocation = response.json().await?;

    Ok(geo)
}
```

#### 数据结构
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct GeoLocation {
    pub ip: String,
    pub city: String,
    pub region: String,
    pub country: String,
    pub country_code: String,
    pub timezone: String,
    pub latitude: f64,
    pub longitude: f64,
}
```

#### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_geo_location() {
        let result = detect_geo_location().await;
        assert!(result.is_ok());

        let geo = result.unwrap();
        assert!(!geo.ip.is_empty());
        assert!(!geo.country.is_empty());
    }
}
```

### 3. HTTP 客户端构建 (`client.rs`)

#### 通用客户端
```rust
pub fn create_http_client() -> Result<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

    // 自动检测代理
    if let Some(proxy) = detect_proxy_sync() {
        let proxy_url = proxy.to_url();
        builder = builder.proxy(Proxy::all(&proxy_url)?);
    }

    builder.build().map_err(|e| anyhow!("创建 HTTP 客户端失败: {}", e))
}
```

#### 更新客户端（带代理）
```rust
pub fn create_update_client() -> Result<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("sanshu-updater");

    // 强制使用代理（用于 GitHub API）
    if let Some(proxy) = detect_proxy_sync() {
        let proxy_url = proxy.to_url();
        builder = builder.proxy(Proxy::all(&proxy_url)?);
    }

    builder.build().map_err(|e| anyhow!("创建更新客户端失败: {}", e))
}
```

#### 下载客户端（无代理）
```rust
pub fn create_download_client() -> Result<Client> {
    // 下载文件时不使用代理（避免速度慢）
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("sanshu-downloader")
        .no_proxy()
        .build()
        .map_err(|e| anyhow!("创建下载客户端失败: {}", e))
}
```

#### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_http_client() {
        let result = create_http_client();
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_update_client() {
        let result = create_update_client();
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_download_client() {
        let result = create_download_client();
        assert!(result.is_ok());
    }
}
```

---

## 代理配置

### 配置结构
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    /// 是否启用代理
    pub enabled: bool,

    /// 代理类型
    pub proxy_type: ProxyType,

    /// 代理主机
    pub host: String,

    /// 代理端口
    pub port: u16,

    /// 仅对特定请求启用
    pub only_for: Option<Vec<String>>,
}
```

### 使用场景
| 场景 | 代理策略 | 说明 |
|------|----------|------|
| GitHub API | 自动检测 | 用于检查更新 |
| 文件下载 | 禁用代理 | 避免速度慢 |
| Iconfont API | 禁用代理 | 国内网站直连 |
| Context7 API | 自动检测 | 国外服务 |
| Augment API | 自动检测 | 国外服务 |

---

## 数据流程

### 代理检测流程
```
启动应用 → 遍历常用端口 → 测试连接 → 返回第一个可用代理
```

### 地理位置检测流程
```
调用 API → 创建客户端（自动代理） → 发送请求 → 解析响应 → 返回地理信息
```

### 客户端构建流程
```
创建 Builder → 检测代理 → 配置代理（可选） → 设置超时 → 构建客户端
```

---

## 常见问题 (FAQ)

### Q: 如何禁用自动代理检测？
A: 在配置文件中设置 `proxy_config.enabled: false`

### Q: 支持哪些代理类型？
A: HTTP 和 SOCKS5

### Q: 如何手动配置代理？
A: 在配置文件中设置 `proxy_config` 字段

### Q: 代理检测失败怎么办？
A: 检查代理软件是否运行，端口是否正确

### Q: 如何测试代理连接？
A: 调用 Tauri 命令 `test_proxy_connection`

---

## 相关文件清单

### 核心文件
- `proxy.rs` - 代理检测
- `geo.rs` - 地理位置检测
- `client.rs` - HTTP 客户端构建
- `commands.rs` - Tauri 命令
- `mod.rs` - 模块导出

---

## 使用示例

### 检测代理
```rust
let proxy = ProxyDetector::detect_available_proxy().await;
if let Some(info) = proxy {
    println!("检测到代理: {}:{} ({})", info.host, info.port, info.proxy_type);
}
```

### 检测地理位置
```rust
let geo = detect_geo_location().await?;
println!("当前位置: {}, {} ({})", geo.city, geo.country, geo.ip);
```

### 创建带代理的客户端
```rust
let client = create_http_client()?;
let response = client.get("https://api.github.com").send().await?;
```

### 创建无代理的客户端
```rust
let client = create_download_client()?;
let response = client.get("https://example.com/file.zip").send().await?;
```

---

**最后更新**: 2026-02-18
