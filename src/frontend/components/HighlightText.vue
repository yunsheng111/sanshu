<script setup lang="ts">
import type { SnippetPart } from '../utils/snippetParser'
/**
 * 高亮文本组件
 * XSS 安全的 FTS5 片段渲染器
 *
 * 约束：
 * - SC-18: 禁止使用 v-html，使用 VNode 渲染
 * - OK-24: XSS 防护验收标准
 *
 * 使用示例：
 * <HighlightText snippet="这是一个<mark>测试</mark>片段" />
 */
import { computed, h } from 'vue'
import { parseFts5Snippet } from '../utils/snippetParser'

interface Props {
  /** FTS5 返回的高亮片段（包含 <mark> 标签） */
  snippet: string
  /** 高亮部分的 CSS 类名（默认：highlight） */
  highlightClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  highlightClass: 'highlight',
})

/**
 * 解析后的片段数组
 */
const parts = computed<SnippetPart[]>(() => {
  return parseFts5Snippet(props.snippet)
})

/**
 * 渲染函数
 * 使用 VNode 而非 v-html，确保 XSS 安全
 */
const renderContent = computed(() => {
  return parts.value.map((part, index) => {
    if (part.isMatch) {
      // 高亮部分：使用 <span class="highlight">
      return h(
        'span',
        {
          key: `match-${index}`,
          class: props.highlightClass,
        },
        part.text,
      )
    }
    else {
      // 普通文本：直接渲染
      return h(
        'span',
        {
          key: `text-${index}`,
        },
        part.text,
      )
    }
  })
})
</script>

<template>
  <span class="highlight-text">
    <!-- 使用 render 函数渲染 VNode 数组 -->
    <component :is="() => renderContent" />
  </span>
</template>

<style scoped>
.highlight-text {
  display: inline;
  word-break: break-word;
}

/**
 * 高亮样式
 * 支持浅色和深色模式
 */
.highlight {
  background-color: var(--highlight-bg, #ffeb3b);
  color: var(--highlight-color, #000);
  padding: 0 2px;
  border-radius: 2px;
  font-weight: 500;
}

/**
 * 深色模式适配
 */
@media (prefers-color-scheme: dark) {
  .highlight {
    background-color: var(--highlight-bg-dark, #fbc02d);
    color: var(--highlight-color-dark, #000);
  }
}

/**
 * 主题色适配（如果项目有主题系统）
 * 可通过 CSS 变量覆盖默认颜色
 */
:root {
  --highlight-bg: #ffeb3b;
  --highlight-color: #000;
  --highlight-bg-dark: #fbc02d;
  --highlight-color-dark: #000;
}
</style>
