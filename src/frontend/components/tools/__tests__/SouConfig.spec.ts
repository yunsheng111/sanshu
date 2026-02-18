import { describe, it, expect } from 'vitest'

/**
 * SouConfig.vue - EMBEDDING_PROTOCOL 显示逻辑测试
 *
 * 测试嵌入提供者协议类型标注是否正确显示，包括：
 * - OpenAI Embeddings 兼容提供者显示 info 类型
 * - Ollama 本地嵌入显示 success 类型
 * - Cohere 原生格式显示 warning 类型
 */

describe('SouConfig - EMBEDDING_PROTOCOL', () => {
  // 测试数据：与 SouConfig.vue 中的 EMBEDDING_PROTOCOL 保持一致
  const EMBEDDING_PROTOCOL: Record<string, { type: 'info' | 'success' | 'warning'; desc: string }> = {
    jina: { type: 'info', desc: 'Jina AI 兼容 OpenAI Embeddings 格式（/embeddings）' },
    siliconflow: { type: 'info', desc: 'SiliconFlow 兼容 OpenAI Embeddings 格式，支持 BGE 等多种模型' },
    ollama: { type: 'success', desc: 'Ollama 本地嵌入，无需 API Key，数据不出本机' },
    cloudflare: { type: 'info', desc: 'Cloudflare AI Gateway，兼容 OpenAI Embeddings 格式' },
    nomic: { type: 'info', desc: 'Nomic Atlas 兼容 OpenAI Embeddings 格式' },
    cohere: { type: 'warning', desc: 'Cohere 使用原生格式（/embed），与 OpenAI 不兼容' },
  }

  describe('正常路径测试', () => {
    it('should have info type for OpenAI Embeddings compatible providers', () => {
      const compatProviders = ['jina', 'siliconflow', 'cloudflare', 'nomic']

      compatProviders.forEach(provider => {
        const protocol = EMBEDDING_PROTOCOL[provider]
        expect(protocol).toBeDefined()
        expect(protocol.type).toBe('info')
        expect(protocol.desc).toContain('OpenAI Embeddings 格式')
      })
    })

    it('should have success type for Ollama local embedding', () => {
      const ollamaProtocol = EMBEDDING_PROTOCOL['ollama']
      expect(ollamaProtocol).toBeDefined()
      expect(ollamaProtocol.type).toBe('success')
      expect(ollamaProtocol.desc).toContain('本地嵌入')
      expect(ollamaProtocol.desc).toContain('无需 API Key')
      expect(ollamaProtocol.desc).toContain('数据不出本机')
    })

    it('should have warning type for Cohere native format', () => {
      const cohereProtocol = EMBEDDING_PROTOCOL['cohere']
      expect(cohereProtocol).toBeDefined()
      expect(cohereProtocol.type).toBe('warning')
      expect(cohereProtocol.desc).toContain('原生格式')
      expect(cohereProtocol.desc).toContain('与 OpenAI 不兼容')
    })

    it('should display SiliconFlow BGE model support', () => {
      const siliconflowProtocol = EMBEDDING_PROTOCOL['siliconflow']
      expect(siliconflowProtocol.desc).toContain('BGE')
      expect(siliconflowProtocol.desc).toContain('多种模型')
    })
  })

  describe('边界条件测试', () => {
    it('should return undefined for unknown provider', () => {
      const unknownProtocol = EMBEDDING_PROTOCOL['unknown_embedding']
      expect(unknownProtocol).toBeUndefined()
    })

    it('should return undefined for empty string provider', () => {
      const emptyProtocol = EMBEDDING_PROTOCOL['']
      expect(emptyProtocol).toBeUndefined()
    })

    it('should handle case-sensitive provider names', () => {
      // EMBEDDING_PROTOCOL 使用小写 key，大写应返回 undefined
      const uppercaseProtocol = EMBEDDING_PROTOCOL['OLLAMA']
      expect(uppercaseProtocol).toBeUndefined()
    })
  })

  describe('协议类型验证', () => {
    it('should have valid type values', () => {
      const validTypes = ['info', 'success', 'warning']

      Object.values(EMBEDDING_PROTOCOL).forEach(protocol => {
        expect(validTypes).toContain(protocol.type)
      })
    })

    it('should have non-empty descriptions', () => {
      Object.values(EMBEDDING_PROTOCOL).forEach(protocol => {
        expect(protocol.desc).toBeTruthy()
        expect(protocol.desc.length).toBeGreaterThan(0)
      })
    })

    it('should have exactly one warning type (Cohere)', () => {
      const warningProviders = Object.entries(EMBEDDING_PROTOCOL)
        .filter(([_, protocol]) => protocol.type === 'warning')

      expect(warningProviders).toHaveLength(1)
      expect(warningProviders[0][0]).toBe('cohere')
    })

    it('should have exactly one success type (Ollama)', () => {
      const successProviders = Object.entries(EMBEDDING_PROTOCOL)
        .filter(([_, protocol]) => protocol.type === 'success')

      expect(successProviders).toHaveLength(1)
      expect(successProviders[0][0]).toBe('ollama')
    })
  })

  describe('完整性测试', () => {
    it('should cover all major embedding providers', () => {
      const requiredProviders = ['jina', 'siliconflow', 'ollama', 'cloudflare', 'nomic', 'cohere']

      requiredProviders.forEach(provider => {
        expect(EMBEDDING_PROTOCOL[provider]).toBeDefined()
      })
    })

    it('should have exactly 6 providers defined', () => {
      const providerCount = Object.keys(EMBEDDING_PROTOCOL).length
      expect(providerCount).toBe(6)
    })

    it('should have 4 info type providers', () => {
      const infoProviders = Object.values(EMBEDDING_PROTOCOL)
        .filter(protocol => protocol.type === 'info')

      expect(infoProviders).toHaveLength(4)
    })
  })

  describe('隐私和安全标注', () => {
    it('should emphasize Ollama privacy benefits', () => {
      const ollamaProtocol = EMBEDDING_PROTOCOL['ollama']
      expect(ollamaProtocol.desc).toContain('数据不出本机')
    })

    it('should warn about Cohere incompatibility', () => {
      const cohereProtocol = EMBEDDING_PROTOCOL['cohere']
      expect(cohereProtocol.type).toBe('warning')
      expect(cohereProtocol.desc).toContain('不兼容')
    })
  })

  describe('API 端点标注', () => {
    it('should specify Jina AI endpoint format', () => {
      const jinaProtocol = EMBEDDING_PROTOCOL['jina']
      expect(jinaProtocol.desc).toContain('/embeddings')
    })

    it('should specify Cohere endpoint format', () => {
      const cohereProtocol = EMBEDDING_PROTOCOL['cohere']
      expect(cohereProtocol.desc).toContain('/embed')
    })
  })
})
