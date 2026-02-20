# HighlightText 组件使用示例

## 基本用法

```vue
<script setup lang="ts">
import HighlightText from './HighlightText.vue'

const snippet = '这是一个<mark>测试</mark>片段'
</script>

<template>
  <HighlightText :snippet="snippet" />
</template>
```

## 自定义高亮样式

```vue
<template>
  <HighlightText
    :snippet="snippet"
    highlight-class="custom-highlight"
  />
</template>

<style>
.custom-highlight {
  background-color: #ff6b6b;
  color: white;
  padding: 2px 4px;
  border-radius: 4px;
}
</style>
```

## 在 MemorySearch.vue 中集成

```vue
<script setup lang="ts">
import { useMemorySearch } from '@/composables/useMemorySearch'
import HighlightText from '@/components/HighlightText.vue'

const { results, searchMode } = useMemorySearch()
</script>

<template>
  <div v-for="result in results" :key="result.id">
    <!-- 显示搜索模式指示器 -->
    <div class="search-mode-indicator">
      <span v-if="searchMode === 'fts5'">🔍+ FTS5 全文搜索</span>
      <span v-else>🔍 模糊匹配</span>
    </div>

    <!-- 使用 HighlightText 渲染高亮片段 -->
    <div class="result-content">
      <HighlightText
        v-if="result.highlighted_snippet"
        :snippet="result.highlighted_snippet"
      />
      <span v-else>{{ result.content }}</span>
    </div>
  </div>
</template>
```

## XSS 安全保证

组件会自动转义所有危险内容：

```vue
<!-- 输入 -->
<HighlightText snippet='<script>alert("xss")</script><mark>测试</mark>' />

<!-- 输出（安全） -->
<!-- <script> 标签被转义，无法执行 -->
&lt;script&gt;alert("xss")&lt;/script&gt;<span class="highlight">测试</span>
```

## API

### Props

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `snippet` | `string` | - | FTS5 返回的高亮片段（包含 `<mark>` 标签） |
| `highlightClass` | `string` | `'highlight'` | 高亮部分的 CSS 类名 |

### 样式变量

可通过 CSS 变量自定义高亮颜色：

```css
:root {
  --highlight-bg: #ffeb3b;           /* 浅色模式背景色 */
  --highlight-color: #000;           /* 浅色模式文字色 */
  --highlight-bg-dark: #fbc02d;      /* 深色模式背景色 */
  --highlight-color-dark: #000;      /* 深色模式文字色 */
}
```

## 技术细节

### 解析器工作原理

1. 使用正则 `/<mark>(.*?)<\/mark>/g` 提取高亮标签
2. `split()` 将字符串分割为 `[非匹配, 匹配, 非匹配, 匹配, ...]`
3. 保留原始索引判断 `isMatch`（奇数索引为匹配部分）
4. Vue `h()` 函数自动转义所有文本内容，防止 XSS

### 为什么不使用 v-html？

- `v-html` 会直接渲染 HTML，存在 XSS 风险
- 即使手动转义，也可能遗漏某些攻击向量
- 使用 VNode 渲染是 Vue 推荐的安全方式

### 测试覆盖

- ✅ 基本高亮渲染
- ✅ 多个高亮片段
- ✅ XSS 注入防护（script、iframe、onerror、javascript:）
- ✅ 边界情况（空片段、无高亮、嵌套标签）
- ✅ 特殊字符转义
- ✅ 中文和 Unicode 支持

## 相关文件

- 组件：`src/frontend/components/HighlightText.vue`
- 解析器：`src/frontend/utils/snippetParser.ts`
- 测试：`src/frontend/components/__tests__/HighlightText.spec.ts`
