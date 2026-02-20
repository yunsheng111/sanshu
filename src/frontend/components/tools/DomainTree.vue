<script setup lang="ts">
/**
 * 域树组件
 * 使用 NTree 渲染域/路径树结构
 * 通过后端"域列表"操作获取数据，支持右键菜单
 */
import { useSafeInvoke } from '../../composables/useSafeInvoke'
import { useMessage } from 'naive-ui'
import { computed, nextTick, onMounted, ref, watch, type TreeOption } from 'vue'

// Props
const props = defineProps<{
  projectRootPath: string
  selectedDomain: string | null
}>()

// Emits
const emit = defineEmits<{
  (e: 'select', domain: string | null): void
}>()

const message = useMessage()
const { safeInvoke, loading } = useSafeInvoke()

// ============ 类型定义 ============

interface DomainInfo {
  /** 域路径 */
  path: string
  /** 记忆数量 */
  count: number
  /** 子域列表 */
  children?: DomainInfo[]
}

// ============ 状态 ============

/** 域列表原始数据 */
const domains = ref<DomainInfo[]>([])

/** 当前选中的 key */
const selectedKeys = ref<string[]>([])

/** 右键菜单状态 */
const showContextMenu = ref(false)
const contextMenuX = ref(0)
const contextMenuY = ref(0)
const contextMenuTarget = ref<string | null>(null)

// ============ 计算属性 ============

/** 将域数据转换为 NTree 的 TreeOption 格式 */
const treeData = computed<TreeOption[]>(() => {
  // 添加"全部"根节点
  const allNode: TreeOption = {
    key: '__all__',
    label: `全部`,
    prefix: () => h('div', { class: 'i-carbon-data-base domain-icon domain-icon--all', 'aria-hidden': 'true' }),
    suffix: () => h('span', { class: 'domain-count domain-count--all' }, totalCount.value.toString()),
  }

  const nodes = domains.value.map(d => domainToTreeOption(d))
  return [allNode, ...nodes]
})

/** 总记忆数量 */
const totalCount = computed(() => {
  return domains.value.reduce((sum, d) => sum + countDomain(d), 0)
})

// ============ 工具函数 ============

/** 递归计算域的记忆总数 */
function countDomain(domain: DomainInfo): number {
  let count = domain.count
  if (domain.children) {
    for (const child of domain.children) {
      count += countDomain(child)
    }
  }
  return count
}

/** 获取域图标（根据层级和内容量） */
function getDomainIcon(domain: DomainInfo): string {
  if (domain.children && domain.children.length > 0) {
    return 'i-carbon-folder'
  }
  if (domain.count === 0) {
    return 'i-carbon-folder-off'
  }
  return 'i-carbon-folder'
}

/** 获取域图标颜色类 */
function getDomainIconClass(domain: DomainInfo): string {
  if (domain.count === 0) {
    return 'domain-icon domain-icon--empty'
  }
  if (domain.children && domain.children.length > 0) {
    return 'domain-icon domain-icon--parent'
  }
  return 'domain-icon domain-icon--leaf'
}

/** 将 DomainInfo 转换为 TreeOption */
function domainToTreeOption(domain: DomainInfo): TreeOption {
  const iconClass = getDomainIcon(domain)
  const colorClass = getDomainIconClass(domain)

  const option: TreeOption = {
    key: domain.path,
    label: getShortName(domain.path),
    prefix: () => h('div', { class: `${iconClass} ${colorClass}`, 'aria-hidden': 'true' }),
    suffix: () => h('span', {
      class: `domain-count ${domain.count === 0 ? 'domain-count--zero' : ''}`,
    }, domain.count.toString()),
  }

  if (domain.children && domain.children.length > 0) {
    option.children = domain.children.map(c => domainToTreeOption(c))
  }

  return option
}

/** 获取域路径的短名称 */
function getShortName(path: string): string {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

// 需要导入 h 函数
import { h } from 'vue'

// ============ 数据加载 ============

/** 加载域列表 */
async function loadDomains() {
  const result = await safeInvoke<DomainInfo[]>('get_domain_list', {
    projectPath: props.projectRootPath,
  })

  if (result) {
    domains.value = result
  }
}

// ============ 事件处理 ============

/** 树节点被选中 */
function handleSelect(keys: string[]) {
  selectedKeys.value = keys
  const selectedKey = keys[0] ?? null
  if (selectedKey === '__all__') {
    emit('select', null) // null 表示全部
  } else {
    emit('select', selectedKey)
  }
}

/** 右键菜单 */
function handleContextMenu(e: MouseEvent, option: TreeOption) {
  e.preventDefault()
  const key = option.key as string
  if (key === '__all__') return // 不对"全部"节点显示菜单

  contextMenuTarget.value = key
  contextMenuX.value = e.clientX
  contextMenuY.value = e.clientY
  showContextMenu.value = true
}

/** 右键菜单选项 */
const contextMenuOptions = [
  {
    label: '删除空域',
    key: 'delete-empty',
    icon: () => h('div', { class: 'i-carbon-trash-can text-red-400' }),
  },
]

/** 处理右键菜单选择 */
async function handleContextMenuSelect(key: string) {
  showContextMenu.value = false

  if (key === 'delete-empty' && contextMenuTarget.value) {
    // 检查域是否为空
    const targetDomain = findDomain(domains.value, contextMenuTarget.value)
    if (targetDomain && targetDomain.count === 0) {
      const result = await safeInvoke<boolean>('delete_empty_domain', {
        projectPath: props.projectRootPath,
        domain: contextMenuTarget.value,
      })
      if (result) {
        message.success('空域已删除')
        await loadDomains()
      }
    } else {
      message.warning('仅可删除记忆数为 0 的空域')
    }
  }
}

/** 递归查找域 */
function findDomain(list: DomainInfo[], path: string): DomainInfo | null {
  for (const d of list) {
    if (d.path === path) return d
    if (d.children) {
      const found = findDomain(d.children, path)
      if (found) return found
    }
  }
  return null
}

/** 关闭右键菜单 */
function handleClickOutside() {
  showContextMenu.value = false
}

// ============ 生命周期 ============

watch(() => props.projectRootPath, () => {
  loadDomains()
})

onMounted(() => {
  if (props.projectRootPath) {
    loadDomains()
  }
})

// 暴露刷新方法
defineExpose({
  refresh: loadDomains,
})
</script>

<template>
  <div
    class="domain-tree"
    role="tree"
    aria-label="域树"
  >
    <!-- 标题栏 -->
    <div class="tree-header">
      <span class="tree-title">域列表</span>
      <n-button
        text
        type="primary"
        size="tiny"
        :loading="loading"
        aria-label="刷新域列表"
        class="refresh-btn"
        @click="loadDomains"
      >
        <template #icon>
          <div class="i-carbon-renew" aria-hidden="true" />
        </template>
      </n-button>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading && domains.length === 0" class="loading-state">
      <div class="skeleton-item" v-for="i in 4" :key="i">
        <n-skeleton text style="width: 70%" />
      </div>
    </div>

    <!-- 空状态 -->
    <div v-else-if="!loading && domains.length === 0" class="empty-state">
      <div class="empty-illustration">
        <div class="i-carbon-tree-view" aria-hidden="true" />
      </div>
      <div class="empty-text">暂无域数据</div>
      <div class="empty-hint">记忆将按项目路径自动归类</div>
    </div>

    <!-- 域树 -->
    <n-tree
      v-else
      :data="treeData"
      :selected-keys="selectedKeys"
      block-line
      selectable
      :keyboard="true"
      @update:selected-keys="handleSelect"
      @node-props="({ option }) => ({
        onContextmenu: (e: MouseEvent) => handleContextMenu(e, option),
      })"
    />

    <!-- 右键菜单 -->
    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="contextMenuX"
      :y="contextMenuY"
      :options="contextMenuOptions"
      :show="showContextMenu"
      :on-clickoutside="handleClickOutside"
      @select="handleContextMenuSelect"
    />
  </div>
</template>

<style scoped>
.domain-tree {
  padding: 10px 10px 16px 10px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.tree-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 4px 10px 4px;
  margin-bottom: 2px;
}

.tree-title {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-on-surface-secondary, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

:root.dark .tree-title {
  color: #94a3b8;
}

.refresh-btn {
  opacity: 0.5;
  transition: opacity 0.2s;
}

.refresh-btn:hover {
  opacity: 1;
}

/* 加载骨架 */
.loading-state {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 8px 4px;
}

.skeleton-item {
  padding: 6px 8px;
}

/* 空状态 - 友好提示 */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px 16px;
  gap: 8px;
  flex: 1;
}

.empty-illustration {
  width: 48px;
  height: 48px;
  border-radius: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.06), rgba(59, 130, 246, 0.04));
  border: 1px dashed rgba(20, 184, 166, 0.2);
  margin-bottom: 4px;
}

:root.dark .empty-illustration {
  background: linear-gradient(135deg, rgba(20, 184, 166, 0.1), rgba(59, 130, 246, 0.08));
  border-color: rgba(20, 184, 166, 0.25);
}

.empty-illustration .i-carbon-tree-view {
  font-size: 22px;
  color: rgba(20, 184, 166, 0.4);
}

.empty-text {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-on-surface-secondary, #6b7280);
  opacity: 0.7;
}

:root.dark .empty-text {
  color: #9ca3af;
}

.empty-hint {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #9ca3af);
  opacity: 0.5;
  text-align: center;
  line-height: 1.4;
}

/* 域图标样式 */
:deep(.domain-icon) {
  font-size: 14px;
  transition: color 0.2s;
}

:deep(.domain-icon--all) {
  color: rgba(20, 184, 166, 0.7);
}

:deep(.domain-icon--parent) {
  color: rgba(251, 191, 36, 0.7);
}

:deep(.domain-icon--leaf) {
  color: rgba(59, 130, 246, 0.6);
}

:deep(.domain-icon--empty) {
  color: rgba(156, 163, 175, 0.4);
}

/* 域计数样式 */
:deep(.domain-count) {
  font-size: 11px;
  color: var(--color-on-surface-secondary, #64748b);
  background: rgba(20, 184, 166, 0.08);
  padding: 1px 8px;
  border-radius: 10px;
  min-width: 22px;
  text-align: center;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  transition: all 0.2s;
}

:deep(.domain-count--all) {
  background: rgba(20, 184, 166, 0.12);
  color: rgba(20, 184, 166, 0.9);
}

:root.dark :deep(.domain-count) {
  background: rgba(20, 184, 166, 0.12);
  color: #94a3b8;
}

:root.dark :deep(.domain-count--all) {
  background: rgba(20, 184, 166, 0.18);
  color: rgba(20, 184, 166, 0.9);
}

:deep(.domain-count--zero) {
  background: rgba(156, 163, 175, 0.08);
  color: rgba(156, 163, 175, 0.5);
}

:root.dark :deep(.domain-count--zero) {
  background: rgba(255, 255, 255, 0.04);
  color: rgba(156, 163, 175, 0.4);
}

/* 树节点悬停增强 */
:deep(.n-tree-node:hover .domain-count) {
  background: rgba(20, 184, 166, 0.15);
}

:root.dark :deep(.n-tree-node:hover .domain-count) {
  background: rgba(20, 184, 166, 0.2);
}
</style>
