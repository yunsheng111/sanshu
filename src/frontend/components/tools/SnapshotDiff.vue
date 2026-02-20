<script setup lang="ts">
import { useMessage } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
/**
 * 版本快照对比组件
 * 调用后端"获取快照"操作获取版本历史
 * 支持选择两个版本进行文本 diff 对比
 * 简易实现：逐行比较，高亮差异行
 * 支持回滚到指定版本
 */
import { useSafeInvoke } from '../../composables/useSafeInvoke'

// Props
const props = defineProps<{
  projectRootPath: string
  memoryId: string
  /** 是否显示回滚按钮 */
  showRollback?: boolean
}>()

// Emits
const emit = defineEmits<{
  (e: 'rollback-success', memoryId: string): void
}>()

const message = useMessage()
const { safeInvoke, loading } = useSafeInvoke()

// ============ 类型定义 ============

interface Snapshot {
  /** 版本号 */
  version: number
  /** 快照时间 */
  timestamp: string
  /** 快照内容 */
  content: string
}

interface DiffLine {
  /** 行内容 */
  text: string
  /** 差异类型：added / removed / unchanged */
  type: 'added' | 'removed' | 'unchanged'
}

// ============ 状态 ============

/** 版本快照列表 */
const snapshots = ref<Snapshot[]>([])

/** 选中的版本 A（较旧） */
const versionA = ref<number | null>(null)

/** 选中的版本 B（较新） */
const versionB = ref<number | null>(null)

/** 回滚确认对话框状态 */
const rollbackModalVisible = ref(false)
const rollbackTargetVersion = ref<number | null>(null)
const rollbackLoading = ref(false)

// ============ 计算属性 ============

/** 版本选项列表 */
const versionOptions = computed(() => {
  return snapshots.value.map(s => ({
    label: `v${s.version} (${formatTime(s.timestamp)})`,
    value: s.version,
  }))
})

/** 版本 A 的内容 */
const contentA = computed(() => {
  if (versionA.value === null)
    return ''
  const snap = snapshots.value.find(s => s.version === versionA.value)
  return snap?.content ?? ''
})

/** 版本 B 的内容 */
const contentB = computed(() => {
  if (versionB.value === null)
    return ''
  const snap = snapshots.value.find(s => s.version === versionB.value)
  return snap?.content ?? ''
})

/** diff 结果 */
const diffLines = computed<DiffLine[]>(() => {
  if (!contentA.value && !contentB.value)
    return []
  return computeSimpleDiff(contentA.value, contentB.value)
})

/** 是否有差异 */
const hasDiff = computed(() => diffLines.value.some(l => l.type !== 'unchanged'))

/** 差异统计 */
const diffStats = computed(() => {
  const added = diffLines.value.filter(l => l.type === 'added').length
  const removed = diffLines.value.filter(l => l.type === 'removed').length
  return { added, removed }
})

// ============ 简易 diff 算法 ============

/**
 * 简易逐行 diff 实现
 * 对比两个文本的行级差异，无需外部 diff 库
 */
function computeSimpleDiff(textA: string, textB: string): DiffLine[] {
  const linesA = textA.split('\n')
  const linesB = textB.split('\n')
  const result: DiffLine[] = []

  // 使用 LCS（最长公共子序列）简化版
  const maxLen = Math.max(linesA.length, linesB.length)
  let i = 0
  let j = 0

  while (i < linesA.length || j < linesB.length) {
    if (i < linesA.length && j < linesB.length && linesA[i] === linesB[j]) {
      // 相同行
      result.push({ text: linesA[i], type: 'unchanged' })
      i++
      j++
    }
    else if (j < linesB.length && (i >= linesA.length || !linesA.slice(i).includes(linesB[j]))) {
      // B 中新增的行
      result.push({ text: linesB[j], type: 'added' })
      j++
    }
    else if (i < linesA.length && (j >= linesB.length || !linesB.slice(j).includes(linesA[i]))) {
      // A 中被删除的行
      result.push({ text: linesA[i], type: 'removed' })
      i++
    }
    else {
      // 两行都存在但不同 —— 标记为删除旧行 + 添加新行
      result.push({ text: linesA[i], type: 'removed' })
      result.push({ text: linesB[j], type: 'added' })
      i++
      j++
    }
  }

  return result
}

// ============ 工具函数 ============

function formatTime(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString('zh-CN')
  }
  catch {
    return isoString
  }
}

// ============ 回滚功能 ============

/** 打开回滚确认对话框 */
function handleOpenRollback(version: number) {
  rollbackTargetVersion.value = version
  rollbackModalVisible.value = true
}

/** 关闭回滚确认对话框 */
function handleCloseRollback() {
  rollbackModalVisible.value = false
  rollbackTargetVersion.value = null
}

/** 执行回滚 */
async function handleConfirmRollback() {
  if (rollbackTargetVersion.value === null)
    return

  rollbackLoading.value = true
  try {
    const result = await safeInvoke<boolean>('rollback_memory', {
      projectPath: props.projectRootPath,
      memoryId: props.memoryId,
      targetVersion: rollbackTargetVersion.value,
    })

    if (result) {
      emit('rollback-success', props.memoryId)
      handleCloseRollback()
    }
    else {
      message.error('回滚失败，请重试')
    }
  }
  catch (err) {
    message.error(`回滚失败：${err instanceof Error ? err.message : '未知错误'}`)
  }
  finally {
    rollbackLoading.value = false
  }
}

/** 获取目标版本的快照信息 */
const rollbackTargetSnapshot = computed(() => {
  if (rollbackTargetVersion.value === null)
    return null
  return snapshots.value.find(s => s.version === rollbackTargetVersion.value) ?? null
})

// ============ 数据加载 ============

async function loadSnapshots() {
  const result = await safeInvoke<Snapshot[]>('get_memory_snapshots', {
    projectPath: props.projectRootPath,
    memoryId: props.memoryId,
  })

  if (result && result.length > 0) {
    snapshots.value = result.sort((a, b) => a.version - b.version)

    // 自动选择最近两个版本
    if (result.length >= 2) {
      versionA.value = snapshots.value[snapshots.value.length - 2].version
      versionB.value = snapshots.value[snapshots.value.length - 1].version
    }
    else if (result.length === 1) {
      versionB.value = snapshots.value[0].version
    }
  }
}

// ============ 生命周期 ============

watch(() => props.memoryId, () => {
  loadSnapshots()
})

onMounted(() => {
  loadSnapshots()
})
</script>

<template>
  <div
    class="snapshot-diff"
    role="region"
    aria-label="版本对比"
  >
    <!-- 加载中 -->
    <div v-if="loading" class="diff-loading" aria-busy="true">
      <div class="loading-skeleton">
        <n-skeleton text style="width: 40%" />
        <n-skeleton text style="width: 100%; height: 60px; margin-top: 8px;" />
        <n-skeleton text style="width: 70%; margin-top: 8px;" />
      </div>
    </div>

    <!-- 无版本历史 -->
    <div v-else-if="snapshots.length === 0" class="diff-empty">
      <div class="empty-icon-container">
        <div class="i-carbon-version" aria-hidden="true" />
      </div>
      <span class="empty-text">暂无版本历史</span>
      <span class="empty-hint">编辑记忆后将自动保存版本快照</span>
    </div>

    <!-- 版本对比 -->
    <template v-else>
      <!-- 版本选择器 -->
      <div class="version-selector">
        <div class="version-picker">
          <div class="picker-header">
            <div class="i-carbon-undo picker-icon picker-icon--old" aria-hidden="true" />
            <span class="picker-label">旧版本</span>
          </div>
          <n-select
            v-model:value="versionA"
            :options="versionOptions"
            size="small"
            placeholder="选择版本"
            class="version-select"
            aria-label="选择旧版本"
          />
        </div>

        <div class="version-arrow">
          <div class="i-carbon-arrow-right" aria-hidden="true" />
        </div>

        <div class="version-picker">
          <div class="picker-header">
            <div class="i-carbon-redo picker-icon picker-icon--new" aria-hidden="true" />
            <span class="picker-label">新版本</span>
          </div>
          <n-select
            v-model:value="versionB"
            :options="versionOptions"
            size="small"
            placeholder="选择版本"
            class="version-select"
            aria-label="选择新版本"
          />
        </div>
      </div>

      <!-- diff 结果 -->
      <div v-if="versionA !== null && versionB !== null" class="diff-result">
        <!-- 差异统计 -->
        <div v-if="hasDiff" class="diff-stats">
          <span class="diff-stat diff-stat--added">+{{ diffStats.added }}</span>
          <span class="diff-stat diff-stat--removed">-{{ diffStats.removed }}</span>
        </div>

        <div v-if="!hasDiff" class="diff-same" role="status">
          <div class="diff-same-icon">
            <div class="i-carbon-checkmark" aria-hidden="true" />
          </div>
          <span>两个版本内容相同</span>
        </div>

        <div v-else class="diff-content" role="code" aria-label="差异对比">
          <div
            v-for="(line, idx) in diffLines"
            :key="idx"
            class="diff-line"
            :class="{
              'diff-added': line.type === 'added',
              'diff-removed': line.type === 'removed',
            }"
          >
            <span class="diff-line-number">{{ idx + 1 }}</span>
            <span class="diff-indicator" aria-hidden="true">
              {{ line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' ' }}
            </span>
            <span class="diff-text">{{ line.text || ' ' }}</span>
          </div>
        </div>
      </div>

      <!-- 版本数量提示 + 回滚按钮 -->
      <div class="version-footer">
        <div class="version-info" role="status">
          <div class="i-carbon-catalog version-info-icon" aria-hidden="true" />
          <span>共 {{ snapshots.length }} 个版本</span>
        </div>
        <n-button
          v-if="showRollback && versionA !== null && snapshots.length > 1"
          size="small"
          type="warning"
          secondary
          class="rollback-btn"
          aria-label="回滚到旧版本"
          @click="handleOpenRollback(versionA)"
        >
          <template #icon>
            <div class="i-carbon-reset" aria-hidden="true" />
          </template>
          回滚到 v{{ versionA }}
        </n-button>
      </div>
    </template>

    <!-- 回滚确认对话框 -->
    <n-modal
      v-model:show="rollbackModalVisible"
      preset="dialog"
      type="warning"
      title="确认回滚"
      :positive-text="rollbackLoading ? '回滚中...' : '确认回滚'"
      negative-text="取消"
      :loading="rollbackLoading"
      :mask-closable="!rollbackLoading"
      :close-on-esc="!rollbackLoading"
      aria-label="回滚确认对话框"
      @positive-click="handleConfirmRollback"
      @negative-click="handleCloseRollback"
      @close="handleCloseRollback"
    >
      <div class="rollback-confirm-content">
        <p>确定要将此记忆回滚到 <strong>v{{ rollbackTargetVersion }}</strong> 吗？</p>
        <p v-if="rollbackTargetSnapshot" class="rollback-time">
          版本时间：{{ formatTime(rollbackTargetSnapshot.timestamp) }}
        </p>
        <p class="rollback-warning">
          此操作将覆盖当前内容，但会保留版本历史。
        </p>
      </div>
    </n-modal>
  </div>
</template>

<style scoped>
.snapshot-diff {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 加载状态 */
.diff-loading {
  padding: 16px;
}

.loading-skeleton {
  display: flex;
  flex-direction: column;
}

/* 空状态 - 重设计 */
.diff-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 12px;
  gap: 6px;
}

.empty-icon-container {
  width: 40px;
  height: 40px;
  border-radius: 12px;
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

.empty-icon-container .i-carbon-version {
  font-size: 18px;
  color: rgba(20, 184, 166, 0.4);
}

.empty-text {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-on-surface-secondary, #6b7280);
  opacity: 0.7;
}

.empty-hint {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
}

/* 版本选择器 */
.version-selector {
  display: flex;
  align-items: flex-end;
  gap: 10px;
  padding: 12px 14px;
  border-radius: 10px;
  background: var(--color-container, rgba(255, 255, 255, 0.4));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
}

:root.dark .version-selector {
  background: rgba(24, 24, 30, 0.3);
  border-color: rgba(255, 255, 255, 0.04);
}

.version-picker {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}

.picker-header {
  display: flex;
  align-items: center;
  gap: 4px;
}

.picker-icon {
  font-size: 12px;
}

.picker-icon--old {
  color: rgba(251, 191, 36, 0.6);
}

.picker-icon--new {
  color: rgba(20, 184, 166, 0.6);
}

.picker-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-on-surface-secondary, #6b7280);
}

:root.dark .picker-label {
  color: #9ca3af;
}

.version-select {
  min-width: 120px;
}

.version-arrow {
  display: flex;
  align-items: center;
  justify-content: center;
  padding-bottom: 4px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.4;
  font-size: 14px;
}

/* diff 结果 */
.diff-result {
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.12));
  background: var(--color-container, rgba(255, 255, 255, 0.3));
}

:root.dark .diff-result {
  border-color: rgba(255, 255, 255, 0.06);
  background: rgba(20, 20, 26, 0.3);
}

/* 差异统计 */
.diff-stats {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
  background: rgba(128, 128, 128, 0.02);
}

:root.dark .diff-stats {
  border-color: rgba(255, 255, 255, 0.04);
  background: rgba(255, 255, 255, 0.02);
}

.diff-stat {
  font-size: 11px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  padding: 1px 6px;
  border-radius: 4px;
}

.diff-stat--added {
  color: #16a34a;
  background: rgba(34, 197, 94, 0.1);
}

.diff-stat--removed {
  color: #dc2626;
  background: rgba(239, 68, 68, 0.1);
}

:root.dark .diff-stat--added {
  color: #4ade80;
  background: rgba(34, 197, 94, 0.15);
}

:root.dark .diff-stat--removed {
  color: #f87171;
  background: rgba(239, 68, 68, 0.15);
}

.diff-same {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 16px;
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
}

.diff-same-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(34, 197, 94, 0.1);
  color: #16a34a;
  font-size: 12px;
}

:root.dark .diff-same-icon {
  background: rgba(34, 197, 94, 0.15);
  color: #4ade80;
}

.diff-content {
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.6;
  max-height: 300px;
  overflow-y: auto;
}

.diff-line {
  display: flex;
  padding: 1px 8px;
  min-height: 1.6em;
  transition: background 0.15s ease;
}

.diff-line:hover {
  background: rgba(128, 128, 128, 0.04);
}

.diff-added {
  background: rgba(34, 197, 94, 0.08);
  color: #16a34a;
}

.diff-added:hover {
  background: rgba(34, 197, 94, 0.12);
}

:root.dark .diff-added {
  background: rgba(34, 197, 94, 0.12);
  color: #4ade80;
}

:root.dark .diff-added:hover {
  background: rgba(34, 197, 94, 0.18);
}

.diff-removed {
  background: rgba(239, 68, 68, 0.08);
  color: #dc2626;
  text-decoration: line-through;
}

.diff-removed:hover {
  background: rgba(239, 68, 68, 0.12);
}

:root.dark .diff-removed {
  background: rgba(239, 68, 68, 0.12);
  color: #f87171;
}

:root.dark .diff-removed:hover {
  background: rgba(239, 68, 68, 0.18);
}

.diff-line-number {
  width: 28px;
  flex-shrink: 0;
  text-align: right;
  padding-right: 8px;
  font-size: 10px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
  user-select: none;
  font-variant-numeric: tabular-nums;
}

.diff-indicator {
  width: 16px;
  flex-shrink: 0;
  text-align: center;
  font-weight: 700;
  user-select: none;
}

.diff-text {
  flex: 1;
  white-space: pre-wrap;
  word-break: break-word;
}

/* 版本信息 */
.version-info {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
}

.version-info-icon {
  font-size: 12px;
}
</style>
