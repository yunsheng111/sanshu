<script setup lang="ts">
/**
 * 记忆管理配置组件
 * 包含：配置设置、记忆列表、搜索、相似度预览、导出功能
 *
 * Step 18: 记忆管理 UI — 前端组件增强
 * - 集成 MemorySearch 组件
 * - 集成 MemoryList 组件
 * - 添加导出功能
 */
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
import ConfigSection from '../common/ConfigSection.vue'
import MemorySearch from './MemorySearch.vue'
import MemoryList from './MemoryList.vue'

// Props
const props = defineProps<{
  active: boolean
  projectRootPath?: string | null
}>()

const message = useMessage()

// ============ 类型定义 ============
interface MemoryEntry {
  id: string
  content: string
  category: string
  created_at: string
}

interface MemoryConfig {
  similarity_threshold: number
  dedup_on_startup: boolean
  enable_dedup: boolean
}

interface MemoryStats {
  total: number
  rules: number
  preferences: number
  patterns: number
  contexts: number
}

interface DedupResult {
  original_count: number
  removed_count: number
  remaining_count: number
  removed_ids: string[]
}

interface SimilarityPreview {
  is_duplicate: boolean
  similarity: number
  matched_id: string | null
  matched_content: string | null
  threshold: number
}

interface ExportMemories {
  version: string
  exported_at: string
  project_path: string
  total_count: number
  entries: MemoryEntry[]
}

interface SearchResult {
  id: string
  content: string
  category: string
  created_at: string
  relevance: number
  highlight: string
}

// ============ 状态 ============
const currentTab = ref('config')
const loading = ref(false)
const projectPath = computed(() => props.projectRootPath || '')

// 组件引用
const memoryListRef = ref<InstanceType<typeof MemoryList> | null>(null)
const memorySearchRef = ref<InstanceType<typeof MemorySearch> | null>(null)

// 配置状态
const config = ref<MemoryConfig>({
  similarity_threshold: 0.70,
  dedup_on_startup: true,
  enable_dedup: true,
})
const configLoading = ref(false)
const configSaving = ref(false)

// 记忆列表状态
const memories = ref<MemoryEntry[]>([])
const stats = ref<MemoryStats>({ total: 0, rules: 0, preferences: 0, patterns: 0, contexts: 0 })
const listLoading = ref(false)
const expandedCategories = ref<string[]>(['规范', '偏好', '模式', '背景'])

// 去重状态
const dedupLoading = ref(false)
const lastDedupResult = ref<DedupResult | null>(null)

// 导出状态
const exportLoading = ref(false)

// 编辑模态框状态
const showEditModal = ref(false)
const editingMemory = ref<SearchResult | null>(null)
const editContent = ref('')
const editSaving = ref(false)

// 相似度预览状态
const previewContent = ref('')
const previewLoading = ref(false)
const previewResult = ref<SimilarityPreview | null>(null)

// 删除确认状态
const deleteConfirmId = ref<string | null>(null)
const deleteLoading = ref(false)

// ============ 计算属性 ============
const groupedMemories = computed(() => {
  const groups: Record<string, MemoryEntry[]> = {
    '规范': [],
    '偏好': [],
    '模式': [],
    '背景': [],
  }
  for (const m of memories.value) {
    if (groups[m.category]) {
      groups[m.category].push(m)
    }
  }
  return groups
})

const thresholdPercent = computed({
  get: () => Math.round(config.value.similarity_threshold * 100),
  set: (val: number) => {
    config.value.similarity_threshold = val / 100
  },
})

// ============ 加载函数 ============
async function loadConfig() {
  if (!projectPath.value) return
  configLoading.value = true
  try {
    const res = await invoke<MemoryConfig>('get_memory_config', { projectPath: projectPath.value })
    config.value = res
  }
  catch (err) {
    message.error(`加载配置失败: ${err}`)
  }
  finally {
    configLoading.value = false
  }
}

async function loadMemories() {
  if (!projectPath.value) return
  listLoading.value = true
  try {
    const [memoryList, memoryStats] = await Promise.all([
      invoke<MemoryEntry[]>('get_memory_list', { projectPath: projectPath.value }),
      invoke<MemoryStats>('get_memory_stats', { projectPath: projectPath.value }),
    ])
    memories.value = memoryList
    stats.value = memoryStats
  }
  catch (err) {
    message.error(`加载记忆列表失败: ${err}`)
  }
  finally {
    listLoading.value = false
  }
}

// ============ 操作函数 ============
async function saveConfig() {
  if (!projectPath.value) return
  configSaving.value = true
  try {
    await invoke('save_memory_config', {
      projectPath: projectPath.value,
      config: config.value,
    })
    message.success('配置已保存')
  }
  catch (err) {
    message.error(`保存配置失败: ${err}`)
  }
  finally {
    configSaving.value = false
  }
}

async function executeDeduplicate() {
  if (!projectPath.value) return
  dedupLoading.value = true
  try {
    const result = await invoke<DedupResult>('deduplicate_memories', { projectPath: projectPath.value })
    lastDedupResult.value = result
    if (result.removed_count > 0) {
      message.success(`去重完成：移除 ${result.removed_count} 条重复记忆`)
      await loadMemories()
    }
    else {
      message.info('未发现重复记忆')
    }
  }
  catch (err) {
    message.error(`去重失败: ${err}`)
  }
  finally {
    dedupLoading.value = false
  }
}

async function previewSimilarity() {
  if (!projectPath.value || !previewContent.value.trim()) {
    message.warning('请输入要检测的内容')
    return
  }
  previewLoading.value = true
  try {
    const result = await invoke<SimilarityPreview>('preview_similarity', {
      projectPath: projectPath.value,
      content: previewContent.value,
    })
    previewResult.value = result
  }
  catch (err) {
    message.error(`预览失败: ${err}`)
  }
  finally {
    previewLoading.value = false
  }
}

async function deleteMemory(id: string) {
  if (!projectPath.value) return
  deleteLoading.value = true
  try {
    await invoke('delete_memory', { projectPath: projectPath.value, memoryId: id })
    message.success('记忆已删除')
    deleteConfirmId.value = null
    await loadMemories()
  }
  catch (err) {
    message.error(`删除失败: ${err}`)
  }
  finally {
    deleteLoading.value = false
  }
}

// ============ 导出功能 ============
async function exportMemories() {
  if (!projectPath.value) return
  exportLoading.value = true
  try {
    const data = await invoke<ExportMemories>('export_memories', {
      projectPath: projectPath.value,
    })

    // 生成 JSON 文件内容
    const jsonStr = JSON.stringify(data, null, 2)
    const blob = new Blob([jsonStr], { type: 'application/json' })
    const url = URL.createObjectURL(blob)

    // 创建下载链接
    const a = document.createElement('a')
    a.href = url
    a.download = `sanshu-memories-${new Date().toISOString().slice(0, 10)}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)

    message.success(`已导出 ${data.total_count} 条记忆`)
  }
  catch (err) {
    message.error(`导出失败: ${err}`)
  }
  finally {
    exportLoading.value = false
  }
}

// ============ 编辑功能 ============
function handleEdit(memory: SearchResult) {
  editingMemory.value = memory
  editContent.value = memory.content
  showEditModal.value = true
}

async function saveEdit() {
  if (!editingMemory.value || !editContent.value.trim()) {
    message.warning('内容不能为空')
    return
  }

  editSaving.value = true
  try {
    await invoke('update_memory', {
      projectPath: projectPath.value,
      update: {
        memory_id: editingMemory.value.id,
        content: editContent.value.trim(),
        append: false,
      },
    })

    message.success('记忆已更新')
    closeEditModal()
    await loadMemories()
    memoryListRef.value?.refresh()
  }
  catch (err) {
    message.error(`更新失败: ${err}`)
  }
  finally {
    editSaving.value = false
  }
}

function closeEditModal() {
  showEditModal.value = false
  editingMemory.value = null
  editContent.value = ''
}

// ============ 搜索结果处理 ============
async function handleSearchDelete(memoryId: string) {
  await deleteMemory(memoryId)
  memorySearchRef.value?.search()
}

function handleStatsUpdate(newStats: MemoryStats) {
  stats.value = newStats
}

function formatDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString('zh-CN')
  }
  catch {
    return isoString
  }
}

function getCategoryIcon(category: string): string {
  const icons: Record<string, string> = {
    '规范': 'i-carbon-rule',
    '偏好': 'i-carbon-user-favorite',
    '模式': 'i-carbon-flow',
    '背景': 'i-carbon-document',
  }
  return icons[category] || 'i-carbon-document'
}

function getCategoryColor(category: string): string {
  const colors: Record<string, string> = {
    '规范': 'text-blue-500',
    '偏好': 'text-purple-500',
    '模式': 'text-green-500',
    '背景': 'text-orange-500',
  }
  return colors[category] || 'text-gray-500'
}

// ============ 生命周期 ============
watch(() => props.active, async (active) => {
  if (active && projectPath.value) {
    loading.value = true
    await Promise.all([loadConfig(), loadMemories()])
    loading.value = false
  }
})

onMounted(async () => {
  if (props.active && projectPath.value) {
    loading.value = true
    await Promise.all([loadConfig(), loadMemories()])
    loading.value = false
  }
})
</script>

<template>
  <div class="memory-config">
    <!-- 无项目路径提示 -->
    <div v-if="!projectPath" class="empty-state">
      <div class="i-carbon-folder-off text-5xl mb-3 opacity-20" />
      <div class="text-sm opacity-60">
        请先在 MCP 工具中指定项目路径
      </div>
    </div>

    <template v-else>
      <n-tabs v-model:value="currentTab" type="line" animated>
        <!-- 配置 Tab -->
        <n-tab-pane name="config" tab="配置">
          <n-scrollbar class="tab-scrollbar">
            <n-space vertical size="large" class="tab-content">
              <!-- 去重设置 -->
              <ConfigSection title="去重设置" description="配置相似度检测阈值和自动去重行为">
                <n-space vertical size="medium">
                  <!-- 相似度阈值滑块 -->
                  <n-form-item label="相似度阈值">
                    <div class="w-full">
                      <div class="flex items-center gap-4">
                        <n-slider
                          v-model:value="thresholdPercent"
                          :min="50"
                          :max="95"
                          :step="5"
                          :marks="{ 50: '50%', 70: '70%', 95: '95%' }"
                          class="flex-1"
                        />
                        <n-tag type="info" :bordered="false">
                          {{ thresholdPercent }}%
                        </n-tag>
                      </div>
                      <div class="text-xs text-gray-500 mt-2">
                        超过此相似度的内容将被视为重复。建议值：70%
                      </div>
                    </div>
                  </n-form-item>

                  <!-- 开关选项 -->
                  <div class="switch-group">
                    <div class="switch-item">
                      <div class="switch-info">
                        <div class="switch-label">启动时自动去重</div>
                        <div class="switch-desc">每次加载记忆时自动检测并移除重复内容</div>
                      </div>
                      <n-switch v-model:value="config.dedup_on_startup" />
                    </div>
                    <div class="switch-item">
                      <div class="switch-info">
                        <div class="switch-label">启用去重检测</div>
                        <div class="switch-desc">添加新记忆时检测是否与现有内容重复</div>
                      </div>
                      <n-switch v-model:value="config.enable_dedup" />
                    </div>
                  </div>
                </n-space>
              </ConfigSection>

              <!-- 快捷操作 -->
              <ConfigSection title="快捷操作" :no-card="true">
                <n-space>
                  <n-button type="primary" :loading="configSaving" @click="saveConfig">
                    <template #icon>
                      <div class="i-carbon-save" />
                    </template>
                    保存配置
                  </n-button>
                  <n-button secondary :loading="dedupLoading" @click="executeDeduplicate">
                    <template #icon>
                      <div class="i-carbon-clean" />
                    </template>
                    立即整理
                  </n-button>
                  <n-button secondary :loading="exportLoading" @click="exportMemories">
                    <template #icon>
                      <div class="i-carbon-export" />
                    </template>
                    导出记忆
                  </n-button>
                </n-space>
              </ConfigSection>

              <!-- 统计信息 -->
              <ConfigSection title="统计信息" :no-card="true">
                <div class="stats-grid">
                  <div class="stat-card">
                    <div class="stat-value">{{ stats.total }}</div>
                    <div class="stat-label">总计</div>
                  </div>
                  <div class="stat-card">
                    <div class="stat-value text-blue-500">{{ stats.rules }}</div>
                    <div class="stat-label">规范</div>
                  </div>
                  <div class="stat-card">
                    <div class="stat-value text-purple-500">{{ stats.preferences }}</div>
                    <div class="stat-label">偏好</div>
                  </div>
                  <div class="stat-card">
                    <div class="stat-value text-green-500">{{ stats.patterns }}</div>
                    <div class="stat-label">模式</div>
                  </div>
                  <div class="stat-card">
                    <div class="stat-value text-orange-500">{{ stats.contexts }}</div>
                    <div class="stat-label">背景</div>
                  </div>
                </div>
              </ConfigSection>

              <!-- 去重结果 -->
              <n-collapse-transition :show="lastDedupResult !== null">
                <ConfigSection v-if="lastDedupResult" title="上次整理结果" :no-card="true">
                  <n-alert type="success" :bordered="false">
                    <template #icon>
                      <div class="i-carbon-checkmark-outline" />
                    </template>
                    移除 <strong>{{ lastDedupResult.removed_count }}</strong> 条重复记忆，
                    保留 <strong>{{ lastDedupResult.remaining_count }}</strong> 条
                  </n-alert>
                </ConfigSection>
              </n-collapse-transition>
            </n-space>
          </n-scrollbar>
        </n-tab-pane>

        <!-- 记忆列表 Tab -->
        <n-tab-pane name="list" tab="记忆列表">
          <n-scrollbar class="tab-scrollbar">
            <MemoryList
              ref="memoryListRef"
              :project-root-path="projectPath"
              :active="currentTab === 'list'"
              @refresh="loadMemories"
              @stats-updated="handleStatsUpdate"
            />
          </n-scrollbar>
        </n-tab-pane>

        <!-- 搜索 Tab -->
        <n-tab-pane name="search" tab="搜索">
          <n-scrollbar class="tab-scrollbar">
            <div class="tab-content">
              <MemorySearch
                ref="memorySearchRef"
                :project-root-path="projectPath"
                @edit="handleEdit"
                @delete="handleSearchDelete"
              />
            </div>
          </n-scrollbar>
        </n-tab-pane>

        <!-- 相似度预览 Tab -->
        <n-tab-pane name="preview" tab="相似度预览">
          <n-scrollbar class="tab-scrollbar">
            <n-space vertical size="large" class="tab-content">
              <ConfigSection title="输入检测" description="输入内容检测与现有记忆的相似度">
                <n-space vertical size="medium">
                  <n-input
                    v-model:value="previewContent"
                    type="textarea"
                    :rows="3"
                    placeholder="输入要检测的内容..."
                    aria-label="待检测内容输入框"
                  />
                  <n-button
                    type="primary"
                    :loading="previewLoading"
                    :disabled="!previewContent.trim()"
                    aria-describedby="similarity-result"
                    @click="previewSimilarity"
                  >
                    <template #icon>
                      <div class="i-carbon-search" />
                    </template>
                    检测相似度
                  </n-button>
                </n-space>
              </ConfigSection>

              <!-- 检测结果 -->
              <n-collapse-transition :show="previewResult !== null">
                <ConfigSection v-if="previewResult" title="检测结果" :no-card="true">
                  <div class="preview-result">
                    <!-- 相似度指示器 -->
                    <div class="similarity-indicator">
                      <div
                        class="similarity-bar"
                        :style="{ width: `${previewResult.similarity * 100}%` }"
                        :class="{
                          'bg-red-500': previewResult.is_duplicate,
                          'bg-green-500': !previewResult.is_duplicate,
                        }"
                      />
                      <div class="similarity-text">
                        相似度: {{ (previewResult.similarity * 100).toFixed(1) }}%
                        <span class="threshold-text">
                          (阈值: {{ (previewResult.threshold * 100).toFixed(0) }}%)
                        </span>
                      </div>
                    </div>

                    <!-- 结果状态 -->
                    <n-alert
                      :type="previewResult.is_duplicate ? 'warning' : 'success'"
                      :bordered="false"
                      class="mt-4"
                    >
                      <template #icon>
                        <div :class="previewResult.is_duplicate ? 'i-carbon-warning' : 'i-carbon-checkmark'" />
                      </template>
                      {{ previewResult.is_duplicate ? '检测到相似内容，添加时将被拒绝' : '未检测到相似内容，可以添加' }}
                    </n-alert>

                    <!-- 匹配的内容 -->
                    <div v-if="previewResult.matched_content" class="matched-content mt-4">
                      <div class="matched-label">最相似的记忆:</div>
                      <div class="matched-text">{{ previewResult.matched_content }}</div>
                    </div>
                  </div>
                </ConfigSection>
              </n-collapse-transition>
            </n-space>
          </n-scrollbar>
        </n-tab-pane>
      </n-tabs>
    </template>

    <!-- 编辑模态框 -->
    <n-modal
      v-model:show="showEditModal"
      preset="card"
      title="编辑记忆"
      :style="{ width: '600px' }"
      :mask-closable="false"
    >
      <n-space vertical size="medium">
        <div v-if="editingMemory" class="edit-info">
          <div class="edit-category">
            <div :class="[getCategoryIcon(editingMemory.category), getCategoryColor(editingMemory.category)]" />
            <span>{{ editingMemory.category }}</span>
          </div>
          <span class="edit-time">{{ formatDate(editingMemory.created_at) }}</span>
        </div>

        <n-input
          v-model:value="editContent"
          type="textarea"
          :rows="6"
          placeholder="输入新内容..."
          :disabled="editSaving"
        />
      </n-space>

      <template #footer>
        <n-space justify="end">
          <n-button @click="closeEditModal" :disabled="editSaving">
            取消
          </n-button>
          <n-button type="primary" :loading="editSaving" @click="saveEdit">
            保存
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.memory-config {
  min-height: 400px;
}

.tab-scrollbar {
  max-height: 500px;
}

.tab-content {
  padding: 16px 4px;
}

/* 空状态 */
.empty-state,
.empty-list {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  color: var(--color-on-surface-muted, #9ca3af);
}

/* 开关组 */
.switch-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .switch-item {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.switch-info {
  flex: 1;
}

.switch-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-on-surface, #111827);
}

:root.dark .switch-label {
  color: #e5e7eb;
}

.switch-desc {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-top: 2px;
}

:root.dark .switch-desc {
  color: #9ca3af;
}

/* 统计网格 */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 12px;
}

.stat-card {
  text-align: center;
  padding: 12px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .stat-card {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.stat-value {
  font-size: 24px;
  font-weight: 600;
  color: var(--color-on-surface, #111827);
}

:root.dark .stat-value {
  color: #e5e7eb;
}

.stat-label {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-top: 4px;
}

/* 分类头部 */
.category-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 500;
}

/* 记忆列表 */
.memory-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.memory-item {
  padding: 12px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .memory-item {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.memory-content {
  font-size: 13px;
  line-height: 1.5;
  color: var(--color-on-surface, #111827);
  word-break: break-word;
}

:root.dark .memory-content {
  color: #e5e7eb;
}

.memory-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.1));
}

.memory-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
}

/* 骨架屏 */
.skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 相似度预览结果 */
.preview-result {
  padding: 16px;
  border-radius: 8px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .preview-result {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.similarity-indicator {
  position: relative;
  height: 24px;
  border-radius: 12px;
  background: var(--color-border, rgba(128, 128, 128, 0.2));
  overflow: hidden;
}

.similarity-bar {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  transition: width 0.3s ease;
}

.similarity-text {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 12px;
  font-weight: 500;
  color: white;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.threshold-text {
  opacity: 0.8;
}

.matched-content {
  padding: 12px;
  border-radius: 8px;
  background: var(--color-border, rgba(128, 128, 128, 0.1));
}

.matched-label {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-bottom: 4px;
}

.matched-text {
  font-size: 13px;
  color: var(--color-on-surface, #111827);
}

:root.dark .matched-text {
  color: #e5e7eb;
}

/* 编辑模态框 */
.edit-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-radius: 6px;
  background: var(--color-container, rgba(255, 255, 255, 0.5));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .edit-info {
  background: rgba(24, 24, 28, 0.5);
  border-color: rgba(255, 255, 255, 0.08);
}

.edit-category {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
}

.edit-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
}
</style>
