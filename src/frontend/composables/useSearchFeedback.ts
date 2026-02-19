// SC-10: 搜索实时反馈 composable
// 提供搜索进度状态和阶段提示

import { computed, ref, watch } from 'vue'

export type SearchPhase =
  | 'idle'
  | 'indexing'
  | 'searching'
  | 'ranking'
  | 'completed'
  | 'error'

export interface SearchFeedbackOptions {
  /** 自动重置延迟（毫秒），0 表示不自动重置 */
  autoResetDelay?: number
}

export interface SearchProgress {
  /** 当前阶段 */
  phase: SearchPhase
  /** 已检索文件数 */
  filesSearched: number
  /** 总文件数（如果已知） */
  totalFiles?: number
  /** 错误信息 */
  errorMessage?: string
}

const phaseLabels: Record<SearchPhase, string> = {
  idle: '等待搜索',
  indexing: '建立索引',
  searching: '语义检索',
  ranking: '融合排序',
  completed: '搜索完成',
  error: '搜索失败',
}

/**
 * 搜索实时反馈 composable
 *
 * @example
 * ```ts
 * const { phase, progress, setPhase, setProgress, reset } = useSearchFeedback()
 *
 * // 开始搜索
 * setPhase('indexing')
 *
 * // 更新进度
 * setProgress({ filesSearched: 50, totalFiles: 100 })
 *
 * // 完成
 * setPhase('completed')
 * ```
 */
export function useSearchFeedback(options: SearchFeedbackOptions = {}) {
  const { autoResetDelay = 3000 } = options

  const progress = ref<SearchProgress>({
    phase: 'idle',
    filesSearched: 0,
  })

  // 计算属性：当前阶段标签
  const phaseLabel = computed(() => phaseLabels[progress.value.phase])

  // 计算属性：进度百分比
  const progressPercent = computed(() => {
    if (!progress.value.totalFiles || progress.value.totalFiles === 0)
      return undefined
    return Math.round(
      (progress.value.filesSearched / progress.value.totalFiles) * 100,
    )
  })

  // 计算属性：是否正在加载
  const isLoading = computed(() =>
    ['indexing', 'searching', 'ranking'].includes(progress.value.phase),
  )

  // 设置搜索阶段
  function setPhase(phase: SearchPhase) {
    progress.value.phase = phase
    if (phase === 'error') {
      progress.value.errorMessage = progress.value.errorMessage || '未知错误'
    }
  }

  // 更新进度
  function setProgress(update: Partial<Omit<SearchProgress, 'phase'>>) {
    Object.assign(progress.value, update)
  }

  // 设置错误
  function setError(message: string) {
    progress.value.phase = 'error'
    progress.value.errorMessage = message
  }

  // 重置状态
  function reset() {
    progress.value = {
      phase: 'idle',
      filesSearched: 0,
      totalFiles: undefined,
      errorMessage: undefined,
    }
  }

  // 自动重置（仅在 completed 阶段）
  let resetTimer: ReturnType<typeof setTimeout> | null = null

  watch(
    () => progress.value.phase,
    (phase) => {
      if (resetTimer) {
        clearTimeout(resetTimer)
        resetTimer = null
      }

      if (phase === 'completed' && autoResetDelay > 0) {
        resetTimer = setTimeout(() => {
          reset()
        }, autoResetDelay)
      }
    },
  )

  return {
    progress,
    phaseLabel,
    progressPercent,
    isLoading,
    setPhase,
    setProgress,
    setError,
    reset,
  }
}
