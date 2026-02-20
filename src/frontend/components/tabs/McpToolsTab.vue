<script setup lang="ts">
import { useMessage } from 'naive-ui'
import { computed, defineAsyncComponent, onMounted, ref } from 'vue'
import { useMcpToolsReactive } from '../../composables/useMcpTools'

const props = defineProps<{
  projectRootPath?: string | null
}>()
// 异步加载配置组件
const SouConfig = defineAsyncComponent(() => import('../tools/SouConfig.vue'))
const Context7Config = defineAsyncComponent(() => import('../tools/Context7Config.vue'))
const IconWorkshop = defineAsyncComponent(() => import('../tools/IconWorkshop/IconWorkshop.vue'))
const EnhanceConfig = defineAsyncComponent(() => import('../tools/EnhanceConfig.vue'))
const MemoryManager = defineAsyncComponent(() => import('../tools/MemoryManager.vue'))

// 全局 MCP 工具状态
const {
  mcpTools,
  loading,
  loadMcpTools,
  toggleTool: globalToggleTool,
  toolStats,
} = useMcpToolsReactive()

const message = useMessage()
const needsReconnect = ref(false)
const showToolConfigModal = ref(false)
const currentToolId = ref('')

// 计算属性：当前工具名称
const currentToolName = computed(() => {
  const tool = mcpTools.value.find(t => t.id === currentToolId.value)
  return tool ? tool.name : ''
})

// 切换工具启用状态
async function toggleTool(toolId: string) {
  try {
    const result = await globalToggleTool(toolId)
    if (result.needsReconnect) {
      needsReconnect.value = true
    }
    message.warning('MCP工具配置已更新，请在MCP客户端中重连服务')
  }
  catch (err) {
    message.error(`更新MCP工具状态失败: ${err}`)
  }
}

// 打开工具配置弹窗
function openToolConfig(toolId: string) {
  currentToolId.value = toolId
  showToolConfigModal.value = true
}

// 组件挂载时加载工具列表
onMounted(async () => {
  try {
    await loadMcpTools()
  }
  catch (err) {
    message.error(`加载MCP工具配置失败: ${err}`)
  }
})
</script>

<template>
  <div class="max-w-4xl mx-auto tab-content p-4">
    <n-space vertical size="large">
      <!-- 重连提示 -->
      <transition name="slide-down">
        <n-alert
          v-if="needsReconnect"
          title="需要重连MCP服务"
          type="warning"
          closable
          class="reconnect-alert"
          @close="needsReconnect = false"
        >
          <template #icon>
            <div class="i-carbon-connection-signal text-lg" />
          </template>
          MCP工具配置已更改，请在您的MCP客户端中重新连接三术服务以使更改生效。
        </n-alert>
      </transition>

      <!-- 加载状态 - 骨架屏 -->
      <div v-if="loading" class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div v-for="i in 4" :key="i" class="tool-card-skeleton">
          <div class="skeleton-header">
            <div class="skeleton-icon" />
            <div class="skeleton-content">
              <div class="skeleton-line w-32" />
              <div class="skeleton-line w-48" />
            </div>
          </div>
          <div class="skeleton-footer">
            <div class="skeleton-line w-16" />
            <div class="skeleton-switch" />
          </div>
        </div>
      </div>

      <!-- 工具卡片网格 -->
      <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div
          v-for="tool in mcpTools"
          :key="tool.id"
          class="tool-card group"
          :class="{ 'tool-card--disabled': !tool.enabled }"
        >
          <!-- 顶部装饰线 -->
          <div class="card-top-border" />

          <div class="card-content">
            <!-- 图标区域 -->
            <div
              class="tool-icon-wrapper"
              :class="[tool.icon_bg, tool.dark_icon_bg]"
            >
              <div class="text-2xl text-white" :class="[tool.icon]" />
            </div>

            <!-- 内容区域 -->
            <div class="tool-info">
              <div class="tool-header">
                <div class="tool-name">
                  {{ tool.name }}
                </div>
                <!-- 状态徽章 -->
                <n-tag
                  v-if="!tool.can_disable"
                  type="info"
                  size="small"
                  round
                  :bordered="false"
                >
                  核心
                </n-tag>
                <n-tag
                  v-else-if="tool.enabled"
                  type="success"
                  size="small"
                  round
                  :bordered="false"
                >
                  启用
                </n-tag>
                <n-tag
                  v-else
                  type="default"
                  size="small"
                  round
                  :bordered="false"
                >
                  禁用
                </n-tag>
              </div>

              <div class="tool-description">
                {{ tool.description }}
              </div>

              <!-- 操作区域 -->
              <div class="tool-actions">
                <n-button
                  v-if="tool.can_disable && tool.has_config"
                  size="tiny"
                  secondary
                  @click="openToolConfig(tool.id)"
                >
                  <template #icon>
                    <div class="i-carbon-settings" />
                  </template>
                  配置
                </n-button>
                <div class="flex-1" />
                <n-switch
                  v-if="tool.can_disable"
                  :value="tool.enabled"
                  size="small"
                  @update:value="toggleTool(tool.id)"
                >
                  <template #checked-icon>
                    <div class="i-carbon-checkmark" />
                  </template>
                  <template #unchecked-icon>
                    <div class="i-carbon-close" />
                  </template>
                </n-switch>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 底部统计 -->
      <div class="stats-footer">
        <div class="stats-badge">
          <div class="i-carbon-tool-kit text-primary-500" />
          {{ toolStats.enabled }} / {{ toolStats.total }} 工具正在运行
        </div>
      </div>
    </n-space>

    <!-- 配置弹窗 -->
    <n-modal
      v-model:show="showToolConfigModal"
      preset="card"
      :title="`${currentToolName} 配置`"
      :style="{ width: '850px', maxWidth: '95vw' }"
      :bordered="false"
      size="huge"
      class="config-modal"
      :content-style="{ maxHeight: 'calc(90vh - 120px)', overflow: 'hidden', display: 'flex', flexDirection: 'column' }"
      transform-origin="center"
    >
      <div class="modal-inner-container">
        <SouConfig v-if="currentToolId === 'sou'" :active="showToolConfigModal" />
        <Context7Config v-else-if="currentToolId === 'context7'" :active="showToolConfigModal" />
        <EnhanceConfig
          v-else-if="currentToolId === 'enhance'"
          :active="showToolConfigModal"
          :project-root-path="props.projectRootPath"
        />
        <IconWorkshop v-else-if="currentToolId === 'icon'" :active="showToolConfigModal" />
        <MemoryManager
          v-else-if="currentToolId === 'ji'"
          :active="showToolConfigModal"
          :project-root-path="props.projectRootPath"
        />
        <div v-else class="empty-config">
          <div class="i-carbon-settings text-5xl mb-3 opacity-20" />
          <div class="text-sm opacity-60">
            暂无高级配置项
          </div>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<style scoped>
/* ========== 工具卡片样式 ========== */
.tool-card {
  position: relative;
  border-radius: 14px;
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
  background: var(--color-container, rgba(255, 255, 255, 0.85));
  backdrop-filter: blur(12px);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04), 0 1px 2px rgba(0, 0, 0, 0.02);
}

/* 深色模式背景 */
:root.dark .tool-card {
  background: rgba(28, 28, 34, 0.9);
  border-color: rgba(255, 255, 255, 0.06);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2), 0 1px 2px rgba(0, 0, 0, 0.1);
}

/* 禁用状态 */
.tool-card--disabled {
  opacity: 0.55;
  filter: grayscale(0.4);
}

.tool-card--disabled:hover {
  opacity: 0.7;
}

/* 悬停效果 */
.tool-card:hover {
  transform: translateY(-3px);
  border-color: rgba(20, 184, 166, 0.25);
  box-shadow:
    0 12px 28px -5px rgba(20, 184, 166, 0.12),
    0 4px 12px -4px rgba(0, 0, 0, 0.06);
}

:root.dark .tool-card:hover {
  border-color: rgba(20, 184, 166, 0.3);
  box-shadow:
    0 12px 28px -5px rgba(20, 184, 166, 0.2),
    0 4px 12px -4px rgba(0, 0, 0, 0.15);
}

/* 顶部装饰线 */
.card-top-border {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: linear-gradient(
    90deg,
    transparent 5%,
    rgba(20, 184, 166, 0.6),
    transparent 95%
  );
  opacity: 0;
  transition: opacity 0.35s ease;
}

.tool-card:hover .card-top-border {
  opacity: 1;
}

/* 卡片内容 */
.card-content {
  display: flex;
  gap: 16px;
  padding: 18px;
}

/* 图标容器 */
.tool-icon-wrapper {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
}

.tool-card:hover .tool-icon-wrapper {
  transform: scale(1.08) rotate(-2deg);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

/* 工具信息区域 */
.tool-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tool-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tool-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-on-surface, #111827);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: -0.01em;
}

:root.dark .tool-name {
  color: #f3f4f6;
}

.tool-description {
  font-size: 12px;
  line-height: 1.5;
  color: var(--color-on-surface-secondary, #6b7280);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 36px;
  opacity: 0.85;
}

:root.dark .tool-description {
  color: #9ca3af;
}

/* 操作区域 */
.tool-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border, rgba(128, 128, 128, 0.1));
}

:root.dark .tool-actions {
  border-color: rgba(255, 255, 255, 0.06);
}

/* ========== 骨架屏样式 ========== */
.tool-card-skeleton {
  border-radius: 14px;
  padding: 18px;
  background: var(--color-container, rgba(255, 255, 255, 0.85));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

:root.dark .tool-card-skeleton {
  background: rgba(28, 28, 34, 0.9);
  border-color: rgba(255, 255, 255, 0.06);
}

.skeleton-header {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}

.skeleton-icon {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  background: linear-gradient(90deg, rgba(128,128,128,0.1) 25%, rgba(128,128,128,0.2) 50%, rgba(128,128,128,0.1) 75%);
  background-size: 200% 100%;
  animation: skeleton-loading 1.5s infinite;
}

.skeleton-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.skeleton-line {
  height: 12px;
  border-radius: 4px;
  background: linear-gradient(90deg, rgba(128,128,128,0.1) 25%, rgba(128,128,128,0.2) 50%, rgba(128,128,128,0.1) 75%);
  background-size: 200% 100%;
  animation: skeleton-loading 1.5s infinite;
}

.skeleton-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-top: 12px;
  border-top: 1px solid rgba(128, 128, 128, 0.1);
}

.skeleton-switch {
  width: 40px;
  height: 20px;
  border-radius: 10px;
  background: linear-gradient(90deg, rgba(128,128,128,0.1) 25%, rgba(128,128,128,0.2) 50%, rgba(128,128,128,0.1) 75%);
  background-size: 200% 100%;
  animation: skeleton-loading 1.5s infinite;
}

@keyframes skeleton-loading {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}

/* ========== 统计底栏 ========== */
.stats-footer {
  display: flex;
  justify-content: center;
  padding-top: 12px;
}

.stats-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 20px;
  border-radius: 24px;
  font-size: 12px;
  font-weight: 500;
  background: var(--color-container, rgba(255, 255, 255, 0.85));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
  color: var(--color-on-surface-secondary, #6b7280);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
  transition: all 0.2s ease;
}

.stats-badge:hover {
  border-color: rgba(20, 184, 166, 0.25);
  box-shadow: 0 2px 8px rgba(20, 184, 166, 0.06);
}

:root.dark .stats-badge {
  background: rgba(28, 28, 34, 0.9);
  border-color: rgba(255, 255, 255, 0.06);
  color: #9ca3af;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

/* ========== 重连提示 ========== */
.reconnect-alert {
  border-radius: 8px;
}

/* ========== 空配置状态 ========== */
.empty-config {
  height: 256px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--color-on-surface-muted, #9ca3af);
}

/* ========== 弹窗内容容器 ========== */
.modal-inner-container {
  height: calc(90vh - 120px);
  min-height: 400px;
  max-height: calc(90vh - 120px);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* ========== 过渡动画 ========== */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
