<!-- SC-9: 搜索结果预览组件 -->
<script setup lang="ts">
import { NCard, NSpace, NTag, NText } from 'naive-ui'
import { computed } from 'vue'

interface SearchResult {
  filePath: string
  lineNumber: number
  content: string
  score?: number
  matches?: Array<{ start: number, end: number }>
}

const props = defineProps<{
  results: SearchResult[]
  keyword?: string
  loading?: boolean
}>()

// 提取文件名
function getFileName(path: string): string {
  return path.split(/[/\\]/).pop() || path
}

// 提取目录路径
function getDirectory(path: string): string {
  const parts = path.split(/[/\\]/)
  parts.pop()
  return parts.join('/') || '.'
}

// 高亮关键词
function highlightKeyword(text: string, keyword: string | undefined): string {
  if (!keyword)
    return text
  const escaped = keyword.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const regex = new RegExp(`(${escaped})`, 'gi')
  return text.replace(regex, '<mark class="bg-yellow-200 dark:bg-yellow-800">$1</mark>')
}

// 格式化评分
function formatScore(score: number | undefined): string {
  if (score === undefined)
    return ''
  return `${Math.round(score * 100)}%`
}

const sortedResults = computed(() => {
  return [...props.results].sort((a, b) => (b.score || 0) - (a.score || 0))
})
</script>

<template>
  <div class="search-preview" role="listbox" aria-label="搜索结果">
    <div v-if="loading" class="flex items-center justify-center py-8">
      <span class="i-carbon-circle-dash animate-spin mr-2" />
      <span>搜索中...</span>
    </div>

    <div v-else-if="results.length === 0" class="text-center py-8 text-gray-500">
      无搜索结果
    </div>

    <div v-else class="space-y-2">
      <NCard
        v-for="(result, index) in sortedResults"
        :key="`${result.filePath}:${result.lineNumber}`"
        size="small"
        hoverable
        role="option"
        :aria-selected="false"
        :tabindex="0"
        class="search-result-item"
      >
        <!-- 文件路径面包屑 -->
        <template #header>
          <NSpace align="center" :size="4">
            <span class="i-carbon-document text-blue-500" />
            <NText strong>
              {{ getFileName(result.filePath) }}
            </NText>
            <NText depth="3" class="text-xs">
              {{ getDirectory(result.filePath) }}
            </NText>
            <NTag v-if="result.score" size="small" type="success">
              {{ formatScore(result.score) }}
            </NTag>
            <NTag size="small" :bordered="false">
              L{{ result.lineNumber }}
            </NTag>
          </NSpace>
        </template>

        <!-- 代码预览 -->
        <pre
          class="text-sm font-mono bg-gray-50 dark:bg-gray-800 p-2 rounded overflow-x-auto"
          v-html="highlightKeyword(result.content, keyword)"
        />
      </NCard>
    </div>
  </div>
</template>

<style scoped>
.search-result-item:focus {
  outline: 2px solid var(--n-color-target);
  outline-offset: 2px;
}

/* SC-11: 响应式设计适配 400px ~ 1920px */
@media (max-width: 640px) {
  .search-preview :deep(.n-card) {
    padding: 8px;
  }

  .search-preview :deep(.n-space) {
    flex-wrap: wrap;
  }

  pre {
    font-size: 12px;
    padding: 8px !important;
  }
}

@media (min-width: 641px) and (max-width: 1024px) {
  .search-preview :deep(.n-card) {
    padding: 12px;
  }
}

@media (min-width: 1025px) {
  .search-preview {
    max-width: 1200px;
    margin: 0 auto;
  }
}
</style>
