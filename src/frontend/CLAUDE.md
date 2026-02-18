# 前端模块 (src/frontend)

[根目录](../../CLAUDE.md) > **frontend**

---

## 模块职责

Vue 3 前端应用，提供三术工具的图形化界面，包括 MCP 弹窗交互、设置管理、主题切换、图标工坊等功能。

---

## 入口与启动

### 主入口
- **文件**: `main.ts`
- **职责**: 初始化 Vue 应用，注册 Naive UI 组件，挂载到 DOM

```typescript
// 核心依赖
import { createApp } from 'vue'
import { create } from 'naive-ui'
import App from './App.vue'

// 初始化 Naive UI
const naive = create({ components: [...] })
const app = createApp(App)
app.use(naive)
app.mount('#app')
```

### 根组件
- **文件**: `App.vue`
- **职责**: 应用容器，管理主题、MCP 请求、事件处理

---

## 对外接口

### Tauri 命令调用
```typescript
import { invoke } from '@tauri-apps/api/core'

// MCP 响应
await invoke('send_mcp_response', { response })

// 配置管理
await invoke('get_config')
await invoke('save_config', { config })

// 窗口控制
await invoke('apply_window_constraints')
await invoke('update_window_size', { size_update })

// 音频播放
await invoke('play_audio', { url })
await invoke('stop_audio')

// Telegram 测试
await invoke('test_telegram_connection', { config })
```

### 事件监听
```typescript
import { listen } from '@tauri-apps/api/event'

// MCP 请求事件
await listen('mcp_request', (event) => {
  // 处理 MCP 请求
})

// 配置重载事件
await listen('config_reloaded', () => {
  // 重新加载配置
})
```

---

## 关键依赖与配置

### 核心依赖
```json
{
  "vue": "^3.5.16",
  "naive-ui": "^2.41.1",
  "@tauri-apps/api": "^2.5.0",
  "@vueuse/core": "^13.3.0",
  "markdown-it": "^14.1.0",
  "highlight.js": "^11.11.1"
}
```

### 构建配置
- **文件**: `vite.config.js`
- **端口**: 5176 (dev), 5177 (HMR)
- **构建目标**: Chrome 105+ (Windows), Safari 13+ (macOS)
- **代码分割**: vendor (vue, @vueuse), markdown (markdown-it, highlight.js)

### 样式配置
- **文件**: `uno.config.ts`
- **预设**: Wind3, Attributify, Icons, Typography, WebFonts
- **图标集**: Carbon, FontAwesome 6
- **主题**: 语义化颜色系统 (surface, primary, success, warning, error)

---

## 组件结构

### 布局组件 (`components/layout/`)
- `LayoutWrapper.vue` - 布局容器
- `MainLayout.vue` - 主布局（标签页导航）

### 弹窗组件 (`components/popup/`)
- `McpPopup.vue` - MCP 交互弹窗（核心组件）
- `PopupHeader.vue` - 弹窗头部
- `PopupContent.vue` - 弹窗内容（Markdown 渲染）
- `PopupInput.vue` - 弹窗输入框
- `PopupActions.vue` - 弹窗操作按钮
- `EnhanceModal.vue` - 提示词增强弹窗
- `ZhiIndexPanel.vue` - 索引状态面板

### 设置组件 (`components/settings/`)
- `ThemeSettings.vue` - 主题设置
- `WindowSettings.vue` - 窗口设置
- `AudioSettings.vue` - 音频设置
- `FontSettings.vue` - 字体设置
- `TelegramSettings.vue` - Telegram 设置
- `ProxySettings.vue` - 代理设置
- `ShortcutSettings.vue` - 快捷键设置
- `CustomPromptSettings.vue` - 自定义提示词
- `ProjectIndexManager.vue` - 项目索引管理
- `VersionChecker.vue` - 版本检查

### 工具组件 (`components/tools/`)
- `IconWorkshop/` - 图标工坊（图标搜索、预览、保存）
- `EnhanceConfig.vue` - 提示词增强配置
- `MemoryConfig.vue` - 记忆管理配置
- `SouConfig.vue` - 搜索工具配置
- `Context7Config.vue` - Context7 配置
- `AcemcpLogViewerDrawer.vue` - Acemcp 日志查看器

### 标签页组件 (`components/tabs/`)
- `IntroTab.vue` - 介绍页
- `McpToolsTab.vue` - MCP 工具页
- `PromptsTab.vue` - 提示词页
- `SettingsTab.vue` - 设置页

---

## Composables (组合式函数)

### 核心 Composables
| 文件 | 职责 |
|------|------|
| `useAppManager.ts` | 统一应用管理器（整合所有 composables） |
| `useAppInitialization.ts` | 应用初始化逻辑 |
| `useMcpHandler.ts` | MCP 请求处理 |
| `useSettings.ts` | 设置管理 |
| `useTheme.ts` | 主题管理 |
| `useAudioManager.ts` | 音频管理 |
| `useEventHandlers.ts` | 事件处理器 |
| `useMcpTools.ts` | MCP 工具状态管理 |
| `useAcemcpSync.ts` | Acemcp 索引同步 |
| `useIconSearch.ts` | 图标搜索逻辑 |
| `useKeyboard.ts` | 键盘事件处理 |
| `useShortcuts.ts` | 快捷键管理 |
| `useFontManager.ts` | 字体管理 |
| `useVersionCheck.ts` | 版本检查 |
| `useProxyConfig.ts` | 代理配置 |
| `useLogViewer.ts` | 日志查看器 |
| `useRelativeTime.ts` | 相对时间格式化 |
| `useExitWarning.ts` | 退出警告 |

### 使用示例
```typescript
import { useAppManager } from './composables/useAppManager'

const {
  naiveTheme,
  mcpRequest,
  showMcpPopup,
  appConfig,
  actions
} = useAppManager()

// 初始化应用
await actions.app.initialize()

// 处理 MCP 响应
await actions.mcp.handleResponse(response)
```

---

## 主题系统

### 颜色系统 (`theme/colors.ts`)
```typescript
export const semanticColors = {
  surface: 'var(--color-surface)',
  primary: 'var(--color-primary)',
  success: 'var(--color-success)',
  warning: 'var(--color-warning)',
  error: 'var(--color-error)',
  info: 'var(--color-info)'
}
```

### 主题切换
- **深色模式**: 自动应用深色配色
- **浅色模式**: 自动应用浅色配色
- **持久化**: 主题选择保存到配置文件

---

## 数据模型

### MCP 请求类型 (`types/popup.d.ts`)
```typescript
interface McpRequest {
  id: string
  message: string
  predefined_options?: string[]
  is_markdown?: boolean
  project_root_path?: string
  uiux_intent?: string
  uiux_context_policy?: string
  uiux_reason?: string
}
```

### 图标类型 (`types/icon.ts`)
```typescript
interface IconItem {
  id: number
  name: string
  show_svg: string
  unicode: string
  font_class: string
}

interface IconSearchRequest {
  query: string
  style?: string
  fills?: string
  page?: number
  page_size?: number
}
```

---

## 常见问题 (FAQ)

### Q: 如何添加新的设置项？
A:
1. 在 `src/rust/config/settings.rs` 添加配置字段
2. 在 `components/settings/` 创建设置组件
3. 在 `SettingsTab.vue` 引入组件
4. 在 `useSettings.ts` 添加状态管理

### Q: 如何自定义主题颜色？
A: 修改 `theme/colors.ts` 中的 CSS 变量定义

### Q: 如何调试前端代码？
A: 使用浏览器开发者工具，或在 VS Code 中配置 Vite 调试

---

## 相关文件清单

### 核心文件
- `main.ts` - 应用入口
- `App.vue` - 根组件
- `vite.config.js` - 构建配置
- `uno.config.ts` - 样式配置

### 组件目录
- `components/layout/` - 布局组件
- `components/popup/` - 弹窗组件
- `components/settings/` - 设置组件
- `components/tools/` - 工具组件
- `components/tabs/` - 标签页组件
- `components/common/` - 通用组件

### 逻辑目录
- `composables/` - 组合式函数
- `theme/` - 主题系统
- `types/` - TypeScript 类型定义
- `constants/` - 常量定义
- `utils/` - 工具函数

---

**最后更新**: 2026-02-18
