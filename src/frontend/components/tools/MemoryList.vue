<script setup lang="ts">
/**
 * 记忆列表组件
 * 支持分页、分类过滤、删除、编辑
 */
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'

// Props
const props = defineProps<{
  projectRootPath: string
  active?: boolean
}>()

// Emits
const emit = defineEmits<{
  (e: 'refresh'): void
  (e: 'stats-updated', stats: MemoryStats): void
}>()

const message = useMessage()

// 类型定义
interface MemoryEntry {
  id: string
  content: string
  category: string
  created_at: string
}

interface MemoryStats {
  total: number
  rules: number
  preferences: number
  patterns: number
  contexts: number
}

interface UpdateMemoryDto {
  memory_id: string
  content: string
  append: boolean
}

// 状态
const memories = ref<MemoryEntry[]>([])
const stats = ref<MemoryStats>({ total: 0, rules: 0, preferences: 0, patterns: 0, contexts: 0 })
const loading = ref(false)
const selectedCategory = ref<string | null>(null)

// 分页
const currentPage = ref(1)
const pageSize = ref(10)

// 编辑状态
const editingId = ref<string | null>(null)
const editContent = ref('')
const editLoading = ref(false)

// 删除状态
const deleteConfirmId = ref<string | null>(null)
const deleteLoading = ref(false)

// 分类选项
const categoryOptions = [
  { label: '全部分类', value: null },
  { label: '规范', value: '规范' },
  { label: '偏好', value: '偏好' },
  { label: '模式', value: '模式' },
  { label: '背景', value: '背景' },
]

// 计算属性
const filteredMemories = computed(() => {
  if (!selectedCategory.value) {
    return memories.value
  }
  return memories.value.filter(m => m.category === selectedCategory.value)
})

const paginatedMemories = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  return filteredMemories.value.slice(start, end)
})

const totalPages = computed(() => Math.ceil(filteredMemories.value.length / pageSize.value))

const isEmpty = computed(() => memories.value.length === 0)
const isFilteredEmpty = computed(() => filteredMemories.value.length === 0 && !isEmpty.value)

// 加载数据
async function loadData() {
  if (!props.projectRootPath)
    return

  loading.value = true
  try {
    const [memoryList, memoryStats] = await Promise.all([
      invoke<MemoryEntry[]>('get_memory_list', { projectPath: props.projectRootPath }),
      invoke<MemoryStats>('get_memory_stats', { projectPath: props.projectRootPath }),
    ])
    memories.value = memoryList
    stats.value = memoryStats
    emit('stats-updated', memoryStats)
  }
  catch (err) {
    message.error(`加载记忆列表失败: ${err}`)
  }
  finally {
    loading.value = false
  }
}

// 删除记忆
async function deleteMemory(id: string) {
  deleteLoading.value = true
  try {
    await invoke('delete_memory', {
      projectPath: props.projectRootPath,
      memoryId: id,
    })
    message.success('记忆已删除')
    deleteConfirmId.value = null
    await loadData()
    emit('refresh')
  }
  catch (err) {
    message.error(`删除失败: ${err}`)
  }
  finally {
    deleteLoading.value = false
  }
}

// 开始编辑
function startEdit(memory: MemoryEntry) {
  editingId.value = memory.id
  editContent.value = memory.content
}

// 取消编辑
function cancelEdit() {
  editingId.value = null
  editContent.value = ''
}

// 保存编辑
async function saveEdit() {
  if (!editingId.value || !editContent.value.trim()) {
    message.warning('内容不能为空')
    return
  }

  editLoading.value = true
  try {
    const update: UpdateMemoryDto = {
      memory_id: editingId.value,
      content: editContent.value.trim(),
      append: false, // 完全替换
    }

    await invoke('update_memory', {
      projectPath: props.projectRootPath,
      update,
    })

    message.success('记忆已更新')
    cancelEdit()
    await loadData()
    emit('refresh')
  }
  catch (err) {
    message.error(`更新失败: ${err}`)
  }
  finally {
    editLoading.value = false
  }
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

// 监听分类变化，重置分页
watch(selectedCategory, () => {
  currentPage.value = 1
})

// 监听 active 变化
watch(() => props.active, (active) => {
  if (active && props.projectRootPath) {
    loadData()
  }
})

// 挂载时加载数据
onMounted(() => {
  if (props.active && props.projectRootPath) {
    loadData()
  }
})

// 暴露刷新方法
defineExpose({
  refresh: loadData,
})
</script>

<template>
  <div class="memory-list" role="region" aria-label="记忆列表">
    <!-- 工具栏 -->
    <div class="toolbar">
      <n-select
        v-model:value="selectedCategory"
        :options="categoryOptions"
        placeholder="筛选分类"
        class="category-filter"
        aria-label="分类筛选"
      />

      <n-button text type="primary" :loading="loading" aria-label="刷新列表" @click="loadData">
        <template #icon>
          <div class="i-carbon-renew" aria-hidden="true" />
        </template>
        刷新
      </n-button>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading && memories.length === 0" class="loading-state" aria-busy="true">
      <n-skeleton v-for="i in 3" :key="i" text :repeat="2" class="mb-4" />
    </div>

    <!-- 空状态 -->
    <div v-else-if="isEmpty" class="empty-state" role="status">
      <div class="i-carbon-document text-4xl mb-2 opacity-20" aria-hidden="true" />
      <div class="text-sm opacity-60">
        暂无记忆条目
      </div>
    </div>

    <!-- 过滤后为空 -->
    <div v-else-if="isFilteredEmpty" class="empty-state" role="status">
      <div class="i-carbon-filter text-4xl mb-2 opacity-20" aria-hidden="true" />
      <div class="text-sm opacity-60">
        当前分类下暂无记忆
      </div>
    </div>

    <!-- 列表 -->
    <template v-else>
      <div class="list-container" role="list" aria-label="记忆条目列表">
        <div v-for="memory in paginatedMemories" :key="memory.id" class="memory-item" role="listitem">
          <!-- 编辑模式 -->
          <template v-if="editingId === memory.id">
            <n-input
              v-model:value="editContent"
              type="textarea"
              :rows="3"
              placeholder="输入新内容..."
              aria-label="编辑记忆内容"
            />
            <div class="edit-actions">
              <n-button size="small" aria-label="取消编辑" @click="cancelEdit">
                取消
              </n-button>
              <n-button type="primary" size="small" :loading="editLoading" aria-label="保存修改" @click="saveEdit">
                保存
              </n-button>
            </div>
          </template>

          <!-- 显示模式 -->
          <template v-else>
            <div class="memory-header">
              <div class="memory-category">
                <div :class="[getCategoryIcon(memory.category), getCategoryColor(memory.category)]" aria-hidden="true" />
                <span>{{ memory.category }}</span>
              </div>
              <span class="memory-time">{{ formatDate(memory.created_at) }}</span>
            </div>

            <div class="memory-content">
              {{ memory.content }}
            </div>

            <div class="memory-actions">
              <n-button text type="primary" size="tiny" aria-label="编辑此记忆" @click="startEdit(memory)">
                <template #icon>
                  <div class="i-carbon-edit" aria-hidden="true" />
                </template>
                编辑
              </n-button>

              <n-popconfirm
                :show="deleteConfirmId === memory.id"
                @positive-click="deleteMemory(memory.id)"
                @negative-click="deleteConfirmId = null"
              >
                <template #trigger>
                  <n-button
                    text
                    type="error"
                    size="tiny"
                    :loading="deleteLoading && deleteConfirmId === memory.id"
                    aria-label="删除此记忆"
                    @click="deleteConfirmId = memory.id"
                  >
                    <template #icon>
                      <div class="i-carbon-trash-can" aria-hidden="true" />
                    </template>
                    删除
                  </n-button>
                </template>
                确定要删除这条记忆吗?
              </n-popconfirm>
            </div>
          </template>
        </div>
      </div>

      <!-- 分页 -->
      <div v-if="totalPages > 1" class="pagination">
        <n-pagination
          v-model:page="currentPage"
          :page-count="totalPages"
          :page-size="pageSize"
          show-size-picker
          :page-sizes="[10, 20, 50]"
          @update:page-size="pageSize = $event"
        />
      </div>
    </template>
  </div>
</template>

<style scoped>
.memory-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 工具栏 */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.category-filter {
  width: 140px;
}

/* 状态 */
.loading-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  color: var(--color-on-surface-muted, #9ca3af);
}

/* 列表容器 */
.list-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 记忆条目 */
.memory-item {
  padding: 14px 16px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .memory-item {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.memory-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.memory-category {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
}

.memory-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
}

.memory-content {
  font-size: 13px;
  line-height: 1.6;
  color: var(--color-on-surface, #111827);
  word-break: break-word;
}

:root.dark .memory-content {
  color: #e5e7eb;
}

.memory-actions {
  display: flex;
  gap: 12px;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.1));
}

/* 编辑状态 */
.edit-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}

/* 分页 */
.pagination {
  display: flex;
  justify-content: center;
  margin-top: 8px;
}
</style>
