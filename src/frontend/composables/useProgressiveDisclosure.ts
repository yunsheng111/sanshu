/**
 * 渐进式披露状态管理
 * 管理三态展示：collapsed（折叠）、expanded（展开）、detail（详情）
 * 用于记忆卡片的渐进式内容展示
 */
import { ref } from 'vue'

export type DisclosureState = 'collapsed' | 'expanded' | 'detail'

/**
 * 状态流转顺序：collapsed <-> expanded <-> detail
 */
const STATE_ORDER: DisclosureState[] = ['collapsed', 'expanded', 'detail']

export function useProgressiveDisclosure() {
  /** 每个条目的展示状态，key 为条目 ID */
  const stateMap = ref<Map<string, DisclosureState>>(new Map())

  /** 获取指定条目的当前状态，默认为 collapsed */
  function getState(id: string): DisclosureState {
    return stateMap.value.get(id) ?? 'collapsed'
  }

  /** 设置指定条目的状态 */
  function setState(id: string, state: DisclosureState) {
    const newMap = new Map(stateMap.value)
    newMap.set(id, state)
    stateMap.value = newMap
  }

  /** 切换到下一个状态（循环：collapsed -> expanded -> detail -> collapsed） */
  function toggleNext(id: string) {
    const current = getState(id)
    const currentIndex = STATE_ORDER.indexOf(current)
    const nextIndex = (currentIndex + 1) % STATE_ORDER.length
    setState(id, STATE_ORDER[nextIndex])
  }

  /** 展开指定条目（从 collapsed 到 expanded） */
  function expand(id: string) {
    setState(id, 'expanded')
  }

  /** 折叠指定条目（回到 collapsed） */
  function collapse(id: string) {
    setState(id, 'collapsed')
  }

  /** 展开到详情态 */
  function showDetail(id: string) {
    setState(id, 'detail')
  }

  /** 折叠所有条目 */
  function collapseAll() {
    stateMap.value = new Map()
  }

  /** 判断指定条目是否处于某个状态 */
  function isState(id: string, state: DisclosureState): boolean {
    return getState(id) === state
  }

  return {
    stateMap,
    getState,
    setState,
    toggleNext,
    expand,
    collapse,
    showDetail,
    collapseAll,
    isState,
  }
}
