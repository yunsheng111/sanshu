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

function getCategoryBgClass(category: string): string {
  const classes: Record<string, string> = {
    '规范': 'category-badge--rule',
    '偏好': 'category-badge--preference',
    '模式': 'category-badge--pattern',
    '背景': 'category-badge--context',
  }
  return classes[category] || ''
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
    <div v-if="!projectPath" class="empty-state" role="status">
      <div class="empty-icon-container">
        <div class="i-carbon-folder-off" aria-hidden="true" />
      </div>
      <div class="empty-text">未指定项目路径</div>
      <div class="empty-hint">请先在 MCP 工具中指定项目路径</div>
    </div>

    <template v-else>
      <n-tabs v-model:value="currentTab" type="line" animated>
        <!-- 配置 Tab -->
        <n-tab-pane name="config" tab="配置">
          <div class="tab-content">
            <div class="config-sections">
              <!-- 去重设置 -->
              <ConfigSection title="去重设置" description="配置相似度检测阈值和自动去重行为">
                <div class="dedup-settings">
                  <!-- 相似度阈值滑块 -->
                  <div class="threshold-control">
                    <div class="threshold-header">
                      <span class="threshold-label">相似度阈值</span>
                      <span class="threshold-value">{{ thresholdPercent }}%</span>
                    </div>
                    <n-slider
                      v-model:value="thresholdPercent"
                      :min="50"
                      :max="95"
                      :step="5"
                      :marks="{ 50: '50%', 70: '70%', 95: '95%' }"
                    />
                    <div class="threshold-hint">
                      超过此相似度的内容将被视为重复。建议值：70%
                    </div>
                  </div>

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
                </div>
              </ConfigSection>

              <!-- 快捷操作 -->
              <ConfigSection title="快捷操作" :no-card="true">
                <div class="quick-actions">
                  <n-button type="primary" :loading="configSaving" class="action-btn" @click="saveConfig">
                    <template #icon>
                      <div class="i-carbon-save" aria-hidden="true" />
                    </template>
                    保存配置
                  </n-button>
                  <n-button secondary :loading="dedupLoading" class="action-btn" @click="executeDeduplicate">
                    <template #icon>
                      <div class="i-carbon-clean" aria-hidden="true" />
                    </template>
                    立即整理
                  </n-button>
                  <n-button secondary :loading="exportLoading" class="action-btn" @click="exportMemories">
                    <template #icon>
                      <div class="i-carbon-export" aria-hidden="true" />
                    </template>
                    导出记忆
                  </n-button>
                </div>
              </ConfigSection>

              <!-- 统计信息 -->
              <ConfigSection title="统计信息" :no-card="true">
                <div class="stats-grid">
                  <div class="stat-card stat-card--total">
                    <div class="stat-accent" />
                    <div class="stat-body">
                      <div class="stat-value stat-value--total">{{ stats.total }}</div>
                      <div class="stat-label">总计</div>
                    </div>
                  </div>
                  <div class="stat-card stat-card--rule">
                    <div class="stat-accent stat-accent--rule" />
                    <div class="stat-body">
                      <div class="stat-value stat-value--rule">{{ stats.rules }}</div>
                      <div class="stat-label">规范</div>
                    </div>
                  </div>
                  <div class="stat-card stat-card--preference">
                    <div class="stat-accent stat-accent--preference" />
                    <div class="stat-body">
                      <div class="stat-value stat-value--preference">{{ stats.preferences }}</div>
                      <div class="stat-label">偏好</div>
                    </div>
                  </div>
                  <div class="stat-card stat-card--pattern">
                    <div class="stat-accent stat-accent--pattern" />
                    <div class="stat-body">
                      <div class="stat-value stat-value--pattern">{{ stats.patterns }}</div>
                      <div class="stat-label">模式</div>
                    </div>
                  </div>
                  <div class="stat-card stat-card--context">
                    <div class="stat-accent stat-accent--context" />
                    <div class="stat-body">
                      <div class="stat-value stat-value--context">{{ stats.contexts }}</div>
                      <div class="stat-label">背景</div>
                    </div>
                  </div>
                </div>
              </ConfigSection>

              <!-- 去重结果 -->
              <n-collapse-transition :show="lastDedupResult !== null">
                <ConfigSection v-if="lastDedupResult" title="上次整理结果" :no-card="true">
                  <div class="dedup-result">
                    <div class="dedup-result-icon">
                      <div class="i-carbon-checkmark-outline" aria-hidden="true" />
                    </div>
                    <div class="dedup-result-body">
                      <div class="dedup-result-text">
                        移除 <strong>{{ lastDedupResult.removed_count }}</strong> 条重复记忆，
                        保留 <strong>{{ lastDedupResult.remaining_count }}</strong> 条
                      </div>
                    </div>
                  </div>
                </ConfigSection>
              </n-collapse-transition>
            </div>
          </div>
        </n-tab-pane>

        <!-- 记忆列表 Tab -->
        <n-tab-pane name="list" tab="记忆列表">
            <MemoryList
              ref="memoryListRef"
              :project-root-path="projectPath"
              :active="currentTab === 'list'"
              @refresh="loadMemories"
              @stats-updated="handleStatsUpdate"
            />
        </n-tab-pane>

        <!-- 搜索 Tab -->
        <n-tab-pane name="search" tab="搜索">
            <div class="tab-content">
              <MemorySearch
                ref="memorySearchRef"
                :project-root-path="projectPath"
                @edit="handleEdit"
                @delete="handleSearchDelete"
              />
            </div>
        </n-tab-pane>

        <!-- 相似度预览 Tab -->
        <n-tab-pane name="preview" tab="相似度预览">
            <div class="tab-content">
              <div class="preview-sections">
                <ConfigSection title="输入检测" description="输入内容检测与现有记忆的相似度">
                  <div class="preview-input-group">
                    <n-input
                      v-model:value="previewContent"
                      type="textarea"
                      :rows="3"
                      placeholder="输入要检测的内容..."
                      aria-label="待检测内容输入框"
                      class="preview-textarea"
                    />
                    <n-button
                      type="primary"
                      :loading="previewLoading"
                      :disabled="!previewContent.trim()"
                      aria-describedby="similarity-result"
                      @click="previewSimilarity"
                    >
                      <template #icon>
                        <div class="i-carbon-search" aria-hidden="true" />
                      </template>
                      检测相似度
                    </n-button>
                  </div>
                </ConfigSection>

                <!-- 检测结果 -->
                <n-collapse-transition :show="previewResult !== null">
                  <ConfigSection v-if="previewResult" title="检测结果" :no-card="true">
                    <div class="preview-result">
                      <!-- 相似度指示器 -->
                      <div class="similarity-indicator">
                        <div class="similarity-track">
                          <div
                            class="similarity-bar"
                            :class="{
                              'similarity-bar--duplicate': previewResult.is_duplicate,
                              'similarity-bar--safe': !previewResult.is_duplicate,
                            }"
                            :style="{ width: `${previewResult.similarity * 100}%` }"
                          />
                        </div>
                        <div class="similarity-info">
                          <span class="similarity-score" :class="{ 'similarity-score--duplicate': previewResult.is_duplicate }">
                            {{ (previewResult.similarity * 100).toFixed(1) }}%
                          </span>
                          <span class="similarity-threshold">
                            阈值 {{ (previewResult.threshold * 100).toFixed(0) }}%
                          </span>
                        </div>
                      </div>

                      <!-- 结果状态 -->
                      <div class="similarity-status" :class="{ 'similarity-status--duplicate': previewResult.is_duplicate }">
                        <div class="similarity-status-icon">
                          <div :class="previewResult.is_duplicate ? 'i-carbon-warning' : 'i-carbon-checkmark'" aria-hidden="true" />
                        </div>
                        <span class="similarity-status-text">
                          {{ previewResult.is_duplicate ? '检测到相似内容，添加时将被拒绝' : '未检测到相似内容，可以添加' }}
                        </span>
                      </div>

                      <!-- 匹配的内容 -->
                      <div v-if="previewResult.matched_content" class="matched-content">
                        <div class="matched-accent" />
                        <div class="matched-body">
                          <div class="matched-label">
                            <div class="i-carbon-link" aria-hidden="true" />
                            最相似的记忆
                          </div>
                          <div class="matched-text">{{ previewResult.matched_content }}</div>
                        </div>
                      </div>
                    </div>
                  </ConfigSection>
                </n-collapse-transition>
              </div>
            </div>
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
      <div class="edit-modal-content">
        <div v-if="editingMemory" class="edit-info">
          <span :class="['category-badge', getCategoryBgClass(editingMemory.category)]">
            <div :class="getCategoryIcon(editingMemory.category)" aria-hidden="true" />
            {{ editingMemory.category }}
          </span>
          <span class="edit-time">{{ formatDate(editingMemory.created_at) }}</span>
        </div>

        <n-input
          v-model:value="editContent"
          type="textarea"
          :rows="6"
          placeholder="输入新内容..."
          :disabled="editSaving"
          class="edit-textarea"
        />
      </div>

      <template #footer>
        <div class="edit-footer">
          <n-button @click="closeEditModal" :disabled="editSaving">
            取消
          </n-button>
          <n-button type="primary" :loading="editSaving" @click="saveEdit">
            保存
          </n-button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.memory-config {
  min-height: 400px;
}

.tab-content {
  padding: 16px 4px;
}

.config-sections,
.preview-sections {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* ============ 空状态 ============ */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 280px;
  gap: 6px;
}

.empty-icon-container {
  width: 56px;
  height: 56px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.06), rgba(59, 130, 246, 0.04));
  border: 1px dashed rgba(20, 184, 166, 0.2);
  margin-bottom: 4px;
}

:root.dark .empty-icon-container {
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.1), rgba(59, 130, 246, 0.08));
  border-color: rgba(20, 184, 166, 0.25);
}

.empty-icon-container [class^="i-carbon-"] {
  font-size: 26px;
  color: rgba(20, 184, 166, 0.4);
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

/* ============ 去重设置 ============ */
.dedup-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.threshold-control {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.threshold-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.threshold-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-on-surface, #111827);
}

:root.dark .threshold-label {
  color: #f3f4f6;
}

.threshold-value {
  font-size: 14px;
  font-weight: 700;
  color: rgba(20, 184, 166, 0.9);
  font-variant-numeric: tabular-nums;
  padding: 2px 10px;
  border-radius: 6px;
  background: rgba(20, 184, 166, 0.08);
}

:root.dark .threshold-value {
  background: rgba(20, 184, 166, 0.12);
}

.threshold-hint {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  margin-top: 2px;
}

/* ============ 开关组 ============ */
.switch-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-radius: 10px;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.12));
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.switch-item:hover {
  border-color: rgba(20, 184, 166, 0.25);
  background: var(--color-container, rgba(255, 255, 255, 0.8));
  box-shadow: 0 2px 8px rgba(20, 184, 166, 0.05);
}

:root.dark .switch-item {
  background: rgba(28, 28, 34, 0.5);
  border-color: rgba(255, 255, 255, 0.06);
}

:root.dark .switch-item:hover {
  background: rgba(32, 32, 38, 0.6);
  border-color: rgba(20, 184, 166, 0.3);
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
  color: #f3f4f6;
}

.switch-desc {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-top: 3px;
  line-height: 1.4;
}

:root.dark .switch-desc {
  color: #9ca3af;
}

/* ============ 快捷操作 ============ */
.quick-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.action-btn {
  transition: all 0.2s ease;
}

/* ============ 统计网格 ============ */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 10px;
}

.stat-card {
  display: flex;
  border-radius: 12px;
  overflow: hidden;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.12));
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.stat-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
  border-color: rgba(20, 184, 166, 0.25);
}

:root.dark .stat-card {
  background: rgba(28, 28, 34, 0.5);
  border-color: rgba(255, 255, 255, 0.06);
}

:root.dark .stat-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  border-color: rgba(20, 184, 166, 0.3);
}

/* 左侧色条 */
.stat-accent {
  width: 3px;
  flex-shrink: 0;
  background: linear-gradient(180deg, rgba(20, 184, 166, 0.6), rgba(20, 184, 166, 0.3));
}

.stat-accent--rule {
  background: linear-gradient(180deg, #3b82f6, #60a5fa);
}

.stat-accent--preference {
  background: linear-gradient(180deg, #a855f7, #c084fc);
}

.stat-accent--pattern {
  background: linear-gradient(180deg, #22c55e, #4ade80);
}

.stat-accent--context {
  background: linear-gradient(180deg, #f97316, #fb923c);
}

.stat-body {
  flex: 1;
  text-align: center;
  padding: 14px 8px;
}

.stat-value {
  font-size: 26px;
  font-weight: 700;
  color: var(--color-on-surface, #111827);
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
}

:root.dark .stat-value {
  color: #f3f4f6;
}

.stat-value--total {
  color: rgba(20, 184, 166, 0.9);
}

:root.dark .stat-value--total {
  color: rgba(45, 212, 191, 0.9);
}

.stat-value--rule {
  color: #3b82f6;
}

:root.dark .stat-value--rule {
  color: #60a5fa;
}

.stat-value--preference {
  color: #a855f7;
}

:root.dark .stat-value--preference {
  color: #c084fc;
}

.stat-value--pattern {
  color: #22c55e;
}

:root.dark .stat-value--pattern {
  color: #4ade80;
}

.stat-value--context {
  color: #f97316;
}

:root.dark .stat-value--context {
  color: #fb923c;
}

.stat-label {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-top: 6px;
  font-weight: 500;
}

/* ============ 去重结果 ============ */
.dedup-result {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px;
  border-radius: 12px;
  background: rgba(34, 197, 94, 0.06);
  border: 1px solid rgba(34, 197, 94, 0.15);
}

:root.dark .dedup-result {
  background: rgba(34, 197, 94, 0.1);
  border-color: rgba(34, 197, 94, 0.2);
}

.dedup-result-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(34, 197, 94, 0.12);
  flex-shrink: 0;
}

:root.dark .dedup-result-icon {
  background: rgba(34, 197, 94, 0.18);
}

.dedup-result-icon [class^="i-carbon-"] {
  font-size: 18px;
  color: #16a34a;
}

:root.dark .dedup-result-icon [class^="i-carbon-"] {
  color: #4ade80;
}

.dedup-result-body {
  flex: 1;
}

.dedup-result-text {
  font-size: 13px;
  line-height: 1.5;
  color: var(--color-on-surface, #374151);
}

:root.dark .dedup-result-text {
  color: #d1d5db;
}

.dedup-result-text strong {
  color: #16a34a;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

:root.dark .dedup-result-text strong {
  color: #4ade80;
}

/* ============ 相似度预览 ============ */
.preview-input-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.preview-textarea {
  /* 继承 Naive UI 默认样式 */
}

.preview-result {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
  border-radius: 12px;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.12));
}

:root.dark .preview-result {
  background: rgba(28, 28, 34, 0.5);
  border-color: rgba(255, 255, 255, 0.06);
}

/* 相似度进度条 */
.similarity-indicator {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.similarity-track {
  position: relative;
  height: 8px;
  border-radius: 4px;
  background: var(--color-border, rgba(128, 128, 128, 0.15));
  overflow: hidden;
}

.similarity-bar {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  border-radius: 4px;
  transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.similarity-bar--duplicate {
  background: linear-gradient(90deg, #ef4444, #f97316);
}

.similarity-bar--safe {
  background: linear-gradient(90deg, #22c55e, #14b8a6);
}

.similarity-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.similarity-score {
  font-size: 18px;
  font-weight: 700;
  color: #16a34a;
  font-variant-numeric: tabular-nums;
}

.similarity-score--duplicate {
  color: #dc2626;
}

:root.dark .similarity-score {
  color: #4ade80;
}

:root.dark .similarity-score--duplicate {
  color: #f87171;
}

.similarity-threshold {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #9ca3af);
  font-variant-numeric: tabular-nums;
}

/* 状态提示 */
.similarity-status {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 10px;
  background: rgba(34, 197, 94, 0.06);
  border: 1px solid rgba(34, 197, 94, 0.12);
}

.similarity-status--duplicate {
  background: rgba(239, 68, 68, 0.06);
  border-color: rgba(239, 68, 68, 0.12);
}

:root.dark .similarity-status {
  background: rgba(34, 197, 94, 0.1);
  border-color: rgba(34, 197, 94, 0.18);
}

:root.dark .similarity-status--duplicate {
  background: rgba(239, 68, 68, 0.1);
  border-color: rgba(239, 68, 68, 0.18);
}

.similarity-status-icon {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(34, 197, 94, 0.1);
  flex-shrink: 0;
}

.similarity-status--duplicate .similarity-status-icon {
  background: rgba(239, 68, 68, 0.1);
}

:root.dark .similarity-status-icon {
  background: rgba(34, 197, 94, 0.15);
}

:root.dark .similarity-status--duplicate .similarity-status-icon {
  background: rgba(239, 68, 68, 0.15);
}

.similarity-status-icon [class^="i-carbon-"] {
  font-size: 14px;
  color: #16a34a;
}

.similarity-status--duplicate .similarity-status-icon [class^="i-carbon-"] {
  color: #dc2626;
}

:root.dark .similarity-status-icon [class^="i-carbon-"] {
  color: #4ade80;
}

:root.dark .similarity-status--duplicate .similarity-status-icon [class^="i-carbon-"] {
  color: #f87171;
}

.similarity-status-text {
  font-size: 13px;
  color: var(--color-on-surface, #374151);
  font-weight: 500;
}

:root.dark .similarity-status-text {
  color: #d1d5db;
}

/* 匹配内容 - 带左侧色条 */
.matched-content {
  display: flex;
  border-radius: 10px;
  overflow: hidden;
  background: var(--color-border, rgba(128, 128, 128, 0.06));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
}

:root.dark .matched-content {
  background: rgba(255, 255, 255, 0.03);
  border-color: rgba(255, 255, 255, 0.06);
}

.matched-accent {
  width: 3px;
  flex-shrink: 0;
  background: linear-gradient(180deg, rgba(251, 191, 36, 0.6), rgba(251, 191, 36, 0.3));
}

.matched-body {
  flex: 1;
  padding: 12px 16px;
}

.matched-label {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin-bottom: 6px;
  font-weight: 600;
}

.matched-label [class^="i-carbon-"] {
  font-size: 12px;
  color: rgba(251, 191, 36, 0.6);
}

.matched-text {
  font-size: 13px;
  line-height: 1.7;
  color: var(--color-on-surface, #111827);
  word-break: break-word;
}

:root.dark .matched-text {
  color: #e5e7eb;
}

/* ============ 编辑模态框 ============ */
.edit-modal-content {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.edit-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--color-container, rgba(255, 255, 255, 0.6));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.12));
}

:root.dark .edit-info {
  background: rgba(28, 28, 34, 0.5);
  border-color: rgba(255, 255, 255, 0.06);
}

/* 分类标签 pill */
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

.edit-time {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  font-variant-numeric: tabular-nums;
}

.edit-textarea {
  /* 继承 Naive UI 默认样式 */
}

.edit-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
