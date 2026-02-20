<script setup lang="ts">
/**
 * 记忆工作区组件
 * 包含 SearchBar + TagFilter + 记忆卡片列表 + VitalityBadge
 * 渐进式披露三态：collapsed / expanded / detail
 * 记忆数 > 100 条时自动启用虚拟滚动
 */
import { useSafeInvoke } from '../../composables/useSafeInvoke'
import { useProgressiveDisclosure, type DisclosureState } from '../../composables/useProgressiveDisclosure'
import type { VitalityData } from '../../composables/useVitalityDecay'
import { useMessage } from 'naive-ui'
import { computed, inject, onMounted, ref, watch } from 'vue'
import TagFilter from './TagFilter.vue'
import VitalityBadge from './VitalityBadge.vue'
import BatchActionBar from './BatchActionBar.vue'
import SnapshotDiff from './SnapshotDiff.vue'
import { MEMORY_DOMAIN_KEY, MEMORY_BATCH_MODE_KEY, MEMORY_SELECTED_IDS_KEY } from './memoryKeys'

// Props
const props = defineProps<{
  projectRootPath: string
  active?: boolean
}>()

// Emits
const emit = defineEmits<{
  (e: 'search', keyword: string): void
}>()

const message = useMessage()
const { safeInvoke, loading } = useSafeInvoke()
const disclosure = useProgressiveDisclosure()

// ============ inject 共享状态 ============

const selectedDomain = inject(MEMORY_DOMAIN_KEY, ref(null))
const batchMode = inject(MEMORY_BATCH_MODE_KEY, ref(false))
const selectedMemoryIds = inject(MEMORY_SELECTED_IDS_KEY, ref<string[]>([]))

// ============ 类型定义 ============

interface MemoryEntry {
  id: string
  content: string
  category: string
  created_at: string
  tags?: string[]
  domain?: string
  vitality?: VitalityData
}

// ============ 状态 ============

/** 记忆列表 */
const memories = ref<MemoryEntry[]>([])

/** 搜索关键词 */
const searchQuery = ref('')

/** 选中的标签 */
const selectedTags = ref<string[]>([])

/** 是否首次加载 */
const initialLoading = ref(true)

/** 当前查看详情的记忆 ID（用于嵌入 SnapshotDiff） */
const detailMemoryId = ref<string | null>(null)

/** 版本历史 Modal 状态 */
const historyModalVisible = ref(false)
const historyMemoryId = ref<string | null>(null)
const historyMemory = ref<MemoryEntry | null>(null)

/** 虚拟滚动阈值 */
const VIRTUAL_SCROLL_THRESHOLD = 100

// ============ 计算属性 ============

/** 所有可用标签 */
const availableTags = computed(() => {
  const tagSet = new Set<string>()
  for (const m of memories.value) {
    if (m.tags) {
      for (const t of m.tags) {
        tagSet.add(t)
      }
    }
    // 将分类也作为标签
    tagSet.add(m.category)
  }
  return Array.from(tagSet).sort()
})

/** 根据域、搜索、标签筛选后的记忆列表 */
const filteredMemories = computed(() => {
  let list = memories.value

  // 按域筛选
  if (selectedDomain.value) {
    list = list.filter(m => m.domain === selectedDomain.value || !m.domain)
  }

  // 按搜索关键词筛选
  if (searchQuery.value.trim()) {
    const keyword = searchQuery.value.trim().toLowerCase()
    list = list.filter(m =>
      m.content.toLowerCase().includes(keyword) ||
      m.category.toLowerCase().includes(keyword) ||
      (m.tags && m.tags.some(t => t.toLowerCase().includes(keyword)))
    )
  }

  // 按标签筛选
  if (selectedTags.value.length > 0) {
    list = list.filter(m => {
      const memoryTags = [...(m.tags || []), m.category]
      return selectedTags.value.some(t => memoryTags.includes(t))
    })
  }

  return list
})

/** 是否启用虚拟滚动 */
const useVirtualScroll = computed(() => filteredMemories.value.length > VIRTUAL_SCROLL_THRESHOLD)

/** 是否为空 */
const isEmpty = computed(() => memories.value.length === 0 && !loading.value)
const isFilteredEmpty = computed(() => filteredMemories.value.length === 0 && memories.value.length > 0)

// ============ 数据加载 ============

async function loadMemories() {
  const result = await safeInvoke<MemoryEntry[]>('get_memory_list', {
    projectPath: props.projectRootPath,
    domain: selectedDomain.value,
  })

  if (result) {
    memories.value = result
  }
  initialLoading.value = false
}

// ============ 事件处理 ============

/** 搜索框回车 */
function handleSearch() {
  if (searchQuery.value.trim()) {
    emit('search', searchQuery.value.trim())
  }
}

/** 标签筛选变化 */
function handleTagChange(tags: string[]) {
  selectedTags.value = tags
}

/** 切换记忆展开状态（键盘 Space） */
function handleToggle(id: string) {
  disclosure.toggleNext(id)
  // 如果进入 detail 态，记录当前查看的记忆
  if (disclosure.getState(id) === 'detail') {
    detailMemoryId.value = id
  } else {
    if (detailMemoryId.value === id) {
      detailMemoryId.value = null
    }
  }
}

/** 切换多选 */
function toggleSelect(id: string) {
  const ids = [...selectedMemoryIds.value]
  const index = ids.indexOf(id)
  if (index >= 0) {
    ids.splice(index, 1)
  } else {
    ids.push(id)
  }
  selectedMemoryIds.value = ids
}

/** 切换批量模式 */
function toggleBatchMode() {
  batchMode.value = !batchMode.value
  if (!batchMode.value) {
    selectedMemoryIds.value = []
  }
}

/** 批量操作完成后刷新 */
async function handleBatchComplete() {
  batchMode.value = false
  selectedMemoryIds.value = []
  await loadMemories()
}

/** 截取内容预览（100字） */
function getPreview(content: string, maxLen = 100): string {
  if (content.length <= maxLen) return content
  return content.slice(0, maxLen) + '...'
}

/** 获取分类图标 */
function getCategoryIcon(category: string): string {
  const icons: Record<string, string> = {
    '规范': 'i-carbon-rule',
    '偏好': 'i-carbon-user-favorite',
    '模式': 'i-carbon-flow',
    '背景': 'i-carbon-document',
  }
  return icons[category] || 'i-carbon-document'
}

/** 获取分类背景色类 */
function getCategoryBgClass(category: string): string {
  const classes: Record<string, string> = {
    '规范': 'category-bg--rule',
    '偏好': 'category-bg--preference',
    '模式': 'category-bg--pattern',
    '背景': 'category-bg--context',
  }
  return classes[category] || 'category-bg--default'
}

/** 获取分类颜色 */
function getCategoryColor(category: string): string {
  const colors: Record<string, string> = {
    '规范': 'text-blue-500',
    '偏好': 'text-purple-500',
    '模式': 'text-green-500',
    '背景': 'text-orange-500',
  }
  return colors[category] || 'text-gray-500'
}

/** 格式化日期 */
function formatDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString('zh-CN')
  } catch {
    return isoString
  }
}

/** 打开版本历史 Modal */
function handleViewHistory(memory: MemoryEntry, event?: Event) {
  event?.stopPropagation()
  historyMemoryId.value = memory.id
  historyMemory.value = memory
  historyModalVisible.value = true
}

/** 关闭版本历史 Modal */
function handleCloseHistoryModal() {
  historyModalVisible.value = false
  historyMemoryId.value = null
  historyMemory.value = null
}

/** 回滚成功后刷新当前条目 */
async function handleRollbackSuccess(memoryId: string) {
  const result = await safeInvoke<MemoryEntry>('get_memory_by_id', {
    projectPath: props.projectRootPath,
    memoryId,
  })

  if (result) {
    const index = memories.value.findIndex(m => m.id === memoryId)
    if (index !== -1) {
      memories.value[index] = result
    }
    if (historyMemory.value?.id === memoryId) {
      historyMemory.value = result
    }
    message.success('回滚成功')
  }
  handleCloseHistoryModal()
}

// ============ 生命周期 ============

watch([() => props.active, selectedDomain], ([active]) => {
  if (active && props.projectRootPath) {
    loadMemories()
  }
})

onMounted(() => {
  if (props.active && props.projectRootPath) {
    loadMemories()
  }
})

defineExpose({
  refresh: loadMemories,
})
</script>

<template>
  <div
    class="memory-workspace"
    role="region"
    aria-label="记忆工作区"
  >
    <!-- 搜索区域 - 独立布局 -->
    <div class="search-section">
      <div class="search-bar">
        <n-input
          v-model:value="searchQuery"
          placeholder="搜索记忆..."
          clearable
          class="search-input"
          aria-label="工作区搜索"
          @keyup.enter="handleSearch"
          @clear="searchQuery = ''"
        >
          <template #prefix>
            <div class="i-carbon-search search-icon" aria-hidden="true" />
          </template>
        </n-input>
      </div>
      <div class="search-actions">
        <n-button
          :type="batchMode ? 'primary' : 'default'"
          size="small"
          secondary
          aria-label="切换批量选择模式"
          class="action-btn"
          @click="toggleBatchMode"
        >
          <template #icon>
            <div class="i-carbon-checkbox-checked" aria-hidden="true" />
          </template>
          {{ batchMode ? '退出多选' : '多选' }}
        </n-button>
        <n-button
          text
          type="primary"
          size="small"
          :loading="loading"
          aria-label="刷新列表"
          class="refresh-btn"
          @click="loadMemories"
        >
          <template #icon>
            <div class="i-carbon-renew" aria-hidden="true" />
          </template>
        </n-button>
      </div>
    </div>

    <!-- 标签筛选 -->
    <TagFilter
      v-if="availableTags.length > 0"
      :tags="availableTags"
      @change="handleTagChange"
    />

    <!-- 统计信息 -->
    <div class="workspace-stats" role="status">
      <span class="stats-text">
        <span class="stats-count">{{ filteredMemories.length }}</span> 条记忆
        <template v-if="selectedDomain">
          <span class="stats-divider">|</span>
          <span class="stats-domain">{{ selectedDomain }}</span>
        </template>
      </span>
      <span v-if="batchMode && selectedMemoryIds.length > 0" class="stats-selection">
        已选 {{ selectedMemoryIds.length }} 条
      </span>
    </div>

    <!-- 骨架屏加载状态 -->
    <div v-if="initialLoading" class="skeleton-list" aria-busy="true" aria-label="加载中">
      <div v-for="i in 5" :key="i" class="skeleton-card">
        <div class="skeleton-header">
          <n-skeleton text style="width: 80px; height: 20px" />
          <n-skeleton text style="width: 120px; height: 14px" />
        </div>
        <n-skeleton text :repeat="1" class="mb-2" />
        <n-skeleton text style="width: 60%" />
      </div>
    </div>

    <!-- 空状态 -->
    <div v-else-if="isEmpty" class="empty-state" role="status">
      <div class="empty-state-icon">
        <div class="i-carbon-document" aria-hidden="true" />
      </div>
      <div class="empty-state-title">暂无记忆条目</div>
      <div class="empty-state-desc">通过 AI 对话自动积累项目知识</div>
    </div>

    <!-- 筛选后为空 -->
    <div v-else-if="isFilteredEmpty" class="empty-state" role="status">
      <div class="empty-state-icon empty-state-icon--filter">
        <div class="i-carbon-filter" aria-hidden="true" />
      </div>
      <div class="empty-state-title">未找到匹配的记忆</div>
      <div class="empty-state-desc">尝试调整搜索条件或标签筛选</div>
    </div>

    <!-- 记忆卡片列表 -->
    <template v-else>
      <!-- 虚拟滚动模式 -->
      <n-virtual-list
        v-if="useVirtualScroll"
        :items="filteredMemories"
        :item-size="80"
        class="card-list virtual-list"
        :style="{ maxHeight: 'calc(100vh - 300px)' }"
        role="list"
        aria-label="记忆列表（虚拟滚动）"
      >
        <template #default="{ item: memory }">
          <div
            :key="memory.id"
            class="memory-card"
            :class="[
              getCategoryBgClass(memory.category),
              {
                'card-expanded': disclosure.isState(memory.id, 'expanded'),
                'card-detail': disclosure.isState(memory.id, 'detail'),
                'card-selected': batchMode && selectedMemoryIds.includes(memory.id),
              },
            ]"
            role="listitem"
            :tabindex="0"
            :aria-expanded="disclosure.getState(memory.id) !== 'collapsed'"
            @click="batchMode ? toggleSelect(memory.id) : handleToggle(memory.id)"
            @keyup.space.prevent="batchMode ? toggleSelect(memory.id) : handleToggle(memory.id)"
            @keyup.enter="handleToggle(memory.id)"
          >
            <!-- 分类色条 -->
            <div class="card-accent" :class="`accent--${memory.category}`" aria-hidden="true" />

            <!-- 多选复选框 -->
            <n-checkbox
              v-if="batchMode"
              :checked="selectedMemoryIds.includes(memory.id)"
              class="batch-checkbox"
              aria-label="选择此记忆"
              @update:checked="toggleSelect(memory.id)"
              @click.stop
            />

            <!-- collapsed 态：标题 + 分类 + 徽章 + 时间 -->
            <div class="card-header">
              <div class="card-title-row">
                <div class="card-category" :class="getCategoryBgClass(memory.category)">
                  <div :class="[getCategoryIcon(memory.category), getCategoryColor(memory.category)]" aria-hidden="true" />
                  <span>{{ memory.category }}</span>
                </div>
                <div class="card-meta">
                  <VitalityBadge
                    v-if="memory.vitality"
                    :vitality="memory.vitality"
                    size="small"
                  />
                  <span class="card-time">{{ formatDate(memory.created_at) }}</span>
                </div>
              </div>
              <div class="card-content-preview">
                {{ getPreview(memory.content, 60) }}
              </div>
            </div>

            <!-- expanded 态：+内容预览100字 + 标签 -->
            <template v-if="disclosure.isState(memory.id, 'expanded') || disclosure.isState(memory.id, 'detail')">
              <div class="card-body">
                <div class="card-content">
                  {{ disclosure.isState(memory.id, 'detail') ? memory.content : getPreview(memory.content) }}
                </div>
                <div v-if="memory.tags && memory.tags.length > 0" class="card-tags">
                  <n-tag
                    v-for="tag in memory.tags"
                    :key="tag"
                    size="tiny"
                    :bordered="false"
                    round
                  >
                    {{ tag }}
                  </n-tag>
                </div>
              </div>
            </template>

            <!-- detail 态：+完整内容 + 版本历史 -->
            <template v-if="disclosure.isState(memory.id, 'detail')">
              <div class="card-detail-section">
                <n-divider class="my-2" />
                <div class="detail-actions">
                  <n-button
                    size="small"
                    secondary
                    class="history-btn"
                    aria-label="查看版本历史"
                    @click="handleViewHistory(memory, $event)"
                  >
                    <template #icon>
                      <div class="i-carbon-time" aria-hidden="true" />
                    </template>
                    查看历史
                  </n-button>
                </div>
                <SnapshotDiff
                  :project-root-path="projectRootPath"
                  :memory-id="memory.id"
                />
              </div>
            </template>

            <!-- 展开/收起指示 -->
            <div class="card-toggle" aria-hidden="true">
              <div
                :class="disclosure.isState(memory.id, 'collapsed')
                  ? 'i-carbon-chevron-down'
                  : 'i-carbon-chevron-up'"
                class="toggle-icon"
              />
            </div>
          </div>
        </template>
      </n-virtual-list>

      <!-- 普通模式 -->
      <div
        v-else
        class="card-list"
        role="list"
        aria-label="记忆列表"
      >
        <div
          v-for="memory in filteredMemories"
          :key="memory.id"
          class="memory-card"
          :class="[
            getCategoryBgClass(memory.category),
            {
              'card-expanded': disclosure.isState(memory.id, 'expanded'),
              'card-detail': disclosure.isState(memory.id, 'detail'),
              'card-selected': batchMode && selectedMemoryIds.includes(memory.id),
            },
          ]"
          role="listitem"
          :tabindex="0"
          :aria-expanded="disclosure.getState(memory.id) !== 'collapsed'"
          @click="batchMode ? toggleSelect(memory.id) : handleToggle(memory.id)"
          @keyup.space.prevent="batchMode ? toggleSelect(memory.id) : handleToggle(memory.id)"
          @keyup.enter="handleToggle(memory.id)"
        >
          <!-- 分类色条 -->
          <div class="card-accent" :class="`accent--${memory.category}`" aria-hidden="true" />

          <!-- 多选复选框 -->
          <n-checkbox
            v-if="batchMode"
            :checked="selectedMemoryIds.includes(memory.id)"
            class="batch-checkbox"
            aria-label="选择此记忆"
            @update:checked="toggleSelect(memory.id)"
            @click.stop
          />

          <!-- collapsed 态 -->
          <div class="card-header">
            <div class="card-title-row">
              <div class="card-category" :class="getCategoryBgClass(memory.category)">
                <div :class="[getCategoryIcon(memory.category), getCategoryColor(memory.category)]" aria-hidden="true" />
                <span>{{ memory.category }}</span>
              </div>
              <div class="card-meta">
                <VitalityBadge
                  v-if="memory.vitality"
                  :vitality="memory.vitality"
                  size="small"
                />
                <span class="card-time">{{ formatDate(memory.created_at) }}</span>
              </div>
            </div>
            <div class="card-content-preview">
              {{ getPreview(memory.content, 60) }}
            </div>
          </div>

          <!-- expanded 态 -->
          <template v-if="disclosure.isState(memory.id, 'expanded') || disclosure.isState(memory.id, 'detail')">
            <div class="card-body">
              <div class="card-content">
                {{ disclosure.isState(memory.id, 'detail') ? memory.content : getPreview(memory.content) }}
              </div>
              <div v-if="memory.tags && memory.tags.length > 0" class="card-tags">
                <n-tag
                  v-for="tag in memory.tags"
                  :key="tag"
                  size="tiny"
                  :bordered="false"
                  round
                >
                  {{ tag }}
                </n-tag>
              </div>
            </div>
          </template>

          <!-- detail 态 -->
          <template v-if="disclosure.isState(memory.id, 'detail')">
            <div class="card-detail-section">
              <n-divider class="my-2" />
              <div class="detail-actions">
                <n-button
                  size="small"
                  secondary
                  class="history-btn"
                  aria-label="查看版本历史"
                  @click="handleViewHistory(memory, $event)"
                >
                  <template #icon>
                    <div class="i-carbon-time" aria-hidden="true" />
                  </template>
                  查看历史
                </n-button>
              </div>
              <SnapshotDiff
                :project-root-path="projectRootPath"
                :memory-id="memory.id"
              />
            </div>
          </template>

          <!-- 展开/收起指示 -->
          <div class="card-toggle" aria-hidden="true">
            <div
              :class="disclosure.isState(memory.id, 'collapsed')
                ? 'i-carbon-chevron-down'
                : 'i-carbon-chevron-up'"
              class="toggle-icon"
            />
          </div>
        </div>
      </div>
    </template>

    <!-- 批量操作条 -->
    <BatchActionBar
      v-if="batchMode && selectedMemoryIds.length > 0"
      :project-root-path="projectRootPath"
      :selected-ids="selectedMemoryIds"
      :memories="filteredMemories"
      @complete="handleBatchComplete"
      @cancel="() => { batchMode = false; selectedMemoryIds = [] }"
    />

    <!-- 版本历史 Modal -->
    <n-modal
      v-model:show="historyModalVisible"
      preset="card"
      :title="historyMemory ? `版本历史 - ${historyMemory.category}` : '版本历史'"
      :style="{ width: '600px', maxWidth: '90vw' }"
      :mask-closable="true"
      :close-on-esc="true"
      class="history-modal"
      aria-label="版本历史对话框"
      @close="handleCloseHistoryModal"
    >
      <SnapshotDiff
        v-if="historyMemoryId"
        :project-root-path="projectRootPath"
        :memory-id="historyMemoryId"
        :show-rollback="true"
        @rollback-success="handleRollbackSuccess"
      />
    </n-modal>
  </div>
</template>

<style scoped>
.memory-workspace {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 搜索区域 - 独立布局 */
.search-section {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
  transition: border-color 0.2s;
}

.search-section:focus-within {
  border-color: rgba(20, 184, 166, 0.3);
}

:root.dark .search-section {
  background: rgba(24, 24, 30, 0.4);
  border-color: rgba(255, 255, 255, 0.04);
}

:root.dark .search-section:focus-within {
  border-color: rgba(20, 184, 166, 0.35);
}

.search-bar {
  flex: 1;
}

.search-input {
  width: 100%;
}

.search-icon {
  color: rgba(20, 184, 166, 0.5);
  font-size: 15px;
}

.search-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.action-btn {
  white-space: nowrap;
}

.refresh-btn {
  opacity: 0.5;
  transition: opacity 0.2s;
}

.refresh-btn:hover {
  opacity: 1;
}

/* 统计 */
.workspace-stats {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  padding: 0 4px;
}

:root.dark .workspace-stats {
  color: #9ca3af;
}

.stats-count {
  font-weight: 700;
  color: rgba(20, 184, 166, 0.8);
  font-variant-numeric: tabular-nums;
}

.stats-divider {
  margin: 0 6px;
  opacity: 0.3;
}

.stats-domain {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 11px;
  opacity: 0.7;
}

.stats-selection {
  color: var(--color-primary, #3b82f6);
  font-weight: 600;
}

/* 骨架屏 */
.skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.skeleton-card {
  padding: 18px 20px;
  border-radius: 12px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
}

.skeleton-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

:root.dark .skeleton-card {
  background: rgba(24, 24, 30, 0.4);
  border-color: rgba(255, 255, 255, 0.04);
}

/* 空状态 */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 240px;
  gap: 10px;
}

.empty-state-icon {
  width: 56px;
  height: 56px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.06), rgba(59, 130, 246, 0.04));
  border: 1px dashed rgba(20, 184, 166, 0.18);
  margin-bottom: 4px;
}

.empty-state-icon .i-carbon-document,
.empty-state-icon .i-carbon-filter {
  font-size: 24px;
  color: rgba(20, 184, 166, 0.35);
}

.empty-state-icon--filter {
  background: linear-gradient(135deg, rgba(251, 191, 36, 0.06), rgba(251, 146, 60, 0.04));
  border-color: rgba(251, 191, 36, 0.18);
}

.empty-state-icon--filter .i-carbon-filter {
  color: rgba(251, 191, 36, 0.4);
}

:root.dark .empty-state-icon {
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.1), rgba(59, 130, 246, 0.08));
  border-color: rgba(20, 184, 166, 0.25);
}

.empty-state-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-on-surface, #374151);
  opacity: 0.65;
}

:root.dark .empty-state-title {
  color: #d1d5db;
}

.empty-state-desc {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
}

/* 卡片列表 */
.card-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.virtual-list {
  max-height: calc(100vh - 300px);
}

/* 记忆卡片 - 重设计 */
.memory-card {
  position: relative;
  padding: 16px 20px 16px 24px;
  border-radius: 12px;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
}

.memory-card:hover {
  border-color: rgba(20, 184, 166, 0.25);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04), 0 1px 4px rgba(20, 184, 166, 0.06);
  transform: translateY(-1px);
}

.memory-card:focus-visible {
  outline: 2px solid var(--color-primary, #3b82f6);
  outline-offset: 2px;
}

:root.dark .memory-card {
  background: rgba(24, 24, 30, 0.5);
  border-color: rgba(255, 255, 255, 0.04);
}

:root.dark .memory-card:hover {
  border-color: rgba(20, 184, 166, 0.3);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15), 0 1px 4px rgba(20, 184, 166, 0.08);
}

/* 分类色条 */
.card-accent {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  border-radius: 3px 0 0 3px;
  transition: width 0.2s;
}

.memory-card:hover .card-accent {
  width: 4px;
}

.accent--规范 { background: linear-gradient(180deg, #3b82f6, #60a5fa); }
.accent--偏好 { background: linear-gradient(180deg, #8b5cf6, #a78bfa); }
.accent--模式 { background: linear-gradient(180deg, #10b981, #34d399); }
.accent--背景 { background: linear-gradient(180deg, #f59e0b, #fbbf24); }

/* 展开态 */
.card-expanded {
  border-color: rgba(20, 184, 166, 0.3);
  background: var(--color-container, rgba(255, 255, 255, 0.75));
  box-shadow: 0 2px 12px rgba(20, 184, 166, 0.06);
}

:root.dark .card-expanded {
  background: rgba(26, 26, 32, 0.6);
}

/* 详情态 */
.card-detail {
  border-color: rgba(20, 184, 166, 0.4);
  box-shadow: 0 4px 20px rgba(20, 184, 166, 0.08);
}

:root.dark .card-detail {
  box-shadow: 0 4px 20px rgba(20, 184, 166, 0.12);
}

/* 选中态 */
.card-selected {
  border-color: var(--color-primary, #3b82f6);
  background: rgba(59, 130, 246, 0.03);
  box-shadow: 0 0 0 1px rgba(59, 130, 246, 0.1) inset;
}

:root.dark .card-selected {
  background: rgba(59, 130, 246, 0.06);
  box-shadow: 0 0 0 1px rgba(59, 130, 246, 0.15) inset;
}

/* 卡片头部 */
.card-header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.card-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-category {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px 2px 6px;
  border-radius: 6px;
  letter-spacing: 0.02em;
}

/* 分类背景色 */
.card-category.category-bg--rule {
  background: rgba(59, 130, 246, 0.08);
  color: #3b82f6;
}

.card-category.category-bg--preference {
  background: rgba(139, 92, 246, 0.08);
  color: #8b5cf6;
}

.card-category.category-bg--pattern {
  background: rgba(16, 185, 129, 0.08);
  color: #10b981;
}

.card-category.category-bg--context {
  background: rgba(245, 158, 11, 0.08);
  color: #f59e0b;
}

.card-category.category-bg--default {
  background: rgba(156, 163, 175, 0.08);
  color: #6b7280;
}

:root.dark .card-category.category-bg--rule { background: rgba(59, 130, 246, 0.15); color: #93c5fd; }
:root.dark .card-category.category-bg--preference { background: rgba(139, 92, 246, 0.15); color: #c4b5fd; }
:root.dark .card-category.category-bg--pattern { background: rgba(16, 185, 129, 0.15); color: #6ee7b7; }
:root.dark .card-category.category-bg--context { background: rgba(245, 158, 11, 0.15); color: #fde68a; }

.card-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #94a3b8);
  font-variant-numeric: tabular-nums;
}

.card-content-preview {
  font-size: 13px;
  color: var(--color-on-surface, #1e293b);
  line-height: 1.55;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:root.dark .card-content-preview {
  color: #e2e8f0;
}

/* 卡片主体（expanded 态） */
.card-body {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.06));
}

.card-content {
  font-size: 13px;
  line-height: 1.7;
  color: var(--color-on-surface, #1e293b);
  word-break: break-word;
  white-space: pre-wrap;
}

:root.dark .card-content {
  color: #e2e8f0;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 12px;
}

/* 详情区域 */
.card-detail-section {
  margin-top: 4px;
}

/* 展开/收起指示 */
.card-toggle {
  display: flex;
  justify-content: center;
  padding-top: 8px;
}

.toggle-icon {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #94a3b8);
  transition: transform 0.25s ease, color 0.2s;
}

.memory-card:hover .toggle-icon {
  color: rgba(20, 184, 166, 0.6);
}

/* 多选复选框 */
.batch-checkbox {
  position: absolute;
  top: 16px;
  right: 20px;
}

/* 详情操作区 */
.detail-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.history-btn {
  font-size: 12px;
}

/* 版本历史 Modal */
.history-modal {
  max-height: 80vh;
}

.history-modal :deep(.n-card__content) {
  max-height: calc(80vh - 60px);
  overflow-y: auto;
}
</style>
