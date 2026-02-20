import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
/**
 * McpToolsTab 集成测试
 * 验证 C1 修复：MemoryManager 组件正确挂载
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import McpToolsTab from '../McpToolsTab.vue'

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
  NSpace: { name: 'NSpace', template: '<div><slot /></div>' },
  NAlert: { name: 'NAlert', template: '<div><slot /></div>' },
  NCard: { name: 'NCard', template: '<div><slot /></div>' },
  NModal: { name: 'NModal', template: '<div><slot /></div>' },
  NButton: { name: 'NButton', template: '<button><slot /></button>' },
  NSwitch: { name: 'NSwitch', template: '<input type="checkbox" />' },
  NSpin: { name: 'NSpin', template: '<div><slot /></div>' },
}))

// Mock useMcpToolsReactive
vi.mock('../../../composables/useMcpTools', () => ({
  useMcpToolsReactive: () => ({
    mcpTools: { value: [{ id: 'ji', name: '记忆管理', enabled: true }] },
    loading: { value: false },
    loadMcpTools: vi.fn().mockResolvedValue(undefined),
    toggleTool: vi.fn(),
    toolStats: { value: { total: 1, enabled: 1, disabled: 0 } },
  }),
}))

describe('mcpToolsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('应该正确挂载 MemoryManager 组件（C1 修复验证）', async () => {
    const wrapper = mount(McpToolsTab, {
      props: {
        projectRootPath: '/test/project',
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          // 异步组件需要 stub
          SouConfig: { name: 'SouConfig', template: '<div>SouConfig</div>' },
          Context7Config: { name: 'Context7Config', template: '<div>Context7Config</div>' },
          IconWorkshop: { name: 'IconWorkshop', template: '<div>IconWorkshop</div>' },
          EnhanceConfig: { name: 'EnhanceConfig', template: '<div>EnhanceConfig</div>' },
          MemoryManager: { name: 'MemoryManager', template: '<div>MemoryManager</div>' },
        },
      },
    })

    // 验证组件挂载成功
    expect(wrapper.exists()).toBe(true)

    // 模拟打开记忆管理配置弹窗
    wrapper.vm.currentToolId = 'ji'
    wrapper.vm.showToolConfigModal = true
    await wrapper.vm.$nextTick()

    // 验证 MemoryManager 组件被渲染（而非旧的 MemoryConfig）
    const html = wrapper.html()
    expect(html).toContain('MemoryManager')
    expect(html).not.toContain('MemoryConfig')
  })

  it('应该正确传递 projectRootPath prop 到 MemoryManager', async () => {
    const testPath = '/test/project/path'
    const wrapper = mount(McpToolsTab, {
      props: {
        projectRootPath: testPath,
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          SouConfig: true,
          Context7Config: true,
          IconWorkshop: true,
          EnhanceConfig: true,
          MemoryManager: {
            name: 'MemoryManager',
            props: ['projectRootPath'],
            template: '<div>{{ projectRootPath }}</div>',
          },
        },
      },
    })

    wrapper.vm.currentToolId = 'ji'
    wrapper.vm.showToolConfigModal = true
    await wrapper.vm.$nextTick()

    // 验证 projectRootPath 正确传递
    expect(wrapper.html()).toContain(testPath)
  })

  it('应该处理 projectRootPath 为 null 的情况', async () => {
    const wrapper = mount(McpToolsTab, {
      props: {
        projectRootPath: null,
      },
      global: {
        plugins: [createPinia()],
        stubs: {
          SouConfig: true,
          Context7Config: true,
          IconWorkshop: true,
          EnhanceConfig: true,
          MemoryManager: {
            name: 'MemoryManager',
            props: ['projectRootPath'],
            template: '<div>{{ projectRootPath === null ? "null" : projectRootPath }}</div>',
          },
        },
      },
    })

    wrapper.vm.currentToolId = 'ji'
    wrapper.vm.showToolConfigModal = true
    await wrapper.vm.$nextTick()

    // 验证 null 值正确传递（MemoryManager 内部会显示提示）
    expect(wrapper.html()).toContain('null')
  })
})
