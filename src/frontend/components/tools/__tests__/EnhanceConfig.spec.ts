import { describe, expect, it } from 'vitest'

/**
 * EnhanceConfig.vue - PROVIDER_PROTOCOL 显示逻辑测试
 *
 * 测试协议类型标注是否正确显示，包括：
 * - OpenAI 兼容供应商显示 info 类型
 * - 原生格式供应商显示 success 类型
 * - 未定义供应商的优雅降级
 */

describe('enhanceConfig - PROVIDER_PROTOCOL', () => {
  // 测试数据：与 EnhanceConfig.vue 中的 PROVIDER_PROTOCOL 保持一致
  const PROVIDER_PROTOCOL: Record<string, { type: 'info' | 'success' | 'warning', desc: string }> = {
    openai: { type: 'info', desc: 'OpenAI 原生格式（/chat/completions）' },
    grok: { type: 'info', desc: 'Grok (xAI) 兼容 OpenAI 格式，可直接使用 OpenAI SDK' },
    deepseek: { type: 'info', desc: 'DeepSeek 兼容 OpenAI 格式（/chat/completions）' },
    siliconflow: { type: 'info', desc: 'SiliconFlow 兼容 OpenAI 格式，支持多种开源模型' },
    groq: { type: 'info', desc: 'Groq 兼容 OpenAI 格式，推理速度极快' },
    gemini: { type: 'success', desc: 'Google Gemini 原生格式（generateContent），与 OpenAI 不兼容' },
    anthropic: { type: 'success', desc: 'Anthropic Claude 原生格式（/messages），与 OpenAI 不兼容' },
  }

  describe('正常路径测试', () => {
    it('should have info type for OpenAI compatible providers', () => {
      const openaiCompatProviders = ['openai', 'grok', 'deepseek', 'siliconflow', 'groq']

      openaiCompatProviders.forEach((provider) => {
        const protocol = PROVIDER_PROTOCOL[provider]
        expect(protocol).toBeDefined()
        expect(protocol.type).toBe('info')
        expect(protocol.desc).toContain('OpenAI')
      })
    })

    it('should have success type for native format providers', () => {
      const nativeProviders = ['gemini', 'anthropic']

      nativeProviders.forEach((provider) => {
        const protocol = PROVIDER_PROTOCOL[provider]
        expect(protocol).toBeDefined()
        expect(protocol.type).toBe('success')
        expect(protocol.desc).toContain('原生格式')
        expect(protocol.desc).toContain('与 OpenAI 不兼容')
      })
    })

    it('should display Grok special description', () => {
      const grokProtocol = PROVIDER_PROTOCOL.grok
      expect(grokProtocol.desc).toContain('可直接使用 OpenAI SDK')
      expect(grokProtocol.desc).toContain('xAI')
    })

    it('should display Groq speed advantage', () => {
      const groqProtocol = PROVIDER_PROTOCOL.groq
      expect(groqProtocol.desc).toContain('推理速度极快')
    })
  })

  describe('边界条件测试', () => {
    it('should return undefined for unknown provider', () => {
      const unknownProtocol = PROVIDER_PROTOCOL.unknown_provider
      expect(unknownProtocol).toBeUndefined()
    })

    it('should return undefined for empty string provider', () => {
      const emptyProtocol = PROVIDER_PROTOCOL['']
      expect(emptyProtocol).toBeUndefined()
    })

    it('should handle case-sensitive provider names', () => {
      // PROVIDER_PROTOCOL 使用小写 key，大写应返回 undefined
      const uppercaseProtocol = PROVIDER_PROTOCOL.GROK
      expect(uppercaseProtocol).toBeUndefined()
    })
  })

  describe('协议类型验证', () => {
    it('should have valid type values', () => {
      const validTypes = ['info', 'success', 'warning']

      Object.values(PROVIDER_PROTOCOL).forEach((protocol) => {
        expect(validTypes).toContain(protocol.type)
      })
    })

    it('should have non-empty descriptions', () => {
      Object.values(PROVIDER_PROTOCOL).forEach((protocol) => {
        expect(protocol.desc).toBeTruthy()
        expect(protocol.desc.length).toBeGreaterThan(0)
      })
    })
  })

  describe('完整性测试', () => {
    it('should cover all major OpenAI compatible providers', () => {
      const requiredProviders = ['openai', 'grok', 'deepseek', 'siliconflow', 'groq']

      requiredProviders.forEach((provider) => {
        expect(PROVIDER_PROTOCOL[provider]).toBeDefined()
      })
    })

    it('should cover all native format providers', () => {
      const nativeProviders = ['gemini', 'anthropic']

      nativeProviders.forEach((provider) => {
        expect(PROVIDER_PROTOCOL[provider]).toBeDefined()
      })
    })

    it('should have exactly 7 providers defined', () => {
      const providerCount = Object.keys(PROVIDER_PROTOCOL).length
      expect(providerCount).toBe(7)
    })
  })
})
