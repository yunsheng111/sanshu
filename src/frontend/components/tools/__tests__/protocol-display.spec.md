# 前端协议标注显示逻辑测试说明

## 测试目标

测试 `EnhanceConfig.vue` 和 `SouConfig.vue` 中的协议类型标注显示逻辑。

---

## EnhanceConfig.vue - PROVIDER_PROTOCOL 测试

### 测试数据结构

```typescript
const PROVIDER_PROTOCOL: Record<string, { type: 'info' | 'success' | 'warning', desc: string }> = {
  openai: { type: 'info', desc: 'OpenAI 原生格式（/chat/completions）' },
  grok: { type: 'info', desc: 'Grok (xAI) 兼容 OpenAI 格式，可直接使用 OpenAI SDK' },
  deepseek: { type: 'info', desc: 'DeepSeek 兼容 OpenAI 格式（/chat/completions）' },
  siliconflow: { type: 'info', desc: 'SiliconFlow 兼容 OpenAI 格式，支持多种开源模型' },
  groq: { type: 'info', desc: 'Groq 兼容 OpenAI 格式，推理速度极快' },
  gemini: { type: 'success', desc: 'Google Gemini 原生格式（generateContent），与 OpenAI 不兼容' },
  anthropic: { type: 'success', desc: 'Anthropic Claude 原生格式（/messages），与 OpenAI 不兼容' },
}
```

### 测试用例

#### 1. 正常路径测试

**测试 1.1: OpenAI 兼容供应商显示 info 类型**
```typescript
// Given: 用户选择 openai/grok/deepseek/siliconflow/groq
// When: 渲染协议标注 n-alert
// Then:
//   - type 属性为 'info'
//   - desc 包含 "兼容 OpenAI 格式" 或 "OpenAI 原生格式"
//   - n-alert 组件可见
```

**测试 1.2: 原生格式供应商显示 success 类型**
```typescript
// Given: 用户选择 gemini 或 anthropic
// When: 渲染协议标注 n-alert
// Then:
//   - type 属性为 'success'
//   - desc 包含 "原生格式" 和 "与 OpenAI 不兼容"
//   - n-alert 组件可见
```

**测试 1.3: Grok 特殊说明**
```typescript
// Given: 用户选择 grok
// When: 渲染协议标注
// Then:
//   - desc 为 'Grok (xAI) 兼容 OpenAI 格式，可直接使用 OpenAI SDK'
//   - 明确标注可使用 OpenAI SDK
```

#### 2. 边界条件测试

**测试 2.1: 未定义供应商的 fallback 行为**
```typescript
// Given: config.provider = 'unknown_provider'
// When: 尝试访问 PROVIDER_PROTOCOL[config.provider]
// Then:
//   - 返回 undefined
//   - v-if 条件阻止 n-alert 渲染
//   - 不显示协议标注（优雅降级）
```

**测试 2.2: 空字符串供应商**
```typescript
// Given: config.provider = ''
// When: 尝试访问 PROVIDER_PROTOCOL['']
// Then:
//   - 返回 undefined
//   - n-alert 不渲染
```

#### 3. 交互测试

**测试 3.1: 切换供应商时协议标注更新**
```typescript
// Given: 初始 provider = 'openai'
// When: 用户切换到 'gemini'
// Then:
//   - n-alert type 从 'info' 变为 'success'
//   - desc 从 "OpenAI 原生格式" 变为 "Google Gemini 原生格式"
```

**测试 3.2: 从云端 API 切换到 Ollama**
```typescript
// Given: isCloudProvider = true, provider = 'grok'
// When: 用户切换到 Ollama 模式
// Then:
//   - 云端 API 配置区域隐藏（v-if="isCloudProvider" 为 false）
//   - 协议标注 n-alert 不再显示
```

---

## SouConfig.vue - EMBEDDING_PROTOCOL 测试

### 测试数据结构

```typescript
const EMBEDDING_PROTOCOL: Record<string, { type: 'info' | 'success' | 'warning', desc: string }> = {
  jina: { type: 'info', desc: 'Jina AI 兼容 OpenAI Embeddings 格式（/embeddings）' },
  siliconflow: { type: 'info', desc: 'SiliconFlow 兼容 OpenAI Embeddings 格式，支持 BGE 等多种模型' },
  ollama: { type: 'success', desc: 'Ollama 本地嵌入，无需 API Key，数据不出本机' },
  cloudflare: { type: 'info', desc: 'Cloudflare AI Gateway，兼容 OpenAI Embeddings 格式' },
  nomic: { type: 'info', desc: 'Nomic Atlas 兼容 OpenAI Embeddings 格式' },
  cohere: { type: 'warning', desc: 'Cohere 使用原生格式（/embed），与 OpenAI 不兼容' },
}
```

### 测试用例

#### 1. 正常路径测试

**测试 1.1: OpenAI Embeddings 兼容提供者显示 info 类型**
```typescript
// Given: souConfig.embedding_provider = 'jina' | 'siliconflow' | 'cloudflare' | 'nomic'
// When: 渲染协议标注 n-alert
// Then:
//   - type 属性为 'info'
//   - desc 包含 "兼容 OpenAI Embeddings 格式"
//   - n-alert 组件可见
```

**测试 1.2: Ollama 本地嵌入显示 success 类型**
```typescript
// Given: souConfig.embedding_provider = 'ollama'
// When: 渲染协议标注 n-alert
// Then:
//   - type 属性为 'success'
//   - desc 为 'Ollama 本地嵌入，无需 API Key，数据不出本机'
//   - 强调本地化和隐私保护
```

**测试 1.3: Cohere 原生格式显示 warning 类型**
```typescript
// Given: souConfig.embedding_provider = 'cohere'
// When: 渲染协议标注 n-alert
// Then:
//   - type 属性为 'warning'
//   - desc 为 'Cohere 使用原生格式（/embed），与 OpenAI 不兼容'
//   - 警告用户不兼容 OpenAI Embeddings 格式
```

#### 2. 边界条件测试

**测试 2.1: 未定义嵌入提供者的 fallback**
```typescript
// Given: souConfig.embedding_provider = 'unknown_embedding'
// When: 尝试访问 EMBEDDING_PROTOCOL[souConfig.embedding_provider]
// Then:
//   - 返回 undefined
//   - v-if 条件阻止 n-alert 渲染
//   - 不显示协议标注
```

**测试 2.2: 仅在 local 模式显示**
```typescript
// Given: souConfig.mode = 'acemcp'
// When: 渲染本地嵌入 tab
// Then:
//   - v-if="isLocalMode" 为 false
//   - 整个嵌入提供者配置区域隐藏
//   - 协议标注不显示
```

#### 3. 交互测试

**测试 3.1: 切换嵌入提供者时协议标注更新**
```typescript
// Given: 初始 embedding_provider = 'jina'
// When: 用户切换到 'cohere'
// Then:
//   - n-alert type 从 'info' 变为 'warning'
//   - desc 从 "Jina AI 兼容..." 变为 "Cohere 使用原生格式..."
```

**测试 3.2: 从 ACE 模式切换到本地模式**
```typescript
// Given: souConfig.mode = 'acemcp'
// When: 用户切换到 mode = 'local'
// Then:
//   - isLocalMode 变为 true
//   - 嵌入提供者配置区域显示
//   - 协议标注 n-alert 显示
```

**测试 3.3: Ollama 选择时 API Key 输入框隐藏**
```typescript
// Given: souConfig.embedding_provider = 'jina'
// When: 用户切换到 'ollama'
// Then:
//   - isOllamaEmbedding 变为 true
//   - v-if="!isOllamaEmbedding" 的 API Key 输入框隐藏
//   - 协议标注显示 success 类型，强调无需 API Key
```

---

## 实施建议

### 使用 Vitest + Vue Test Utils

```typescript
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import EnhanceConfig from '../EnhanceConfig.vue'

describe('enhanceConfig - PROVIDER_PROTOCOL', () => {
  it('should display info alert for OpenAI compatible providers', () => {
    const wrapper = mount(EnhanceConfig, {
      props: { active: true },
      data() {
        return {
          config: { provider: 'grok' }
        }
      }
    })

    const alert = wrapper.find('n-alert')
    expect(alert.exists()).toBe(true)
    expect(alert.props('type')).toBe('info')
    expect(alert.text()).toContain('兼容 OpenAI 格式')
  })

  it('should display success alert for native format providers', () => {
    const wrapper = mount(EnhanceConfig, {
      data() {
        return {
          config: { provider: 'gemini' }
        }
      }
    })

    const alert = wrapper.find('n-alert')
    expect(alert.props('type')).toBe('success')
    expect(alert.text()).toContain('原生格式')
    expect(alert.text()).toContain('与 OpenAI 不兼容')
  })

  it('should not display alert for undefined provider', () => {
    const wrapper = mount(EnhanceConfig, {
      data() {
        return {
          config: { provider: 'unknown' }
        }
      }
    })

    const alert = wrapper.find('n-alert')
    expect(alert.exists()).toBe(false)
  })
})
```

### 手动测试清单

#### EnhanceConfig.vue
- [ ] 选择 openai，查看 info 类型标注
- [ ] 选择 grok，确认显示 "可直接使用 OpenAI SDK"
- [ ] 选择 deepseek，查看 info 类型标注
- [ ] 选择 siliconflow，查看 info 类型标注
- [ ] 选择 groq，确认显示 "推理速度极快"
- [ ] 选择 gemini，查看 success 类型标注和 "与 OpenAI 不兼容"
- [ ] 选择 anthropic，查看 success 类型标注
- [ ] 切换到 Ollama 模式，确认云端 API 区域隐藏

#### SouConfig.vue
- [ ] 切换到本地嵌入 tab
- [ ] 选择 jina，查看 info 类型标注
- [ ] 选择 siliconflow，查看 info 类型标注
- [ ] 选择 ollama，查看 success 类型标注和 "无需 API Key"
- [ ] 选择 ollama，确认 API Key 输入框隐藏
- [ ] 选择 cloudflare，查看 info 类型标注
- [ ] 选择 nomic，查看 info 类型标注
- [ ] 选择 cohere，查看 warning 类型标注和 "与 OpenAI 不兼容"
- [ ] 切换回 ACE 模式，确认嵌入提供者配置区域隐藏

---

## 覆盖率目标

- **正常路径**: 100% (所有供应商类型都有测试)
- **边界条件**: 100% (未定义供应商、空字符串、模式切换)
- **异常路径**: 100% (优雅降级、v-if 条件)
- **交互路径**: 100% (供应商切换、模式切换)

---

## 注意事项

1. **协议标注位置**: 标注应显示在供应商选择下拉框之后、API 端点输入框之前
2. **响应式更新**: 切换供应商时，标注应立即更新，无需刷新页面
3. **类型颜色**: info (蓝色), success (绿色), warning (黄色)
4. **图标**: 所有标注使用 `i-carbon-information` 图标
5. **优雅降级**: 未定义供应商时不显示标注，不抛出错误
