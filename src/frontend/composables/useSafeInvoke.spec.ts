// SC-19: useSafeInvoke composable 单元测试
import { describe, expect, it, vi } from 'vitest'
import { useSafeInvoke } from './useSafeInvoke'

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('useSafeInvoke', () => {
  it('返回正确的初始状态', () => {
    const { error, loading } = useSafeInvoke()

    expect(error.value).toBeNull()
    expect(loading.value).toBe(false)
  })

  it('safeInvoke 成功时返回结果', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockResolvedValueOnce('test-result')

    const { safeInvoke, error, loading } = useSafeInvoke()
    const result = await safeInvoke<string>('test_command')

    expect(result).toBe('test-result')
    expect(error.value).toBeNull()
    expect(loading.value).toBe(false)
  })

  it('safeInvoke 失败时返回 null 并设置 error', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockRejectedValueOnce(new Error('连接失败'))

    const { safeInvoke, error } = useSafeInvoke()
    const result = await safeInvoke<string>('test_command')

    expect(result).toBeNull()
    expect(error.value).toBe('连接失败')
  })

  it('safeInvoke 超时时返回 null', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockImplementationOnce(
      () => new Promise(resolve => setTimeout(resolve, 5000)),
    )

    const { safeInvoke, error } = useSafeInvoke()
    const result = await safeInvoke<string>('test_command', {}, { timeout: 100 })

    expect(result).toBeNull()
    expect(error.value).toContain('IPC 调用超时')
  })

  it('clearError 清除错误状态', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockRejectedValueOnce(new Error('测试错误'))

    const { safeInvoke, error, clearError } = useSafeInvoke()
    await safeInvoke<string>('test_command')

    expect(error.value).not.toBeNull()

    clearError()
    expect(error.value).toBeNull()
  })

  it('silent 模式不输出 console.error', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockRejectedValueOnce(new Error('静默错误'))
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    const { safeInvoke } = useSafeInvoke()
    await safeInvoke<string>('test_command', {}, { silent: true })

    expect(consoleSpy).not.toHaveBeenCalled()
    consoleSpy.mockRestore()
  })
})
