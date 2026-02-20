<script setup lang="ts">
/**
 * 记忆搜索组件（增强版）
 * 支持关键词搜索、分类过滤、结果高亮
 * 增强功能：
 * - @domain 前缀按域过滤
 * - #tag 前缀按标签过滤
 * - 300ms debounce 实时搜索
 * - 正则解析前缀，失败时降级为纯全文搜索
 */
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, onUnmounted, ref, watch } from 'vue'

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
  domain?: string
  tags?: string[]
}

/** 解析后的搜索参数 */
interface ParsedQuery {
  /** 纯文本搜索关键词 */
  text: string
  /** @domain 前缀提取的域名 */
  domain: string | null
  /** #tag 前缀提取的标签列表 */
  tags: string[]
}

// 状态
const query = ref('')
const selectedCategory = ref<string | null>(null)
const loading = ref(false)
const results = ref<SearchResult[]>([])
const hasSearched = ref(false)

/** debounce 定时器 */
let debounceTimer: ReturnType<typeof setTimeout> | null = null

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

/** 解析后的查询信息（用于搜索栏下方显示） */
const parsedInfo = computed(() => {
  const parsed = parseQuery(query.value)
  const parts: string[] = []
  if (parsed.domain) parts.push(`域: ${parsed.domain}`)
  if (parsed.tags.length > 0) parts.push(`标签: ${parsed.tags.join(', ')}`)
  if (parsed.text) parts.push(`关键词: ${parsed.text}`)
  return parts.length > 0 ? parts.join(' | ') : ''
})

// ============ 查询解析 ============

/**
 * 解析搜索查询字符串
 * 支持 @domain 和 #tag 前缀语法
 * 解析失败时降级为纯全文搜索
 */
function parseQuery(input: string): ParsedQuery {
  const result: ParsedQuery = { text: '', domain: null, tags: [] }
  if (!input.trim()) return result

  try {
    let remaining = input.trim()

    // 提取 @domain 前缀（只取第一个）
    const domainMatch = remaining.match(/@(\S+)/)
    if (domainMatch) {
      result.domain = domainMatch[1]
      remaining = remaining.replace(domainMatch[0], '').trim()
    }

    // 提取所有 #tag 前缀
    const tagRegex = /#(\S+)/g
    let tagMatch: RegExpExecArray | null
    while ((tagMatch = tagRegex.exec(remaining)) !== null) {
      result.tags.push(tagMatch[1])
    }
    // 移除所有 #tag
    remaining = remaining.replace(/#\S+/g, '').trim()

    // 剩余部分为纯文本搜索关键词
    result.text = remaining
  } catch {
    // 正则解析失败，降级为纯全文搜索
    result.text = input.trim()
    result.domain = null
    result.tags = []
  }

  return result
}

// ============ 搜索函数 ============

async function search() {
  const trimmed = query.value.trim()
  if (!trimmed) {
    // 空查询时不搜索，但不弹提示（因为 debounce 会频繁触发）
    return
  }

  loading.value = true
  hasSearched.value = true

  try {
    const parsed = parseQuery(trimmed)

    const res = await invoke<SearchResult[]>('search_memories', {
      projectPath: props.projectRootPath,
      query: parsed.text || trimmed, // 如果解析后无文本，用原始输入
      category: selectedCategory.value,
      domain: parsed.domain,
      tags: parsed.tags.length > 0 ? parsed.tags : undefined,
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

/** 手动触发搜索（用于按钮和回车） */
function manualSearch() {
  if (!query.value.trim()) {
    message.warning('请输入搜索关键词')
    return
  }
  // 取消 debounce 定时器，立即搜索
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  search()
}

// 清空搜索
function clearSearch() {
  query.value = ''
  results.value = []
  hasSearched.value = false
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
}

// ============ debounce 实时搜索 ============

watch(query, (newVal) => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
  }

  if (!newVal.trim()) {
    // 输入清空时直接清除结果
    results.value = []
    hasSearched.value = false
    return
  }

  // 300ms debounce
  debounceTimer = setTimeout(() => {
    search()
  }, 300)
})

// 组件卸载时清理定时器
onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
  }
})

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

// 获取分类色条颜色
function getCategoryAccentColor(category: string): string {
  const colors: Record<string, string> = {
    '规范': 'linear-gradient(180deg, #3b82f6, #60a5fa)',
    '偏好': 'linear-gradient(180deg, #a855f7, #c084fc)',
    '模式': 'linear-gradient(180deg, #22c55e, #4ade80)',
    '背景': 'linear-gradient(180deg, #f97316, #fb923c)',
  }
  return colors[category] || 'linear-gradient(180deg, #9ca3af, #d1d5db)'
}

// 获取分类标签背景
function getCategoryBgClass(category: string): string {
  const classes: Record<string, string> = {
    '规范': 'category-badge--rule',
    '偏好': 'category-badge--preference',
    '模式': 'category-badge--pattern',
    '背景': 'category-badge--context',
  }
  return classes[category] || ''
}

// 监听分类变化，自动重新搜索
watch(selectedCategory, () => {
  if (hasSearched.value && query.value.trim()) {
    search()
  }
})

// 暴露方法给父组件
defineExpose({
  search: manualSearch,
  clearSearch,
})
</script>

<template>
  <div class="memory-search" role="search" aria-label="记忆搜索">
    <!-- 搜索栏容器 -->
    <div class="search-container">
      <div class="search-bar">
        <n-input
          v-model:value="query"
          placeholder="搜索记忆... 使用 @域名 或 #标签 精确筛选"
          clearable
          class="search-input"
          aria-label="搜索关键词（支持 @domain 和 #tag 前缀）"
          @keyup.enter="manualSearch"
          @clear="clearSearch"
        >
          <template #prefix>
            <div class="i-carbon-search search-icon" aria-hidden="true" />
          </template>
        </n-input>

        <n-select
          v-model:value="selectedCategory"
          :options="categoryOptions"
          placeholder="分类"
          class="category-select"
          aria-label="分类筛选"
        />

        <n-button type="primary" :loading="loading" class="search-btn" @click="manualSearch">
          搜索
        </n-button>
      </div>

      <!-- 搜索前缀解析提示 -->
      <div v-if="parsedInfo && query.trim()" class="parsed-info" role="status">
        <div class="i-carbon-information parsed-info-icon" aria-hidden="true" />
        <span>{{ parsedInfo }}</span>
      </div>
    </div>

    <!-- 搜索语法提示 -->
    <n-collapse-transition :show="!hasSearched && !query.trim()">
      <div class="syntax-hints">
        <div class="hint-item">
          <n-tag size="tiny" type="info" :bordered="false">@域名</n-tag>
          <span>按域过滤，如 <code>@myproject</code></span>
        </div>
        <div class="hint-item">
          <n-tag size="tiny" type="success" :bordered="false">#标签</n-tag>
          <span>按标签过滤，如 <code>#规范 #偏好</code></span>
        </div>
        <div class="hint-item">
          <n-tag size="tiny" type="default" :bordered="false">关键词</n-tag>
          <span>全文搜索，300ms 实时响应</span>
        </div>
      </div>
    </n-collapse-transition>

    <!-- 搜索结果 -->
    <div class="search-results" role="region" aria-live="polite" aria-label="搜索结果">
      <!-- 加载状态 -->
      <div v-if="loading" class="loading-state" aria-busy="true">
        <n-spin size="medium" />
        <span class="loading-text">搜索中...</span>
      </div>

      <!-- 空结果 -->
      <div v-else-if="isEmpty" class="empty-state" role="status">
        <div class="empty-icon-container">
          <div class="i-carbon-search-locate" aria-hidden="true" />
        </div>
        <div class="empty-text">未找到匹配的记忆</div>
        <div class="empty-hint">尝试调整关键词或分类筛选</div>
      </div>

      <!-- 结果列表 -->
      <template v-else-if="hasResults">
        <div class="result-header">
          <span class="result-count" role="status">
            找到 <strong>{{ results.length }}</strong> 条结果
          </span>
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
            <!-- 左侧色条 -->
            <div
              class="result-accent"
              :style="{ background: getCategoryAccentColor(item.category) }"
            />

            <div class="result-body">
              <!-- 头部：分类 + 相关度 -->
              <div class="result-meta">
                <div class="result-category">
                  <span :class="['category-badge', getCategoryBgClass(item.category)]">
                    <div :class="getCategoryIcon(item.category)" aria-hidden="true" />
                    {{ item.category }}
                  </span>
                  <n-tag v-if="item.domain" size="tiny" :bordered="false" type="info" class="domain-tag">
                    {{ item.domain }}
                  </n-tag>
                </div>
                <n-tag :type="getRelevanceType(item.relevance)" size="small" :bordered="false" class="relevance-tag">
                  {{ formatRelevance(item.relevance) }}
                </n-tag>
              </div>

              <!-- 高亮内容 -->
              <div class="result-highlight">
                {{ item.highlight }}
              </div>

              <!-- 标签 -->
              <div v-if="item.tags && item.tags.length > 0" class="result-tags">
                <n-tag
                  v-for="tag in item.tags"
                  :key="tag"
                  size="tiny"
                  :bordered="false"
                  round
                >
                  {{ tag }}
                </n-tag>
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
        </div>
      </template>

      <!-- 初始状态 -->
      <div v-else-if="!query.trim()" class="initial-state">
        <div class="empty-icon-container empty-icon-container--large">
          <div class="i-carbon-search" aria-hidden="true" />
        </div>
        <div class="empty-text">输入关键词开始搜索</div>
        <div class="empty-hint">支持全文检索和 @域名 / #标签 精确筛选</div>
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

/* 搜索栏容器 */
.search-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
  border-radius: 12px;
  background: var(--color-container, rgba(255, 255, 255, 0.4));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
  transition: border-color 0.2s ease;
}

.search-container:focus-within {
  border-color: rgba(20, 184, 166, 0.3);
}

:root.dark .search-container {
  background: rgba(24, 24, 30, 0.3);
  border-color: rgba(255, 255, 255, 0.04);
}

:root.dark .search-container:focus-within {
  border-color: rgba(20, 184, 166, 0.35);
}

.search-bar {
  display: flex;
  gap: 10px;
  align-items: center;
}

.search-input {
  flex: 1;
}

.search-icon {
  color: rgba(20, 184, 166, 0.5);
}

.category-select {
  width: 120px;
}

.search-btn {
  flex-shrink: 0;
}

/* 搜索前缀解析提示 */
.parsed-info {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(20, 184, 166, 0.04);
  border: 1px solid rgba(20, 184, 166, 0.08);
}

.parsed-info-icon {
  color: rgba(20, 184, 166, 0.6);
  font-size: 13px;
  flex-shrink: 0;
}

:root.dark .parsed-info {
  background: rgba(20, 184, 166, 0.08);
  border-color: rgba(20, 184, 166, 0.12);
  color: #9ca3af;
}

/* 语法提示 */
.syntax-hints {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  padding: 10px 0;
}

.hint-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
}

:root.dark .hint-item {
  color: #9ca3af;
}

.hint-item code {
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--color-border, rgba(128, 128, 128, 0.1));
  font-family: 'Fira Code', 'Consolas', monospace;
}

/* 搜索结果 */
.search-results {
  min-height: 200px;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  gap: 12px;
  color: var(--color-on-surface-muted, #9ca3af);
}

.loading-text {
  font-size: 13px;
  color: var(--color-on-surface-secondary, #9ca3af);
}

/* 空状态和初始状态 */
.empty-state,
.initial-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  gap: 6px;
}

.empty-icon-container {
  width: 48px;
  height: 48px;
  border-radius: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.06), rgba(59, 130, 246, 0.04));
  border: 1px dashed rgba(20, 184, 166, 0.2);
  margin-bottom: 4px;
}

.empty-icon-container--large {
  width: 56px;
  height: 56px;
  border-radius: 16px;
}

:root.dark .empty-icon-container {
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.1), rgba(59, 130, 246, 0.08));
  border-color: rgba(20, 184, 166, 0.25);
}

.empty-icon-container [class^="i-carbon-"] {
  font-size: 22px;
  color: rgba(20, 184, 166, 0.4);
}

.empty-icon-container--large [class^="i-carbon-"] {
  font-size: 26px;
}

.empty-text {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-on-surface-secondary, #6b7280);
  opacity: 0.7;
}

:root.dark .empty-text {
  color: #9ca3af;
}

.empty-hint {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
}

/* 结果头部 */
.result-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.result-count {
  font-size: 13px;
  color: var(--color-on-surface-secondary, #6b7280);
}

.result-count strong {
  color: rgba(20, 184, 166, 0.9);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.result-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 结果卡片 - 带左侧色条 */
.result-item {
  display: flex;
  border-radius: 12px;
  overflow: hidden;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.1));
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.result-item:hover {
  border-color: rgba(20, 184, 166, 0.25);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04);
  transform: translateY(-1px);
}

:root.dark .result-item {
  background: rgba(28, 28, 34, 0.5);
  border-color: rgba(255, 255, 255, 0.05);
}

:root.dark .result-item:hover {
  border-color: rgba(20, 184, 166, 0.3);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.result-accent {
  width: 3px;
  flex-shrink: 0;
}

.result-body {
  flex: 1;
  padding: 14px 18px;
  min-width: 0;
}

.result-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.result-category {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 分类标签 */
.category-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 600;
}

.category-badge--rule {
  background: rgba(59, 130, 246, 0.08);
  color: #3b82f6;
}

.category-badge--preference {
  background: rgba(168, 85, 247, 0.08);
  color: #a855f7;
}

.category-badge--pattern {
  background: rgba(34, 197, 94, 0.08);
  color: #22c55e;
}

.category-badge--context {
  background: rgba(249, 115, 22, 0.08);
  color: #f97316;
}

:root.dark .category-badge--rule {
  background: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
}

:root.dark .category-badge--preference {
  background: rgba(168, 85, 247, 0.15);
  color: #c084fc;
}

:root.dark .category-badge--pattern {
  background: rgba(34, 197, 94, 0.15);
  color: #4ade80;
}

:root.dark .category-badge--context {
  background: rgba(249, 115, 22, 0.15);
  color: #fb923c;
}

.domain-tag {
  font-size: 10px;
}

.relevance-tag {
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

.result-highlight {
  font-size: 13px;
  line-height: 1.7;
  color: var(--color-on-surface, #111827);
  word-break: break-word;
}

:root.dark .result-highlight {
  color: #e5e7eb;
}

.result-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 8px;
}

.result-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.06));
}

.result-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  font-variant-numeric: tabular-nums;
}

.result-actions {
  display: flex;
  gap: 10px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.result-item:hover .result-actions {
  opacity: 1;
}

.result-item:focus {
  outline: 2px solid rgba(20, 184, 166, 0.5);
  outline-offset: 2px;
}
</style>
