// 渐进式披露状态管理 composable 单元测试
import { describe, expect, it } from 'vitest'
import { useProgressiveDisclosure } from '../useProgressiveDisclosure'

describe('useProgressiveDisclosure', () => {
  // --- 正常路径 ---
  describe('正常路径', () => {
    it('默认状态应为 collapsed', () => {
      // Arrange
      const { getState } = useProgressiveDisclosure()

      // Act
      const state = getState('any-id')

      // Assert
      expect(state).toBe('collapsed')
    })

    it('toggleNext 应按 collapsed -> expanded -> detail -> collapsed 循环', () => {
      // Arrange
      const { getState, toggleNext } = useProgressiveDisclosure()
      const id = 'test-1'

      // Act & Assert: collapsed -> expanded
      toggleNext(id)
      expect(getState(id)).toBe('expanded')

      // expanded -> detail
      toggleNext(id)
      expect(getState(id)).toBe('detail')

      // detail -> collapsed（循环回到起点）
      toggleNext(id)
      expect(getState(id)).toBe('collapsed')
    })

    it('expand 应将状态设置为 expanded', () => {
      const { getState, expand } = useProgressiveDisclosure()
      const id = 'expand-test'

      expand(id)
      expect(getState(id)).toBe('expanded')
    })

    it('collapse 应将状态设置为 collapsed', () => {
      const { getState, expand, collapse } = useProgressiveDisclosure()
      const id = 'collapse-test'

      // 先展开再折叠
      expand(id)
      expect(getState(id)).toBe('expanded')

      collapse(id)
      expect(getState(id)).toBe('collapsed')
    })

    it('showDetail 应将状态设置为 detail', () => {
      const { getState, showDetail } = useProgressiveDisclosure()
      const id = 'detail-test'

      showDetail(id)
      expect(getState(id)).toBe('detail')
    })

    it('setState 应直接设置任意合法状态', () => {
      const { getState, setState } = useProgressiveDisclosure()
      const id = 'set-state-test'

      setState(id, 'detail')
      expect(getState(id)).toBe('detail')

      setState(id, 'expanded')
      expect(getState(id)).toBe('expanded')

      setState(id, 'collapsed')
      expect(getState(id)).toBe('collapsed')
    })
  })

  // --- 边界条件 ---
  describe('边界条件', () => {
    it('多个条目应独立维护各自状态', () => {
      const { getState, expand, showDetail } = useProgressiveDisclosure()

      expand('item-a')
      showDetail('item-b')

      expect(getState('item-a')).toBe('expanded')
      expect(getState('item-b')).toBe('detail')
      // 未设置的条目应保持默认
      expect(getState('item-c')).toBe('collapsed')
    })

    it('collapseAll 应清空所有状态（回到 collapsed）', () => {
      const { getState, expand, showDetail, collapseAll } = useProgressiveDisclosure()

      expand('a')
      showDetail('b')
      expand('c')

      collapseAll()

      expect(getState('a')).toBe('collapsed')
      expect(getState('b')).toBe('collapsed')
      expect(getState('c')).toBe('collapsed')
    })

    it('对同一条目重复设置相同状态应幂等', () => {
      const { getState, expand } = useProgressiveDisclosure()
      const id = 'idempotent'

      expand(id)
      expand(id)
      expand(id)

      expect(getState(id)).toBe('expanded')
    })

    it('空字符串 ID 应正常工作', () => {
      const { getState, expand } = useProgressiveDisclosure()

      expand('')
      expect(getState('')).toBe('expanded')
    })
  })

  // --- isState 判断 ---
  describe('isState 判断', () => {
    it('isState 应正确判断当前状态', () => {
      const { isState, expand, showDetail } = useProgressiveDisclosure()
      const id = 'is-state-test'

      // 默认状态
      expect(isState(id, 'collapsed')).toBe(true)
      expect(isState(id, 'expanded')).toBe(false)
      expect(isState(id, 'detail')).toBe(false)

      // 展开后
      expand(id)
      expect(isState(id, 'collapsed')).toBe(false)
      expect(isState(id, 'expanded')).toBe(true)

      // 详情后
      showDetail(id)
      expect(isState(id, 'detail')).toBe(true)
      expect(isState(id, 'expanded')).toBe(false)
    })
  })

  // --- stateMap 响应性 ---
  describe('stateMap 响应性', () => {
    it('stateMap 应在状态变更后更新', () => {
      const { stateMap, expand, collapse } = useProgressiveDisclosure()

      // 初始为空 Map
      expect(stateMap.value.size).toBe(0)

      expand('test')
      expect(stateMap.value.size).toBe(1)
      expect(stateMap.value.get('test')).toBe('expanded')

      collapse('test')
      expect(stateMap.value.get('test')).toBe('collapsed')
    })

    it('collapseAll 后 stateMap 应为空 Map', () => {
      const { stateMap, expand, collapseAll } = useProgressiveDisclosure()

      expand('a')
      expand('b')
      expect(stateMap.value.size).toBe(2)

      collapseAll()
      expect(stateMap.value.size).toBe(0)
    })
  })
})
