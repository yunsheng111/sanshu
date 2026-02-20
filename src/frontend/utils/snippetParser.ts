/**
 * FTS5 片段解析器
 * 安全解析 FTS5 返回的高亮片段，防止 XSS 注入
 *
 * 约束：SC-18 - 禁止使用 v-html，必须使用自定义解析器
 */

/**
 * 片段部分接口
 */
export interface SnippetPart {
  /** 文本内容（原始文本，由 Vue h() 函数自动转义） */
  text: string
  /** 是否为匹配高亮部分 */
  isMatch: boolean
}

/**
 * 解析 FTS5 片段
 * 提取 <mark>...</mark> 标签，移除标签但保留文本内容
 *
 * 注意：不需要手动转义 HTML，Vue 的 h() 函数会自动转义文本内容，防止 XSS
 *
 * 输入示例：
 * - "这是一个<mark>测试</mark>片段"
 * - "包含<script>alert('xss')</script>的<mark>恶意</mark>内容"
 *
 * 输出：
 * - [{ text: "这是一个", isMatch: false }, { text: "测试", isMatch: true }, { text: "片段", isMatch: false }]
 * - [{ text: "包含<script>alert('xss')</script>的", isMatch: false }, { text: "恶意", isMatch: true }, { text: "内容", isMatch: false }]
 *   （Vue h() 会自动转义 <script> 标签）
 *
 * @param snippet - FTS5 返回的高亮片段（包含 <mark> 标签）
 * @returns 解析后的片段数组
 */
export function parseFts5Snippet(snippet: string): SnippetPart[] {
  if (!snippet || typeof snippet !== 'string') {
    return []
  }

  try {
    // 使用正则提取 <mark>...</mark> 标签
    // 注意：计划文件中提到使用 <mark> 标签，而非 <b> 标签
    const regex = /<mark>(.*?)<\/mark>/g
    const parts = snippet.split(regex)

    // split 后的数组：[非匹配, 匹配, 非匹配, 匹配, ...]
    // 原始索引：偶数索引为非匹配部分，奇数索引为匹配部分
    const result: SnippetPart[] = []

    parts.forEach((text, originalIndex) => {
      // 跳过空字符串
      if (text.length === 0)
        return

      result.push({
        text, // 保留原始文本，由 Vue h() 自动转义
        isMatch: originalIndex % 2 === 1, // 原始奇数索引为匹配部分
      })
    })

    return result
  }
  catch (error) {
    // 解析失败时降级：返回完整文本
    console.error('[snippetParser] 解析失败:', error)
    return [
      {
        text: snippet,
        isMatch: false,
      },
    ]
  }
}

/**
 * 验证片段是否包含危险模式（用于测试）
 * 注意：Vue h() 会自动转义，所以这里检查的是原始文本中的危险模式
 *
 * @param parts - 解析后的片段数组
 * @returns 是否包含危险模式
 */
export function hasDangerousPatterns(parts: SnippetPart[]): boolean {
  const dangerousPatterns = [
    /<script/i,
    /<iframe/i,
    /javascript:/i,
    /on\w+=/i, // onclick, onerror 等事件处理器
  ]

  return parts.some(part =>
    dangerousPatterns.some(pattern => pattern.test(part.text)),
  )
}
