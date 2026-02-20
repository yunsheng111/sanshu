<script setup lang="ts">
/**
 * 批量操作条组件
 * 底部固定显示，多选模式下提供批量操作
 * 支持：批量删除 / 批量重新分类 / 批量导出 / 刷新活力值
 * 操作前 confirm 弹窗确认
 */
import { useSafeInvoke } from '../../composables/useSafeInvoke'
import { useMessage } from 'naive-ui'
import { computed, ref } from 'vue'

// Props
const props = defineProps<{
  projectRootPath: string
  selectedIds: string[]
  memories: Array<{
    id: string
    content: string
    category: string
  }>
}>()

// Emits
const emit = defineEmits<{
  (e: 'complete'): void
  (e: 'cancel'): void
}>()

const message = useMessage()
const { safeInvoke } = useSafeInvoke()

// ============ 状态 ============

const actionLoading = ref(false)

/** 重新分类弹窗 */
const showReclassify = ref(false)
const targetCategory = ref<string | null>(null)

// ============ 分类选项 ============

const categoryOptions = [
  { label: '规范', value: '规范' },
  { label: '偏好', value: '偏好' },
  { label: '模式', value: '模式' },
  { label: '背景', value: '背景' },
]

// ============ 计算属性 ============

const selectedCount = computed(() => props.selectedIds.length)

// ============ 批量操作 ============

/** 批量删除 */
async function batchDelete() {
  actionLoading.value = true
  try {
    let successCount = 0
    for (const id of props.selectedIds) {
      const result = await safeInvoke<boolean>('delete_memory', {
        projectPath: props.projectRootPath,
        memoryId: id,
      })
      if (result !== null) successCount++
    }
    message.success(`已删除 ${successCount} 条记忆`)
    emit('complete')
  } catch (err) {
    message.error(`批量删除失败: ${err}`)
  } finally {
    actionLoading.value = false
  }
}

/** 批量重新分类 */
async function batchReclassify() {
  if (!targetCategory.value) {
    message.warning('请选择目标分类')
    return
  }

  showReclassify.value = false
  actionLoading.value = true

  try {
    let successCount = 0
    for (const id of props.selectedIds) {
      const result = await safeInvoke<boolean>('update_memory_category', {
        projectPath: props.projectRootPath,
        memoryId: id,
        category: targetCategory.value,
      })
      if (result !== null) successCount++
    }
    message.success(`已重新分类 ${successCount} 条记忆`)
    emit('complete')
  } catch (err) {
    message.error(`批量分类失败: ${err}`)
  } finally {
    actionLoading.value = false
  }
}

/** 批量导出 */
async function batchExport() {
  actionLoading.value = true
  try {
    const selectedMemories = props.memories.filter(m => props.selectedIds.includes(m.id))
    const exportData = {
      exported_at: new Date().toISOString(),
      count: selectedMemories.length,
      entries: selectedMemories,
    }

    const jsonStr = JSON.stringify(exportData, null, 2)
    const blob = new Blob([jsonStr], { type: 'application/json' })
    const url = URL.createObjectURL(blob)

    const a = document.createElement('a')
    a.href = url
    a.download = `sanshu-memories-selected-${new Date().toISOString().slice(0, 10)}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)

    message.success(`已导出 ${selectedMemories.length} 条记忆`)
  } catch (err) {
    message.error(`导出失败: ${err}`)
  } finally {
    actionLoading.value = false
  }
}

/** 刷新活力值 */
async function refreshVitality() {
  actionLoading.value = true
  try {
    const result = await safeInvoke<boolean>('refresh_vitality', {
      projectPath: props.projectRootPath,
      memoryIds: props.selectedIds,
    })
    if (result !== null) {
      message.success('活力值已刷新')
      emit('complete')
    }
  } catch (err) {
    message.error(`刷新活力值失败: ${err}`)
  } finally {
    actionLoading.value = false
  }
}
</script>

<template>
  <n-affix :bottom="16" :trigger-bottom="0" class="batch-affix">
    <div
      class="batch-action-bar"
      role="toolbar"
      aria-label="批量操作工具栏"
    >
      <!-- 左侧选中信息 -->
      <div class="bar-info">
        <div class="bar-info-badge">
          <div class="i-carbon-checkbox-checked" aria-hidden="true" />
          <span class="bar-info-count">{{ selectedCount }}</span>
        </div>
        <span class="bar-info-text">条记忆已选择</span>
      </div>

      <!-- 右侧操作按钮组 -->
      <div class="bar-actions">
        <!-- 批量删除 -->
        <n-popconfirm
          @positive-click="batchDelete"
        >
          <template #trigger>
            <n-button
              type="error"
              size="small"
              secondary
              :loading="actionLoading"
              aria-label="批量删除选中记忆"
              class="action-btn"
            >
              <template #icon>
                <div class="i-carbon-trash-can" aria-hidden="true" />
              </template>
              删除
            </n-button>
          </template>
          确定要删除选中的 {{ selectedCount }} 条记忆吗？此操作不可撤销。
        </n-popconfirm>

        <!-- 分隔线 -->
        <div class="bar-divider" />

        <!-- 批量重新分类 -->
        <n-button
          size="small"
          secondary
          :loading="actionLoading"
          aria-label="批量重新分类"
          class="action-btn"
          @click="showReclassify = true"
        >
          <template #icon>
            <div class="i-carbon-category" aria-hidden="true" />
          </template>
          分类
        </n-button>

        <!-- 批量导出 -->
        <n-button
          size="small"
          secondary
          :loading="actionLoading"
          aria-label="导出选中记忆"
          class="action-btn"
          @click="batchExport"
        >
          <template #icon>
            <div class="i-carbon-export" aria-hidden="true" />
          </template>
          导出
        </n-button>

        <!-- 刷新活力值 -->
        <n-button
          size="small"
          secondary
          :loading="actionLoading"
          aria-label="刷新选中记忆的活力值"
          class="action-btn"
          @click="refreshVitality"
        >
          <template #icon>
            <div class="i-carbon-renew" aria-hidden="true" />
          </template>
          刷新活力
        </n-button>

        <!-- 分隔线 -->
        <div class="bar-divider" />

        <!-- 取消 -->
        <n-button
          size="small"
          quaternary
          aria-label="退出批量模式"
          class="action-btn cancel-btn"
          @click="emit('cancel')"
        >
          <template #icon>
            <div class="i-carbon-close" aria-hidden="true" />
          </template>
          取消
        </n-button>
      </div>
    </div>
  </n-affix>

  <!-- 重新分类弹窗 -->
  <n-modal
    v-model:show="showReclassify"
    preset="card"
    title="批量重新分类"
    :style="{ width: '400px' }"
    :mask-closable="true"
  >
    <n-space vertical size="medium">
      <div class="reclassify-hint">
        将选中的 <strong>{{ selectedCount }}</strong> 条记忆移动到：
      </div>
      <n-select
        v-model:value="targetCategory"
        :options="categoryOptions"
        placeholder="选择目标分类"
        aria-label="目标分类"
      />
    </n-space>
    <template #footer>
      <n-space justify="end">
        <n-button @click="showReclassify = false">
          取消
        </n-button>
        <n-button
          type="primary"
          :loading="actionLoading"
          :disabled="!targetCategory"
          @click="batchReclassify"
        >
          确认分类
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<style scoped>
.batch-affix {
  width: 100%;
  z-index: 100;
}

.batch-action-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  border-radius: 14px;
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.92) 0%, rgba(248, 250, 252, 0.95) 100%);
  border: 1px solid rgba(20, 184, 166, 0.15);
  box-shadow:
    0 -2px 20px rgba(0, 0, 0, 0.06),
    0 0 0 1px rgba(20, 184, 166, 0.05);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}

:root.dark .batch-action-bar {
  background: linear-gradient(135deg, rgba(24, 24, 30, 0.92) 0%, rgba(20, 20, 26, 0.95) 100%);
  border-color: rgba(20, 184, 166, 0.2);
  box-shadow:
    0 -2px 20px rgba(0, 0, 0, 0.25),
    0 0 0 1px rgba(20, 184, 166, 0.08);
}

/* 左侧选中信息 */
.bar-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bar-info-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 10px;
  border-radius: 8px;
  background: rgba(20, 184, 166, 0.1);
  color: rgba(20, 184, 166, 0.9);
  font-size: 12px;
}

:root.dark .bar-info-badge {
  background: rgba(20, 184, 166, 0.15);
}

.bar-info-count {
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  font-size: 14px;
}

.bar-info-text {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-on-surface, #374151);
}

:root.dark .bar-info-text {
  color: #d1d5db;
}

/* 右侧操作按钮组 */
.bar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.action-btn {
  transition: all 0.2s ease;
}

.bar-divider {
  width: 1px;
  height: 20px;
  background: var(--color-border, rgba(128, 128, 128, 0.15));
  margin: 0 4px;
  flex-shrink: 0;
}

:root.dark .bar-divider {
  background: rgba(255, 255, 255, 0.08);
}

.cancel-btn {
  opacity: 0.6;
}

.cancel-btn:hover {
  opacity: 1;
}

/* 重新分类弹窗提示文字 */
.reclassify-hint {
  font-size: 14px;
  color: var(--color-on-surface, #374151);
  line-height: 1.5;
}

:root.dark .reclassify-hint {
  color: #d1d5db;
}
</style>
