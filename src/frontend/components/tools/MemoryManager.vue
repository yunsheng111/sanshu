<script setup lang="ts">
/**
 * 记忆管理器主容器
 * 使用左右分栏布局：左侧域树 + 中间工作区
 * 底部 Tab 切换：工作区 | 搜索 | 配置
 * 通过 provide/inject 传递共享状态
 */
import { computed, provide, ref } from 'vue'
import DomainTree from './DomainTree.vue'
import MemoryWorkspace from './MemoryWorkspace.vue'
import MemorySearch from './MemorySearch.vue'
import MemoryConfig from './MemoryConfig.vue'
import {
  MEMORY_DOMAIN_KEY,
  MEMORY_SEARCH_KEY,
  MEMORY_TAGS_KEY,
  MEMORY_BATCH_MODE_KEY,
  MEMORY_SELECTED_IDS_KEY,
} from './memoryKeys'

// Props
const props = defineProps<{
  projectRootPath?: string | null
}>()

// ============ 共享状态（provide/inject） ============

/** 当前选中的域路径 */
const selectedDomain = ref<string | null>(null)

/** 搜索关键词 */
const searchKeyword = ref('')

/** 当前选中的标签筛选 */
const selectedTags = ref<string[]>([])

/** 是否处于多选模式 */
const batchMode = ref(false)

/** 多选选中的记忆 ID 列表 */
const selectedMemoryIds = ref<string[]>([])

// 注入 key 从 memoryKeys.ts 导入（<script setup> 不支持 export）

// 提供共享状态给子组件
provide(MEMORY_DOMAIN_KEY, selectedDomain)
provide(MEMORY_SEARCH_KEY, searchKeyword)
provide(MEMORY_TAGS_KEY, selectedTags)
provide(MEMORY_BATCH_MODE_KEY, batchMode)
provide(MEMORY_SELECTED_IDS_KEY, selectedMemoryIds)

// ============ 布局状态 ============

/** 侧边栏折叠状态 */
const siderCollapsed = ref(false)

/** 当前激活的底部 Tab */
const activeTab = ref('workspace')

/** 项目路径计算属性 */
const projectPath = computed(() => props.projectRootPath || '')

// ============ 事件处理 ============

/** 域树节点被选中 */
function handleDomainSelect(domain: string | null) {
  selectedDomain.value = domain
  // 切换域时退出批量模式
  batchMode.value = false
  selectedMemoryIds.value = []
}

/** 搜索触发：自动切换到搜索 Tab */
function handleSearchTrigger(keyword: string) {
  searchKeyword.value = keyword
  activeTab.value = 'search'
}

/** 退出批量模式 */
function exitBatchMode() {
  batchMode.value = false
  selectedMemoryIds.value = []
}
</script>

<template>
  <div
    class="memory-manager"
    role="region"
    aria-label="记忆管理器"
  >
    <!-- 无项目路径提示 -->
    <div v-if="!projectPath" class="empty-state">
      <div class="empty-state-icon">
        <div class="i-carbon-folder-off" aria-hidden="true" />
      </div>
      <div class="empty-state-title">
        尚未连接项目
      </div>
      <div class="empty-state-desc">
        请先在 MCP 工具中指定项目路径
      </div>
    </div>

    <template v-else>
      <div class="manager-layout">
        <!-- 左侧域树 -->
        <div v-if="!siderCollapsed" class="domain-sider" role="navigation" aria-label="域树导航">
          <div class="sider-header">
            <div class="sider-header-title">
              <div class="i-carbon-tree-view sider-header-icon" aria-hidden="true" />
              <span>记忆空间</span>
            </div>
            <n-button quaternary size="tiny" class="sider-collapse-btn" @click="siderCollapsed = true">
              <template #icon><div class="i-carbon-chevron-left" /></template>
            </n-button>
          </div>
          <n-scrollbar class="sider-scroll">
            <DomainTree
              :project-root-path="projectPath"
              :selected-domain="selectedDomain"
              @select="handleDomainSelect"
            />
          </n-scrollbar>
        </div>
        <div v-else class="domain-sider-collapsed" @click="siderCollapsed = false">
          <div class="collapsed-indicator">
            <div class="i-carbon-chevron-right" />
          </div>
        </div>

        <!-- 中间内容区 -->
        <div class="main-content">
          <n-tabs
            v-model:value="activeTab"
            type="line"
            animated
            class="content-tabs"
          >
            <!-- 工作区 Tab -->
            <n-tab-pane name="workspace" tab="工作区">
              <MemoryWorkspace
                :project-root-path="projectPath"
                :active="activeTab === 'workspace'"
                @search="handleSearchTrigger"
              />
            </n-tab-pane>

            <!-- 搜索 Tab -->
            <n-tab-pane name="search" tab="搜索">
              <MemorySearch
                :project-root-path="projectPath"
              />
            </n-tab-pane>

            <!-- 配置 Tab -->
            <n-tab-pane name="config" tab="配置">
              <MemoryConfig
                :active="activeTab === 'config'"
                :project-root-path="projectPath"
              />
            </n-tab-pane>
          </n-tabs>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.memory-manager {
  min-width: 0;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
}

/* 空状态 - 重设计 */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 400px;
  color: var(--color-on-surface-muted, #9ca3af);
  gap: 12px;
}

.empty-state-icon {
  width: 72px;
  height: 72px;
  border-radius: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.08), rgba(59, 130, 246, 0.06));
  border: 1px solid rgba(20, 184, 166, 0.12);
  margin-bottom: 4px;
}

:root.dark .empty-state-icon {
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.12), rgba(59, 130, 246, 0.1));
  border-color: rgba(20, 184, 166, 0.2);
}

.empty-state-icon .i-carbon-folder-off {
  font-size: 32px;
  opacity: 0.4;
  color: var(--color-on-surface-secondary, #6b7280);
}

.empty-state-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-on-surface, #374151);
  opacity: 0.7;
}

:root.dark .empty-state-title {
  color: #d1d5db;
}

.empty-state-desc {
  font-size: 13px;
  opacity: 0.5;
}

/* 主布局 */
.manager-layout {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

/* 左侧域树 - 重设计 */
.domain-sider {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--color-border, rgba(128, 128, 128, 0.08));
  background: linear-gradient(180deg, rgba(248, 250, 252, 0.8) 0%, rgba(241, 245, 249, 0.6) 100%);
}

:root.dark .domain-sider {
  background: linear-gradient(180deg, rgba(20, 20, 26, 0.6) 0%, rgba(16, 16, 22, 0.5) 100%);
  border-color: rgba(255, 255, 255, 0.04);
}

/* 侧边栏标题 */
.sider-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 14px 10px 14px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.06);
}

:root.dark .sider-header {
  border-color: rgba(255, 255, 255, 0.04);
}

.sider-header-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  font-weight: 700;
  color: var(--color-on-surface-secondary, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

:root.dark .sider-header-title {
  color: #94a3b8;
}

.sider-header-icon {
  font-size: 14px;
  color: rgba(20, 184, 166, 0.7);
}

.sider-collapse-btn {
  opacity: 0.4;
  transition: opacity 0.2s;
}

.sider-collapse-btn:hover {
  opacity: 1;
}

.sider-scroll {
  flex: 1;
  min-height: 0;
}

/* 折叠状态 - 重设计 */
.domain-sider-collapsed {
  width: 28px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-right: 1px solid var(--color-border, rgba(128, 128, 128, 0.06));
  transition: all 0.2s;
}

.domain-sider-collapsed:hover {
  background: linear-gradient(180deg, rgba(20, 184, 166, 0.04) 0%, rgba(20, 184, 166, 0.02) 100%);
}

:root.dark .domain-sider-collapsed:hover {
  background: linear-gradient(180deg, rgba(20, 184, 166, 0.08) 0%, rgba(20, 184, 166, 0.04) 100%);
}

.collapsed-indicator {
  opacity: 0.3;
  transition: opacity 0.2s, color 0.2s;
  font-size: 12px;
  color: var(--color-on-surface-secondary, #9ca3af);
}

.domain-sider-collapsed:hover .collapsed-indicator {
  opacity: 0.8;
  color: rgba(20, 184, 166, 0.8);
}

/* 右侧内容区 */
.main-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: 0 20px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* n-tabs 内部需要 flex 布局才能让 pane-wrapper 撑满 */
.content-tabs :deep(.n-tabs-nav) {
  flex-shrink: 0;
}

.content-tabs :deep(.n-tabs-pane-wrapper) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.content-tabs :deep(.n-tab-pane) {
  padding: 16px 0;
  height: 100%;
  overflow: auto;
}
</style>
