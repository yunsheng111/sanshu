<script setup lang="ts">
/**
 * 记忆搜索组件
 * 支持关键词搜索、分类过滤、结果高亮
 */
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, ref, watch } from 'vue'

// Props
const props = defineProps<{
  projectRootPath: string
}>()

// Emits
const emit = defineEmits<{
  (e: 'select', memory: SearchResult): void
  (e: 'edit', memory: SearchResult): void
  (e: 'delete', memoryId: string): void
}>()

const message = useMessage()

// 类型定义
interface SearchResult {
  id: string
  content: string
  category: string
  created_at: string
  relevance: number
  highlight: string
}

// 状态
const query = ref('')
const selectedCategory = ref<string | null>(null)
const loading = ref(false)
const results = ref<SearchResult[]>([])
const hasSearched = ref(false)

// 分类选项
const categoryOptions = [
  { label: '全部分类', value: null },
  { label: '规范', value: '规范' },
  { label: '偏好', value: '偏好' },
  { label: '模式', value: '模式' },
  { label: '背景', value: '背景' },
]

// 计算属性
const isEmpty = computed(() => hasSearched.value && results.value.length === 0)
const hasResults = computed(() => results.value.length > 0)

// 搜索函数
async function search() {
  if (!query.value.trim()) {
    message.warning('请输入搜索关键词')
    return
  }

  loading.value = true
  hasSearched.value = true

  try {
    const res = await invoke<SearchResult[]>('search_memories', {
      projectPath: props.projectRootPath,
      query: query.value.trim(),
      category: selectedCategory.value,
    })
    results.value = res
  }
  catch (err) {
    message.error(`搜索失败: ${err}`)
    results.value = []
  }
  finally {
    loading.value = false
  }
}

// 清空搜索
function clearSearch() {
  query.value = ''
  results.value = []
  hasSearched.value = false
}

// 格式化日期
function formatDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString('zh-CN')
  }
  catch {
    return isoString
  }
}

// 格式化相关度
function formatRelevance(relevance: number): string {
  return `${Math.round(relevance * 100)}%`
}

// 获取相关度颜色
function getRelevanceType(relevance: number): 'success' | 'warning' | 'info' {
  if (relevance >= 0.6)
    return 'success'
  if (relevance >= 0.3)
    return 'warning'
  return 'info'
}

// 获取分类图标
function getCategoryIcon(category: string): string {
  const icons: Record<string, string> = {
    '规范': 'i-carbon-rule',
    '偏好': 'i-carbon-user-favorite',
    '模式': 'i-carbon-flow',
    '背景': 'i-carbon-document',
  }
  return icons[category] || 'i-carbon-document'
}

// 获取分类颜色
function getCategoryColor(category: string): string {
  const colors: Record<string, string> = {
    '规范': 'text-blue-500',
    '偏好': 'text-purple-500',
    '模式': 'text-green-500',
    '背景': 'text-orange-500',
  }
  return colors[category] || 'text-gray-500'
}

// 监听分类变化，自动重新搜索
watch(selectedCategory, () => {
  if (hasSearched.value && query.value.trim()) {
    search()
  }
})

// 暴露方法给父组件
defineExpose({
  search,
  clearSearch,
})
</script>

<template>
  <div class="memory-search" role="search" aria-label="记忆搜索">
    <!-- 搜索栏 -->
    <div class="search-bar">
      <n-input
        v-model:value="query"
        placeholder="输入关键词搜索记忆..."
        clearable
        class="search-input"
        aria-label="搜索关键词"
        @keyup.enter="search"
        @clear="clearSearch"
      >
        <template #prefix>
          <div class="i-carbon-search text-gray-400" aria-hidden="true" />
        </template>
      </n-input>

      <n-select
        v-model:value="selectedCategory"
        :options="categoryOptions"
        placeholder="分类"
        class="category-select"
        aria-label="分类筛选"
      />

      <n-button type="primary" :loading="loading" @click="search">
        搜索
      </n-button>
    </div>

    <!-- 搜索结果 -->
    <div class="search-results" role="region" aria-live="polite" aria-label="搜索结果">
      <!-- 加载状态 -->
      <div v-if="loading" class="loading-state" aria-busy="true">
        <n-spin size="medium" />
        <span class="ml-2">搜索中...</span>
      </div>

      <!-- 空结果 -->
      <div v-else-if="isEmpty" class="empty-state" role="status">
        <div class="i-carbon-search-locate text-4xl mb-2 opacity-20" aria-hidden="true" />
        <div class="text-sm opacity-60">
          未找到匹配的记忆
        </div>
      </div>

      <!-- 结果列表 -->
      <template v-else-if="hasResults">
        <div class="result-header">
          <span class="result-count" role="status">找到 {{ results.length }} 条结果</span>
        </div>

        <div class="result-list" role="listbox" aria-label="搜索结果列表">
          <div
            v-for="(item, index) in results"
            :key="item.id"
            class="result-item"
            role="option"
            :aria-selected="false"
            :tabindex="0"
            @click="emit('select', item)"
            @keyup.enter="emit('select', item)"
          >
            <!-- 头部：分类 + 相关度 -->
            <div class="result-meta">
              <div class="result-category">
                <div :class="[getCategoryIcon(item.category), getCategoryColor(item.category)]" aria-hidden="true" />
                <span>{{ item.category }}</span>
              </div>
              <n-tag :type="getRelevanceType(item.relevance)" size="small" :bordered="false">
                相关度 {{ formatRelevance(item.relevance) }}
              </n-tag>
            </div>

            <!-- 高亮内容 -->
            <div class="result-highlight">
              {{ item.highlight }}
            </div>

            <!-- 底部：时间 + 操作 -->
            <div class="result-footer">
              <span class="result-time">{{ formatDate(item.created_at) }}</span>
              <div class="result-actions">
                <n-button
                  text
                  type="primary"
                  size="tiny"
                  aria-label="编辑此记忆"
                  @click.stop="emit('edit', item)"
                >
                  <template #icon>
                    <div class="i-carbon-edit" aria-hidden="true" />
                  </template>
                  编辑
                </n-button>
                <n-button
                  text
                  type="error"
                  size="tiny"
                  aria-label="删除此记忆"
                  @click.stop="emit('delete', item.id)"
                >
                  <template #icon>
                    <div class="i-carbon-trash-can" aria-hidden="true" />
                  </template>
                  删除
                </n-button>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- 初始状态 -->
      <div v-else class="initial-state">
        <div class="i-carbon-search text-4xl mb-2 opacity-20" aria-hidden="true" />
        <div class="text-sm opacity-60">
          输入关键词开始搜索
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.memory-search {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 搜索栏 */
.search-bar {
  display: flex;
  gap: 12px;
  align-items: center;
}

.search-input {
  flex: 1;
}

.category-select {
  width: 120px;
}

/* 搜索结果 */
.search-results {
  min-height: 200px;
}

.loading-state,
.empty-state,
.initial-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  color: var(--color-on-surface-muted, #9ca3af);
}

.result-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.result-count {
  font-size: 13px;
  color: var(--color-on-surface-secondary, #6b7280);
}

.result-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-item {
  padding: 12px 16px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
  cursor: pointer;
  transition: all 0.2s ease;
}

.result-item:hover {
  border-color: var(--color-primary, #3b82f6);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
}

:root.dark .result-item {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

:root.dark .result-item:hover {
  border-color: var(--color-primary, #60a5fa);
}

.result-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.result-category {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
}

.result-highlight {
  font-size: 13px;
  line-height: 1.6;
  color: var(--color-on-surface, #111827);
  word-break: break-word;
}

:root.dark .result-highlight {
  color: #e5e7eb;
}

.result-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.1));
}

.result-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
}

.result-actions {
  display: flex;
  gap: 8px;
}

.result-item:focus {
  outline: 2px solid var(--color-primary, #3b82f6);
  outline-offset: 2px;
}
</style>
