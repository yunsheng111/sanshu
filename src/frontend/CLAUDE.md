# 前端模块 (src/frontend)

[根目录](../../CLAUDE.md) > **frontend**

---

## 模块职责

Vue 3 前端应用，提供三术工具的图形化界面，包括 MCP 弹窗交互、设置管理、主题切换、图标工坊、记忆管理、搜索预览等功能。新增 i18n 国际化、Pinia 状态管理、vitest 测试框架和 IPC 弹性调用基础设施。

---

## 入口与启动

### 主入口
- **文件**: `main.ts`
- **职责**: 初始化 Vue 应用，注册 Naive UI 组件、Pinia 状态管理、i18n 国际化，挂载到 DOM

### 根组件
- **文件**: `App.vue`
- **职责**: 应用容器，管理主题、MCP 请求、事件处理

---

## 对外接口

### Tauri 命令调用（推荐使用 useSafeInvoke）
```typescript
import { useSafeInvoke } from './composables/useSafeInvoke'

const { safeInvoke, error, loading } = useSafeInvoke()

// 带超时和错误处理的 IPC 调用
const config = await safeInvoke<AppConfig>('get_config')
```

### 事件监听
```typescript
import { listen } from '@tauri-apps/api/event'

await listen('mcp_request', (event) => { /* 处理 MCP 请求 */ })
await listen('config_reloaded', () => { /* 重新加载配置 */ })
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
  "highlight.js": "^11.11.1",
  "pinia": "^2.x",
  "vue-i18n": "^9.x"
}
```

### 开发依赖（P3 新增）
```json
{
  "vitest": "^x.x",
  "@vitejs/plugin-vue": "^x.x",
  "happy-dom": "^x.x"
}
```

### 构建配置
- **文件**: `vite.config.js`
- **端口**: 5176 (dev), 5177 (HMR)
- **测试配置**: `vitest.config.ts`（happy-dom 环境，v8 覆盖率）

---

## 组件结构

### 布局组件 (`components/layout/`)
- `LayoutWrapper.vue` - 布局容器
- `MainLayout.vue` - 主布局（标签页导航）

### 弹窗组件 (`components/popup/`)
- `McpPopup.vue` - MCP 交互弹窗（核心组件）
- `PopupHeader.vue` / `PopupContent.vue` / `PopupInput.vue` / `PopupActions.vue`
- `EnhanceModal.vue` - 提示词增强弹窗
- `ZhiIndexPanel.vue` / `McpIndexStatusDrawer.vue`
- `enhance/` - EnhanceConfigPanel / EnhancePreview / EnhanceResult

### 设置组件 (`components/settings/`)
- ThemeSettings / WindowSettings / AudioSettings / FontSettings
- TelegramSettings / ProxySettings / ShortcutSettings
- CustomPromptSettings / ProjectIndexManager / VersionChecker / ReplySettings

### 工具组件 (`components/tools/`) - **P1-P3 新增 4 个**
- `IconWorkshop/` - 图标工坊（搜索、预览、保存）
- `EnhanceConfig.vue` - 提示词增强配置
- `MemoryConfig.vue` - 记忆管理配置
- `MemorySearch.vue` - **P3 新增** 记忆搜索 UI
- `SearchPreview.vue` - **P3 新增** 搜索结果预览
- `MemoryList.vue` - **P3 新增** 记忆列表 UI（含 ARIA 无障碍）
- `SouConfig.vue` - 搜索工具配置
- `Context7Config.vue` / `AcemcpLogViewerDrawer.vue` / `SouProxySettingsModal.vue`

### 标签页组件 (`components/tabs/`)
- IntroTab / McpToolsTab / PromptsTab / SettingsTab

### 通用组件 (`components/common/`)
- ConfigSection / FeatureCard / ProjectInfoCard / SkeletonLoader / ThemeIcon / UpdateModal

---

## Composables (组合式函数) - **P3 新增 2 个**

| 文件 | 职责 |
|------|------|
| `useAppManager.ts` | 统一应用管理器 |
| `useAppInitialization.ts` | 应用初始化 |
| `useMcpHandler.ts` | MCP 请求处理 |
| `useSettings.ts` | 设置管理 |
| `useTheme.ts` | 主题管理 |
| `useAudioManager.ts` | 音频管理 |
| `useEventHandlers.ts` | 事件处理器 |
| `useMcpTools.ts` | MCP 工具状态 |
| `useAcemcpSync.ts` | Acemcp 索引同步 |
| `useIconSearch.ts` | 图标搜索 |
| `useKeyboard.ts` | 键盘事件 |
| `useShortcuts.ts` | 快捷键管理 |
| `useFontManager.ts` | 字体管理 |
| `useVersionCheck.ts` | 版本检查 |
| `useProxyConfig.ts` | 代理配置 |
| `useLogViewer.ts` | 日志查看器 |
| `useRelativeTime.ts` | 相对时间格式化 |
| `useExitWarning.ts` | 退出警告 |
| **`useSafeInvoke.ts`** | **P3 新增** IPC 弹性调用（超时 + 错误 + 加载状态） |
| **`useSearchFeedback.ts`** | **P3 新增** 搜索实时反馈（阶段进度 + 状态） |

---

## 新增基础设施（P3）

### 1. i18n 国际化 (`i18n/`)

```typescript
// i18n/index.ts
import { createI18n } from 'vue-i18n'
import zh from './zh'
import en from './en'

export const i18n = createI18n<[MessageSchema], 'zh' | 'en'>({
  legacy: false,
  locale: 'zh',
  fallbackLocale: 'en',
  messages: { zh, en },
})
```

- **默认语言**: 中文 (zh)
- **回退语言**: 英文 (en)
- **使用方式**: `const { t } = useI18n()`

### 2. Pinia 状态管理 (`stores/`)

```typescript
// stores/searchStore.ts
export const useSearchStore = defineStore('search', {
  state: () => ({
    history: [],         // 搜索历史（最多 50 条）
    lastQuery: '',       // 上次搜索查询
    preferences: { ... } // 用户偏好（语义搜索、最大结果数、文件类型）
  }),
  getters: { recentSearches, ... },
  actions: { addSearch, clearHistory, ... }
})
```

### 3. IPC 弹性调用 (`useSafeInvoke.ts`)

```typescript
const { safeInvoke, error, loading } = useSafeInvoke()

// 自动超时控制（默认 30s）
// 错误状态管理
// 加载状态跟踪
const result = await safeInvoke<T>('command', args, { timeout: 30000, silent: false })
```

### 4. 搜索实时反馈 (`useSearchFeedback.ts`)

```typescript
type SearchPhase = 'idle' | 'indexing' | 'searching' | 'ranking' | 'completed' | 'error'

const { phase, progress, setPhase, setProgress, reset } = useSearchFeedback()
```

---

## 测试策略（P3 新增）

### 测试框架
- **运行器**: vitest
- **环境**: happy-dom
- **覆盖率**: v8 provider

### 测试文件
- `components/tools/__tests__/EnhanceConfig.spec.ts` - 增强配置组件测试
- `components/tools/__tests__/SouConfig.spec.ts` - 搜索配置组件测试
- `components/tools/MemoryList.spec.ts` - 记忆列表组件测试
- `composables/useSafeInvoke.spec.ts` - IPC 弹性调用测试

### 测试运行
```bash
pnpm vitest              # 运行所有测试
pnpm vitest --coverage   # 运行测试并生成覆盖率报告
```

### 测试配置 (`vitest.config.ts`)
```typescript
export default defineConfig({
  plugins: [Vue()],
  test: {
    globals: true,
    environment: 'happy-dom',
    include: ['src/frontend/**/*.{test,spec}.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      include: ['src/frontend/**/*.{ts,vue}'],
      exclude: ['src/frontend/types/**', 'src/frontend/**/*.d.ts', 'src/frontend/test/**'],
    },
  },
})
```

---

## 主题系统

### 颜色系统 (`theme/colors.ts`)
- 语义化颜色：surface, primary, success, warning, error, info
- 深色/浅色模式自动切换
- 主题选择持久化到配置文件

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
}
```

---

## 相关文件清单

### 核心文件
- `main.ts` - 应用入口
- `App.vue` - 根组件
- `vite.config.js` - 构建配置
- `uno.config.ts` - 样式配置
- `vitest.config.ts` - P3 测试配置

### 组件目录
- `components/layout/` - 布局组件 (2)
- `components/popup/` - 弹窗组件 (11)
- `components/settings/` - 设置组件 (11)
- `components/tools/` - 工具组件 (12，含 3 个 P3 新增)
- `components/tabs/` - 标签页组件 (4)
- `components/common/` - 通用组件 (6)
- `components/index/` - 首页组件 (3)

### 逻辑目录
- `composables/` - 组合式函数 (20，含 2 个 P3 新增)
- `stores/` - P3 Pinia 状态管理 (2)
- `i18n/` - P3 国际化 (3)
- `theme/` - 主题系统 (2)
- `types/` - TypeScript 类型定义 (3)
- `constants/` - 常量定义 (3)
- `utils/` - 工具函数 (1)
- `test/` - 测试页面和配置 (6)

---

**最后更新**: 2026-02-19
