<script setup lang="ts">
/**
 * 提示词增强配置面板
 * 支持多供应商：Ollama / OpenAI / Grok / DeepSeek / SiliconFlow / Groq / Gemini / Anthropic / 规则引擎
 */
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
import ConfigSection from '../common/ConfigSection.vue'

const props = defineProps<{
  active: boolean
  projectRootPath?: string | null
}>()

const message = useMessage()

// 配置状态
const config = ref({
  provider: 'ollama',
  ollama_url: 'http://localhost:11434',
  ollama_model: 'qwen2.5-coder:7b',
  base_url: '',
  api_key: '',
  model: '',
})

const loadingConfig = ref(false)
const savingConfig = ref(false)
const testingOllama = ref(false)
const ollamaAvailable = ref<boolean | null>(null)
const historyCount = ref<number | null>(null)
const historyLoading = ref(false)

const hasProject = computed(() => !!props.projectRootPath)

// 供应商默认配置
const PROVIDER_DEFAULTS: Record<string, { base_url: string; model: string; label: string }> = {
  ollama: { base_url: 'http://localhost:11434', model: 'qwen2.5-coder:7b', label: 'Ollama 本地' },
  openai: { base_url: 'https://api.openai.com/v1', model: 'gpt-4o-mini', label: 'OpenAI' },
  grok: { base_url: 'https://api.x.ai/v1', model: 'grok-3-mini', label: 'Grok (xAI)' },
  deepseek: { base_url: 'https://api.deepseek.com/v1', model: 'deepseek-chat', label: 'DeepSeek' },
  siliconflow: { base_url: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-Coder-7B-Instruct', label: 'SiliconFlow' },
  groq: { base_url: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile', label: 'Groq' },
  gemini: { base_url: 'https://generativelanguage.googleapis.com/v1beta', model: 'gemini-2.0-flash', label: 'Google Gemini' },
  anthropic: { base_url: 'https://api.anthropic.com/v1', model: 'claude-3-5-haiku-20241022', label: 'Anthropic Claude' },
  rule_engine: { base_url: '', model: '', label: '规则引擎（无需 API）' },
}

const providerOptions = Object.entries(PROVIDER_DEFAULTS).map(([value, { label }]) => ({ label, value }))

// 各供应商协议类型说明
const PROVIDER_PROTOCOL: Record<string, { type: 'info' | 'success' | 'warning'; desc: string }> = {
  openai: { type: 'info', desc: 'OpenAI 原生格式（/chat/completions）' },
  grok: { type: 'info', desc: 'Grok (xAI) 兼容 OpenAI 格式，可直接使用 OpenAI SDK' },
  deepseek: { type: 'info', desc: 'DeepSeek 兼容 OpenAI 格式（/chat/completions）' },
  siliconflow: { type: 'info', desc: 'SiliconFlow 兼容 OpenAI 格式，支持多种开源模型' },
  groq: { type: 'info', desc: 'Groq 兼容 OpenAI 格式，推理速度极快' },
  gemini: { type: 'success', desc: 'Google Gemini 原生格式（generateContent），与 OpenAI 不兼容' },
  anthropic: { type: 'success', desc: 'Anthropic Claude 原生格式（/messages），与 OpenAI 不兼容' },
}

const isOllamaProvider = computed(() => config.value.provider === 'ollama')
const isRuleEngine = computed(() => config.value.provider === 'rule_engine')
const isCloudProvider = computed(() => !isOllamaProvider.value && !isRuleEngine.value)

function onProviderChange(val: string) {
  const defaults = PROVIDER_DEFAULTS[val]
  if (!defaults)
    return
  // 自动填充默认值（仅当字段为空或匹配其他供应商默认值时）
  const knownUrls = Object.values(PROVIDER_DEFAULTS).map(d => d.base_url)
  const knownModels = Object.values(PROVIDER_DEFAULTS).map(d => d.model)
  if (!config.value.base_url || knownUrls.includes(config.value.base_url))
    config.value.base_url = defaults.base_url
  if (!config.value.model || knownModels.includes(config.value.model))
    config.value.model = defaults.model
}

async function loadConfig() {
  loadingConfig.value = true
  try {
    const res = await invoke('get_enhance_config') as any
    config.value = {
      provider: res.provider || 'ollama',
      ollama_url: res.ollama_url || 'http://localhost:11434',
      ollama_model: res.ollama_model || 'qwen2.5-coder:7b',
      base_url: res.base_url || '',
      api_key: res.api_key || '',
      model: res.model || '',
    }
  }
  catch (err) {
    message.error(`加载配置失败: ${err}`)
  }
  finally {
    loadingConfig.value = false
  }
}

async function saveConfig() {
  savingConfig.value = true
  try {
    await invoke('save_enhance_config', {
      configDto: {
        provider: config.value.provider,
        ollama_url: config.value.ollama_url,
        ollama_model: config.value.ollama_model,
        base_url: config.value.base_url,
        api_key: config.value.api_key,
        model: config.value.model,
      },
    })
    message.success('配置已保存')
  }
  catch (err) {
    message.error(`保存失败: ${err}`)
  }
  finally {
    savingConfig.value = false
  }
}

async function testOllamaConnection() {
  testingOllama.value = true
  ollamaAvailable.value = null
  try {
    const response = await fetch(`${config.value.ollama_url}/api/tags`)
    if (response.ok) {
      ollamaAvailable.value = true
      message.success('Ollama 连接成功')
    }
    else {
      ollamaAvailable.value = false
      message.error('Ollama 响应异常')
    }
  }
  catch {
    ollamaAvailable.value = false
    message.error('无法连接到 Ollama，请确认服务已启动')
  }
  finally {
    testingOllama.value = false
  }
}

async function loadHistoryCount() {
  if (!hasProject.value) {
    historyCount.value = null
    return
  }
  historyLoading.value = true
  try {
    const res = await invoke('get_chat_history', {
      projectRootPath: props.projectRootPath,
      count: 20,
    }) as any[]
    historyCount.value = res.length
  }
  catch {
    historyCount.value = null
  }
  finally {
    historyLoading.value = false
  }
}

async function clearHistory() {
  if (!hasProject.value) {
    message.warning('未检测到项目路径')
    return
  }
  try {
    await invoke('clear_chat_history', { projectRootPath: props.projectRootPath })
    historyCount.value = 0
    message.success('历史已清空')
  }
  catch (err) {
    message.error(`清空失败: ${err}`)
  }
}

watch(() => props.active, (active) => {
  if (active) {
    loadConfig()
    loadHistoryCount()
  }
})

watch(() => props.projectRootPath, () => {
  if (props.active)
    loadHistoryCount()
})

onMounted(() => {
  if (props.active) {
    loadConfig()
    loadHistoryCount()
  }
})
</script>

<template>
  <div class="enhance-config">
    <n-scrollbar class="config-scrollbar">
      <n-space vertical size="large" class="config-content">

        <!-- 供应商选择 -->
        <ConfigSection title="供应商选择" description="选择提示词增强使用的 AI 服务">
          <n-select
            v-model:value="config.provider"
            :options="providerOptions"
            :disabled="loadingConfig"
            @update:value="onProviderChange"
          />
          <div v-if="isRuleEngine" class="mt-3">
            <n-alert type="warning" :bordered="false">
              <template #icon>
                <div class="i-carbon-warning" />
              </template>
              规则引擎是兜底方案，增强效果有限。建议优先使用 Ollama 或云端 API。
            </n-alert>
          </div>
        </ConfigSection>

        <!-- Ollama 配置 -->
        <ConfigSection
          v-if="isOllamaProvider"
          title="Ollama 本地配置"
          description="使用本地 Ollama 服务进行提示词增强"
        >
          <n-form-item label="Ollama 端点">
            <n-input
              v-model:value="config.ollama_url"
              :disabled="loadingConfig"
              placeholder="http://localhost:11434"
              clearable
            />
          </n-form-item>

          <n-form-item label="模型名称">
            <n-input
              v-model:value="config.ollama_model"
              :disabled="loadingConfig"
              placeholder="qwen2.5-coder:7b"
              clearable
            />
            <template #feedback>
              <span class="form-feedback">推荐：qwen2.5-coder:7b、deepseek-coder:6.7b</span>
            </template>
          </n-form-item>

          <div class="flex items-center gap-3 mt-3">
            <n-button
              size="small"
              :loading="testingOllama"
              @click="testOllamaConnection"
            >
              <template #icon>
                <div class="i-carbon-connection-signal" />
              </template>
              测试连接
            </n-button>
            <n-tag v-if="ollamaAvailable === true" type="success" size="small" round>
              连接正常
            </n-tag>
            <n-tag v-else-if="ollamaAvailable === false" type="error" size="small" round>
              连接失败
            </n-tag>
          </div>
        </ConfigSection>

        <!-- 云端 API 配置 -->
        <ConfigSection
          v-if="isCloudProvider"
          :title="`${PROVIDER_DEFAULTS[config.provider]?.label ?? config.provider} 配置`"
          description="配置 API 端点、密钥和模型"
        >
          <!-- 协议类型标注 -->
          <n-alert
            :type="PROVIDER_PROTOCOL[config.provider]?.type ?? 'default'"
            :bordered="false"
            class="mb-3"
          >
            <template #icon>
              <div class="i-carbon-information" />
            </template>
            {{ PROVIDER_PROTOCOL[config.provider]?.desc ?? '使用 OpenAI 兼容格式' }}
          </n-alert>

          <n-form-item label="API 端点">
            <n-input
              v-model:value="config.base_url"
              :disabled="loadingConfig"
              placeholder="https://api.example.com/v1"
              clearable
            />
          </n-form-item>

          <n-form-item label="API Key">
            <n-input
              v-model:value="config.api_key"
              :disabled="loadingConfig"
              type="password"
              show-password-on="click"
              placeholder="sk-xxx 或对应密钥"
              clearable
            />
          </n-form-item>

          <n-form-item label="模型名称">
            <n-input
              v-model:value="config.model"
              :disabled="loadingConfig"
              :placeholder="PROVIDER_DEFAULTS[config.provider]?.model ?? ''"
              clearable
            />
          </n-form-item>
        </ConfigSection>

        <!-- 保存按钮 -->
        <div class="flex justify-end">
          <n-button
            type="primary"
            :loading="savingConfig"
            @click="saveConfig"
          >
            <template #icon>
              <div class="i-carbon-save" />
            </template>
            保存配置
          </n-button>
        </div>

        <!-- 历史管理 -->
        <ConfigSection title="增强历史管理" description="仅保存文本摘要，不包含图片原始数据">
          <div class="flex items-center justify-between">
            <div class="text-sm text-on-surface-secondary">
              <div>当前项目增强历史条数</div>
              <div class="text-xs opacity-70 mt-1">
                <span v-if="historyLoading">加载中...</span>
                <span v-else>{{ historyCount ?? '--' }}</span>
              </div>
            </div>
            <n-button
              type="warning"
              size="small"
              :disabled="!hasProject || historyLoading"
              @click="clearHistory"
            >
              <template #icon>
                <div class="i-carbon-trash-can" />
              </template>
              清空历史
            </n-button>
          </div>

          <n-alert v-if="!hasProject" type="warning" :bordered="false" class="mt-3">
            <template #icon>
              <div class="i-carbon-warning" />
            </template>
            未检测到项目路径，历史统计与清理不可用。
          </n-alert>
        </ConfigSection>
      </n-space>
    </n-scrollbar>
  </div>
</template>

<style scoped>
.enhance-config {
  height: 100%;
}

.config-scrollbar {
  max-height: 65vh;
}

.config-content {
  padding-right: 8px;
  padding-bottom: 16px;
}

.form-feedback {
  font-size: 11px;
  color: var(--color-on-surface-muted, #9ca3af);
}
</style>
