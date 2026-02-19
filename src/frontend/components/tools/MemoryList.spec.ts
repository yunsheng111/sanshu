// MemoryList.vue ARIA 属性渲染测试
// 验证无障碍访问标记的正确性
import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { h } from 'vue'
import MemoryList from './MemoryList.vue'

// Mock Naive UI 组件为简单占位符，避免 happy-dom 不支持的渲染问题
vi.mock('naive-ui', () => ({
  useMessage: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  }),
  NSelect: {
    name: 'NSelect',
    props: ['modelValue', 'options', 'placeholder'],
    setup(_: any, { attrs }: any) {
      return () => h('select', { ...attrs })
    },
  },
  NButton: {
    name: 'NButton',
    props: ['text', 'type', 'size', 'loading'],
    setup(_: any, { slots, attrs }: any) {
      return () => h('button', { ...attrs }, slots.default?.())
    },
  },
  NSkeleton: {
    name: 'NSkeleton',
    setup() {
      return () => h('div', { class: 'skeleton' })
    },
  },
  NInput: {
    name: 'NInput',
    props: ['modelValue', 'type', 'rows', 'placeholder'],
    setup(_: any, { attrs }: any) {
      return () => h('textarea', { ...attrs })
    },
  },
  NPopconfirm: {
    name: 'NPopconfirm',
    props: ['show'],
    setup(_: any, { slots }: any) {
      return () => slots.trigger?.()
    },
  },
  NPagination: {
    name: 'NPagination',
    setup() {
      return () => h('nav', { role: 'navigation' })
    },
  },
}))

// Mock Tauri invoke，返回包含记忆条目的模拟数据
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_memory_list') {
      return [
        { id: '1', content: '测试记忆内容', category: '规范', created_at: '2026-01-01T00:00:00Z' },
        { id: '2', content: '用户偏好设置', category: '偏好', created_at: '2026-01-02T00:00:00Z' },
      ]
    }
    if (cmd === 'get_memory_stats') {
      return { total: 2, rules: 1, preferences: 1, patterns: 0, contexts: 0 }
    }
    return null
  }),
}))

describe('MemoryList.vue ARIA 属性', () => {
  // --- 根容器 ARIA ---
  it('根容器应有 role="region" 和 aria-label="记忆列表"', async () => {
    // Arrange & Act
    const wrapper = mount(MemoryList, {
      props: {
        projectRootPath: '/test/project',
        active: true,
      },
    })

    // 等待异步数据加载完成
    await wrapper.vm.$nextTick()
    await new Promise(resolve => setTimeout(resolve, 50))
    await wrapper.vm.$nextTick()

    // Assert
    const root = wrapper.find('.memory-list')
    expect(root.exists()).toBe(true)
    expect(root.attributes('role')).toBe('region')
    expect(root.attributes('aria-label')).toBe('记忆列表')
  })

  // --- 列表容器 ARIA ---
  it('列表容器应有 role="list" 和 aria-label', async () => {
    // Arrange & Act
    const wrapper = mount(MemoryList, {
      props: {
        projectRootPath: '/test/project',
        active: true,
      },
    })

    // 等待数据加载
    await wrapper.vm.$nextTick()
    await new Promise(resolve => setTimeout(resolve, 50))
    await wrapper.vm.$nextTick()

    // Assert
    const listContainer = wrapper.find('.list-container')
    expect(listContainer.exists()).toBe(true)
    expect(listContainer.attributes('role')).toBe('list')
    expect(listContainer.attributes('aria-label')).toBe('记忆条目列表')
  })

  // --- 列表条目 ARIA ---
  it('每个记忆条目应有 role="listitem"', async () => {
    // Arrange & Act
    const wrapper = mount(MemoryList, {
      props: {
        projectRootPath: '/test/project',
        active: true,
      },
    })

    // 等待数据加载
    await wrapper.vm.$nextTick()
    await new Promise(resolve => setTimeout(resolve, 50))
    await wrapper.vm.$nextTick()

    // Assert
    const items = wrapper.findAll('.memory-item')
    expect(items.length).toBeGreaterThan(0)
    for (const item of items) {
      expect(item.attributes('role')).toBe('listitem')
    }
  })
})
