// SC-17: IPC 弹性错误处理
// 提供带超时和错误处理的 Tauri invoke 包装器

import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

export interface SafeInvokeOptions {
  /** 超时时间（毫秒），默认 30000 */
  timeout?: number
  /** 是否在错误时静默（不抛出异常），默认 false */
  silent?: boolean
}

/**
 * 安全的 Tauri IPC 调用包装器
 *
 * 特性：
 * - 自动超时控制
 * - 错误状态管理
 * - 加载状态跟踪
 *
 * @example
 * ```ts
 * const { safeInvoke, error, loading } = useSafeInvoke()
 *
 * const result = await safeInvoke<string>('get_config', { key: 'theme' })
 * if (result) {
 *   console.log('配置值:', result)
 * } else {
 *   console.error('调用失败:', error.value)
 * }
 * ```
 */
export function useSafeInvoke() {
  const error = ref<string | null>(null)
  const loading = ref(false)

  /**
   * 执行 Tauri 命令调用
   *
   * @param command - Tauri 命令名称
   * @param args - 命令参数
   * @param options - 调用选项
   * @returns 成功时返回结果，失败时返回 null
   */
  async function safeInvoke<T>(
    command: string,
    args?: Record<string, unknown>,
    options?: SafeInvokeOptions
  ): Promise<T | null> {
    const timeout = options?.timeout ?? 30000 // 默认 30 秒
    const silent = options?.silent ?? false

    loading.value = true
    error.value = null

    try {
      const result = await Promise.race([
        invoke<T>(command, args),
        new Promise<never>((_, reject) =>
          setTimeout(
            () => reject(new Error(`IPC 调用超时 (${timeout}ms): ${command}`)),
            timeout
          )
        ),
      ])
      return result
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e)
      error.value = errorMessage

      if (!silent) {
        console.error(`[useSafeInvoke] 调用失败: ${command}`, errorMessage)
      }

      return null
    } finally {
      loading.value = false
    }
  }

  /**
   * 清除错误状态
   */
  function clearError() {
    error.value = null
  }

  return {
    safeInvoke,
    error,
    loading,
    clearError,
  }
}
