// SC-16: 搜索状态持久化 store
// 使用 Pinia + persistedstate 插件实现搜索历史和偏好的持久化

import { defineStore } from 'pinia'

export interface SearchHistoryItem {
  query: string
  timestamp: number
  resultCount?: number
}

export interface SearchPreferences {
  /** 是否启用语义搜索 */
  enableSemanticSearch: boolean
  /** 最大结果数 */
  maxResults: number
  /** 是否显示预览 */
  showPreview: boolean
  /** 搜索范围（文件类型） */
  fileTypes: string[]
}

interface SearchState {
  /** 搜索历史（最多 50 条） */
  history: SearchHistoryItem[]
  /** 上次搜索查询 */
  lastQuery: string
  /** 用户偏好设置 */
  preferences: SearchPreferences
}

const MAX_HISTORY_ITEMS = 50

export const useSearchStore = defineStore('search', {
  state: (): SearchState => ({
    history: [],
    lastQuery: '',
    preferences: {
      enableSemanticSearch: true,
      maxResults: 20,
      showPreview: true,
      fileTypes: [],
    },
  }),

  getters: {
    /** 获取最近的搜索记录 */
    recentSearches: (state): SearchHistoryItem[] => {
      return state.history.slice(0, 10)
    },

    /** 获取搜索历史中的唯一查询词 */
    uniqueQueries: (state): string[] => {
      const seen = new Set<string>()
      return state.history
        .filter((item) => {
          if (seen.has(item.query))
            return false
          seen.add(item.query)
          return true
        })
        .map(item => item.query)
    },
  },

  actions: {
    /** 添加搜索记录 */
    addSearch(query: string, resultCount?: number) {
      const trimmedQuery = query.trim()
      if (!trimmedQuery)
        return

      // 移除重复的查询
      this.history = this.history.filter(item => item.query !== trimmedQuery)

      // 添加到历史开头
      this.history.unshift({
        query: trimmedQuery,
        timestamp: Date.now(),
        resultCount,
      })

      // 限制历史数量
      if (this.history.length > MAX_HISTORY_ITEMS) {
        this.history = this.history.slice(0, MAX_HISTORY_ITEMS)
      }

      this.lastQuery = trimmedQuery
    },

    /** 清除搜索历史 */
    clearHistory() {
      this.history = []
      this.lastQuery = ''
    },

    /** 删除单条搜索记录 */
    removeSearch(query: string) {
      this.history = this.history.filter(item => item.query !== query)
    },

    /** 更新偏好设置 */
    updatePreferences(preferences: Partial<SearchPreferences>) {
      this.preferences = { ...this.preferences, ...preferences }
    },

    /** 重置偏好设置 */
    resetPreferences() {
      this.preferences = {
        enableSemanticSearch: true,
        maxResults: 20,
        showPreview: true,
        fileTypes: [],
      }
    },
  },

  // 持久化配置
  persist: {
    key: 'sanshu-search',
    storage: localStorage,
    pick: ['history', 'lastQuery', 'preferences'],
  },
})
