/**
 * AppContent 集成测试
 * 验证 C2 修复：fallbackProjectPath 降级逻辑
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia } from 'pinia'
import AppContent from '../AppContent.vue'
import { invoke } from '@tauri-apps/api/core'

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

// Mock Naive UI
vi.mock('naive-ui', () => ({
  useMessage: () => ({
    error: vi.fn(),
    warning: vi.fn(),
    success: vi.fn(),
  }),
  NMessageProvider: { name: 'NMessageProvider', template: '<div><slot /></div>' },
  NTabs: { name: 'NTabs', template: '<div><slot /></div>' },
  NTabPane: { name: 'NTabPane', template: '<div><slot /></div>' },
}))

// Mock composables
vi.mock('../../composables/useAcemcpSync', () => ({
  useAcemcpSync: () => ({
    resyncAcemcp: vi.fn(),
  }),
}))

vi.mock('../../composables/useExitWarning', () => ({
  setupExitWarningListener: vi.fn(),
}))

vi.mock('../../composables/useKeyboard', () => ({
  useKeyboard: () => ({
    handleExitShortcut: vi.fn(),
  }),
}))

vi.mock('../../composables/useMcpTools', () => ({
  useMcpToolsReactive: () => ({
    mcpTools: { value: [] },
    loading: { value: false },
    loadMcpTools: vi.fn().mockResolvedValue(undefined),
  }),
}))

vi.mock('../../composables/useVersionCheck', () => ({
  setupAutoExitListener: vi.fn(),
}))

describe('AppContent - fallbackProjectPath 降级逻辑', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('非 MCP 弹窗模式下应该调用 get_current_dir（C2 修复验证）', async () => {
    const mockProjectPath = 'D:/test/project'
    vi.mocked(invoke).mockResolvedValue(mockProjectPath)

    const wrapper = mount(AppContent, {
      props: {
        showMcpPopup: false, // 非弹窗模式
        isIconMode: false,
        iconParams: null,
        appConfig: {},
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          IntroTab: true,
          McpToolsTab: true,
          PromptsTab: true,
          SettingsTab: true,
          IconPopupMode: true,
        },
      },
    })

    await flushPromises()

    // 验证 get_current_dir 被调用
    expect(invoke).toHaveBeenCalledWith('get_current_dir')

    // 验证 fallbackProjectPath 被设置
    expect(wrapper.vm.fallbackProjectPath).toBe(mockProjectPath)
  })

  it('MCP 弹窗模式下不应该调用 get_current_dir', async () => {
    const wrapper = mount(AppContent, {
      props: {
        showMcpPopup: true, // 弹窗模式
        isIconMode: false,
        iconParams: null,
        appConfig: {},
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          IntroTab: true,
          McpToolsTab: true,
          PromptsTab: true,
          SettingsTab: true,
          IconPopupMode: true,
        },
      },
    })

    await flushPromises()

    // 验证 get_current_dir 未被调用
    expect(invoke).not.toHaveBeenCalledWith('get_current_dir')

    // 验证 fallbackProjectPath 保持 null
    expect(wrapper.vm.fallbackProjectPath).toBeNull()
  })

  it('get_current_dir 失败时应该将 fallbackProjectPath 设为 null', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('获取目录失败'))

    const wrapper = mount(AppContent, {
      props: {
        showMcpPopup: false,
        isIconMode: false,
        iconParams: null,
        appConfig: {},
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          IntroTab: true,
          McpToolsTab: true,
          PromptsTab: true,
          SettingsTab: true,
          IconPopupMode: true,
        },
      },
    })

    await flushPromises()

    // 验证 get_current_dir 被调用
    expect(invoke).toHaveBeenCalledWith('get_current_dir')

    // 验证失败时 fallbackProjectPath 为 null
    expect(wrapper.vm.fallbackProjectPath).toBeNull()
  })

  it('应该将 fallbackProjectPath 传递给 McpToolsTab', async () => {
    const mockProjectPath = 'D:/test/project'
    vi.mocked(invoke).mockResolvedValue(mockProjectPath)

    const wrapper = mount(AppContent, {
      props: {
        showMcpPopup: false,
        isIconMode: false,
        iconParams: null,
        appConfig: {},
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          IntroTab: true,
          McpToolsTab: {
            name: 'McpToolsTab',
            props: ['projectRootPath'],
            template: '<div>{{ projectRootPath }}</div>',
          },
          PromptsTab: true,
          SettingsTab: true,
          IconPopupMode: true,
        },
      },
    })

    await flushPromises()

    // 验证 fallbackProjectPath 正确传递给 McpToolsTab
    expect(wrapper.html()).toContain(mockProjectPath)
  })
})
