/**
 * 记忆搜索 Composable
 * 封装搜索逻辑，支持模糊搜索和未来的 FTS5 搜索
 */
import { ref } from 'vue'
import { useSafeInvoke } from './useSafeInvoke'

export interface MemorySearchOptions {
  /** 搜索关键词 */
  query: string
  /** 分类过滤 */
  category?: string
  /** 域过滤 */
  domain?: string
  /** 标签过滤 */
  tags?: string[]
  /** 结果数量限制 */
  limit?: number
}

export interface MemorySearchResult {
  id: string
  content: string
  category: string
  created_at: string
  tags?: string[]
  domain?: string
  /** 搜索相关度（0-1） */
  relevance?: number
  /** 高亮片段（HTML） */
  highlighted_snippet?: string
}

export interface SearchMetadata {
  /** 搜索模式：fuzzy（模糊匹配）或 fts5（全文搜索） */
  mode: 'fuzzy' | 'fts5'
  /** 搜索耗时（毫秒） */
  duration?: number
  /** 结果总数 */
  total: number
}

/**
 * 记忆搜索 Hook
 */
export function useMemorySearch() {
  const { safeInvoke, loading } = useSafeInvoke()

  /** 搜索结果 */
  const results = ref<MemorySearchResult[]>([])

  /** 搜索元数据 */
  const metadata = ref<SearchMetadata>({
    mode: 'fuzzy',
    total: 0,
  })

  /** 是否启用 FTS5（预留标志，当前始终为 false） */
  const useFts5 = ref(false)

  /**
   * 执行搜索
   */
  async function search(options: MemorySearchOptions): Promise<boolean> {
    const startTime = Date.now()

    try {
      // 当前版本：使用模糊搜索
      // TODO: 未来集成 FTS5 后，根据 useFts5 标志选择搜索方式
      const searchResults = await searchFuzzy(options)

      if (searchResults) {
        results.value = searchResults
        metadata.value = {
          mode: 'fuzzy',
          duration: Date.now() - startTime,
          total: searchResults.length,
        }
        return true
      }

      return false
    } catch (error) {
      console.error('[useMemorySearch] 搜索失败:', error)
      results.value = []
      metadata.value = {
        mode: 'fuzzy',
        duration: Date.now() - startTime,
        total: 0,
      }
      return false
    }
  }

  /**
   * 模糊搜索（当前实现）
   */
  async function searchFuzzy(options: MemorySearchOptions): Promise<MemorySearchResult[] | null> {
    const result = await safeInvoke<MemorySearchResult[]>('search_memories', {
      projectPath: options.query, // 临时：需要传递 projectPath
      query: options.query,
      category: options.category,
      domain: options.domain,
      tags: options.tags,
    })

    return result
  }

  /**
   * FTS5 搜索（预留接口）
   *
   * TODO: 实现 FTS5 搜索逻辑
   * 1. 调用 invoke('search_memories_fts5', { query, limit })
   * 2. 处理高亮片段
   * 3. 失败时降级到模糊搜索
   */
  async function searchFts5(options: MemorySearchOptions): Promise<MemorySearchResult[] | null> {
    // 预留实现
    console.warn('[useMemorySearch] FTS5 搜索尚未实现，降级到模糊搜索')
    return searchFuzzy(options)
  }

  /**
   * 清空搜索结果
   */
  function clearResults() {
    results.value = []
    metadata.value = {
      mode: 'fuzzy',
      total: 0,
    }
  }

  return {
    // 状态
    results,
    metadata,
    loading,
    useFts5,

    // 方法
    search,
    clearResults,
  }
}
