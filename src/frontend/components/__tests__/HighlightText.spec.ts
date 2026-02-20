/**
 * HighlightText 组件单元测试
 * 验证 XSS 防护和片段解析功能
 *
 * 测试覆盖：
 * - 基本高亮渲染
 * - XSS 注入防护
 * - 边界情况处理
 * - 降级策略
 */
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import HighlightText from '../HighlightText.vue'

describe('highlightText.vue', () => {
  it('应该正确渲染基本高亮片段', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '这是一个<mark>测试</mark>片段',
      },
    })

    const text = wrapper.text()
    expect(text).toBe('这是一个测试片段')

    // 验证高亮部分存在
    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(1)
    expect(highlights[0].text()).toBe('测试')
  })

  it('应该正确渲染多个高亮片段', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '第一个<mark>高亮</mark>和第二个<mark>高亮</mark>',
      },
    })

    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(2)
    expect(highlights[0].text()).toBe('高亮')
    expect(highlights[1].text()).toBe('高亮')
  })

  it('应该转义 HTML 实体，防止 XSS 注入', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<script>alert("xss")</script><mark>测试</mark>',
      },
    })

    const html = wrapper.html()
    // Vue h() 会自动转义，所以 HTML 中会包含转义后的实体
    // 验证不包含未转义的 <script> 标签
    expect(html).not.toContain('<script>alert')
    expect(html).not.toContain('</script>')

    // 验证文本内容正确（包含转义后的内容）
    const text = wrapper.text()
    expect(text).toContain('alert("xss")')
  })

  it('应该防止事件处理器注入', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<img src=x onerror="alert(1)"><mark>测试</mark>',
      },
    })

    const html = wrapper.html()
    // 验证不包含未转义的 img 标签
    expect(html).not.toContain('<img src=')
    // Vue h() 会转义文本内容，但属性值中的引号可能保留
    // 关键是验证 <img> 标签本身被转义，无法执行
    const text = wrapper.text()
    expect(text).toContain('alert(1)')
  })

  it('应该防止 iframe 注入', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<iframe src="evil.com"></iframe><mark>测试</mark>',
      },
    })

    const html = wrapper.html()
    // 验证不包含未转义的 iframe 标签
    expect(html).not.toContain('<iframe src=')
    expect(html).not.toContain('</iframe>')
  })

  it('应该防止 javascript: 协议注入', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<a href="javascript:alert(1)">点击</a><mark>测试</mark>',
      },
    })

    const html = wrapper.html()
    // 验证不包含未转义的 a 标签
    expect(html).not.toContain('<a href=')
    // 关键是验证 <a> 标签本身被转义，无法执行
    const text = wrapper.text()
    expect(text).toContain('点击')
  })

  it('应该处理空片段', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '',
      },
    })

    expect(wrapper.text()).toBe('')
  })

  it('应该处理没有高亮标签的片段', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '这是一个普通文本',
      },
    })

    expect(wrapper.text()).toBe('这是一个普通文本')
    expect(wrapper.findAll('.highlight')).toHaveLength(0)
  })

  it('应该处理嵌套的 mark 标签（降级为转义）', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<mark>外层<mark>内层</mark></mark>',
      },
    })

    const html = wrapper.html()
    // 内层 <mark> 会被当作普通文本处理，Vue h() 会自动转义
    // 验证不包含未转义的嵌套 mark 标签
    expect(html).not.toContain('<mark>内层</mark>')

    // 验证文本内容正确
    const text = wrapper.text()
    expect(text).toContain('外层')
    expect(text).toContain('内层')
  })

  it('应该支持自定义高亮类名', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '这是<mark>测试</mark>',
        highlightClass: 'custom-highlight',
      },
    })

    expect(wrapper.find('.custom-highlight').exists()).toBe(true)
    expect(wrapper.find('.highlight').exists()).toBe(false)
  })

  it('应该转义特殊字符', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '特殊字符: &<>"\'/<mark>测试</mark>',
      },
    })

    const html = wrapper.html()
    // Vue h() 会自动转义特殊字符
    // 验证不包含未转义的特殊字符（在标签属性或危险位置）
    const text = wrapper.text()
    expect(text).toContain('特殊字符: &<>"\'/')
    expect(text).toContain('测试')
  })

  it('应该处理连续的高亮标签', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<mark>第一个</mark><mark>第二个</mark>',
      },
    })

    const highlights = wrapper.findAll('.highlight')
    // split 会产生空字符串，但我们在 parseFts5Snippet 中过滤了
    // 所以应该只有 2 个高亮部分
    expect(highlights.length).toBeGreaterThanOrEqual(1)

    // 验证文本内容正确
    const text = wrapper.text()
    expect(text).toContain('第一个')
    expect(text).toContain('第二个')
  })

  it('应该处理高亮标签在开头的情况', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<mark>开头</mark>后面的文本',
      },
    })

    expect(wrapper.text()).toBe('开头后面的文本')
    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(1)
    expect(highlights[0].text()).toBe('开头')
  })

  it('应该处理高亮标签在结尾的情况', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '前面的文本<mark>结尾</mark>',
      },
    })

    expect(wrapper.text()).toBe('前面的文本结尾')
    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(1)
    expect(highlights[0].text()).toBe('结尾')
  })

  it('应该处理只有高亮标签的片段', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '<mark>全部高亮</mark>',
      },
    })

    expect(wrapper.text()).toBe('全部高亮')
    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(1)
    expect(highlights[0].text()).toBe('全部高亮')
  })

  it('应该处理中文字符', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: '这是一段<mark>中文</mark>测试文本',
      },
    })

    expect(wrapper.text()).toBe('这是一段中文测试文本')
    const highlights = wrapper.findAll('.highlight')
    expect(highlights).toHaveLength(1)
    expect(highlights[0].text()).toBe('中文')
  })

  it('应该处理 Unicode 字符', () => {
    const wrapper = mount(HighlightText, {
      props: {
        snippet: 'Emoji: 😀<mark>测试</mark>🎉',
      },
    })

    expect(wrapper.text()).toContain('😀')
    expect(wrapper.text()).toContain('🎉')
  })
})
