// SC-19: 前端测试框架设置文件
// 提供全局测试工具和 mock

import { vi } from 'vitest'

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

// Mock Tauri Shell Plugin
vi.mock('@tauri-apps/plugin-shell', () => ({
  Command: {
    create: vi.fn(),
  },
}))

// 全局测试工具
declare global {
  // 用于在测试中 mock Tauri invoke
  function mockTauriInvoke<T>(command: string, result: T): void
}

globalThis.mockTauriInvoke = <T>(command: string, result: T) => {
  const { invoke } = vi.mocked(require('@tauri-apps/api/core'))
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === command)
      return result
    throw new Error(`Unexpected command: ${cmd}`)
  })
}
