import { useDebounceFn } from '@vueuse/core'
/**
 * 记忆搜索 Composable
 * 封装搜索逻辑，支持模糊搜索和 FTS5 全文搜索
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
  /** 搜索模式（由后端返回） */
  search_mode?: string
}

export interface SearchMetadata {
  /** 搜索模式：fuzzy（模糊匹配）或 fts5（全文搜索） */
  mode: 'fuzzy' | 'fts5'
  /** 搜索耗时（毫秒） */
  duration?: number
  /** 结果总数 */
  total: number
}

/** LRU 缓存项 */
interface CacheEntry {
  results: MemorySearchResult[]
  timestamp: number
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

  /** 搜索模式状态（用于 UI 指示器） */
  const searchMode = ref<'fuzzy' | 'fts5'>('fuzzy')

  /** 是否启用 FTS5（已启用） */
  const useFts5 = ref(true) // 启用 FTS5 搜索

  /** 中文 IME 组合输入状态 */
  const isComposing = ref(false)

  /** LRU 缓存（最多 50 条，TTL 5 分钟） */
  const cache = new Map<string, CacheEntry>()
  const CACHE_MAX_SIZE = 50
  const CACHE_TTL = 5 * 60 * 1000 // 5 分钟

  /**
   * 生成缓存键
   */
  function getCacheKey(options: MemorySearchOptions): string {
    return JSON.stringify({
      query: options.query,
      category: options.category,
      domain: options.domain,
      tags: options.tags?.sort(),
      limit: options.limit,
    })
  }

  /**
   * 从缓存获取结果
   */
  function getFromCache(key: string): MemorySearchResult[] | null {
    const entry = cache.get(key)
    if (!entry)
      return null

    // 检查 TTL
    if (Date.now() - entry.timestamp > CACHE_TTL) {
      cache.delete(key)
      return null
    }

    return entry.results
  }

  /**
   * 保存到缓存（LRU 策略）
   */
  function saveToCache(key: string, results: MemorySearchResult[]) {
    // LRU 淘汰：删除最旧的条目
    if (cache.size >= CACHE_MAX_SIZE) {
      const firstKey = cache.keys().next().value
      if (firstKey)
        cache.delete(firstKey)
    }

    cache.set(key, {
      results,
      timestamp: Date.now(),
    })
  }

  /**
   * 执行搜索（内部实现，不带防抖）
   */
  async function searchInternal(options: MemorySearchOptions): Promise<boolean> {
    // 跳过 IME 组合输入中的搜索
    if (isComposing.value) {
      return false
    }

    const startTime = Date.now()
    const cacheKey = getCacheKey(options)

    try {
      // 1. 尝试从缓存获取
      const cachedResults = getFromCache(cacheKey)
      if (cachedResults) {
        results.value = cachedResults
        metadata.value = {
          mode: searchMode.value,
          duration: Date.now() - startTime,
          total: cachedResults.length,
        }
        return true
      }

      // 2. 根据 useFts5 标志选择搜索方式
      const searchResults = useFts5.value
        ? await searchFts5(options)
        : await searchFuzzy(options)

      if (searchResults) {
        results.value = searchResults
        metadata.value = {
          mode: searchMode.value,
          duration: Date.now() - startTime,
          total: searchResults.length,
        }

        // 3. 保存到缓存
        saveToCache(cacheKey, searchResults)

        return true
      }

      return false
    }
    catch (error) {
      console.error('[useMemorySearch] 搜索失败:', error)
      results.value = []
      metadata.value = {
        mode: searchMode.value,
        duration: Date.now() - startTime,
        total: 0,
      }
      return false
    }
  }

  /**
   * 执行搜索（带 300ms 防抖）
   */
  const search = useDebounceFn(searchInternal, 300)

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
   * FTS5 全文搜索
   * 调用后端 search_memories 命令，解析返回的 search_mode 字段
   */
  async function searchFts5(options: MemorySearchOptions): Promise<MemorySearchResult[] | null> {
    try {
      const result = await safeInvoke<MemorySearchResult[]>('search_memories', {
        query: options.query,
        category: options.category,
        domain: options.domain,
        tags: options.tags,
        limit: options.limit ?? 20,
      })

      if (result && result.length > 0) {
        // 解析后端返回的 search_mode 字段（根据接口契约）
        const firstResult = result[0]
        if (firstResult.search_mode) {
          // 更新搜索模式状态（用于 UI 指示器）
          searchMode.value = firstResult.search_mode === 'fts5' ? 'fts5' : 'fuzzy'
        }
      }
      else {
        // 空结果时重置为默认模式
        searchMode.value = 'fuzzy'
      }

      return result
    }
    catch (error) {
      console.error('[useMemorySearch] FTS5 搜索失败:', error)
      // 搜索失败时重置模式
      searchMode.value = 'fuzzy'
      throw error
    }
  }

  /**
   * 处理 IME 组合开始事件
   */
  function handleCompositionStart() {
    isComposing.value = true
  }

  /**
   * 处理 IME 组合结束事件
   */
  function handleCompositionEnd() {
    isComposing.value = false
  }

  /**
   * 清空搜索结果
   */
  function clearResults() {
    results.value = []
    metadata.value = {
      mode: searchMode.value,
      total: 0,
    }
  }

  /**
   * 清空缓存
   */
  function clearCache() {
    cache.clear()
  }

  return {
    // 状态
    results,
    metadata,
    loading,
    useFts5,
    searchMode,
    isComposing,

    // 方法
    search,
    clearResults,
    clearCache,
    handleCompositionStart,
    handleCompositionEnd,
  }
}
