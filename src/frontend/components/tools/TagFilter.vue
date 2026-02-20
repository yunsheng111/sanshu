<script setup lang="ts">
/**
 * 标签筛选组件
 * 展示标签云，点击切换筛选状态
 * 通过 inject 获取共享的标签筛选状态
 */
import { computed, inject, ref } from 'vue'
import { MEMORY_TAGS_KEY } from './memoryKeys'

const props = defineProps<{
  /** 可用标签列表 */
  tags: string[]
}>()

const emit = defineEmits<{
  (e: 'change', selectedTags: string[]): void
}>()

// 尝试从 provide/inject 获取共享标签状态，如果不存在则使用本地状态
const injectedTags = inject(MEMORY_TAGS_KEY, undefined)
const localSelectedTags = ref<string[]>([])
const selectedTags = computed({
  get: () => injectedTags?.value ?? localSelectedTags.value,
  set: (val: string[]) => {
    if (injectedTags) {
      injectedTags.value = val
    }
    else {
      localSelectedTags.value = val
    }
  },
})

// ============ 工具函数 ============

/** 获取标签的颜色类型（分类标签有对应颜色） */
function getTagType(tag: string): 'default' | 'info' | 'success' | 'warning' | 'error' {
  const typeMap: Record<string, 'info' | 'success' | 'warning' | 'error'> = {
    规范: 'info',
    偏好: 'warning',
    模式: 'success',
    背景: 'warning',
  }
  return typeMap[tag] || 'default'
}

// ============ 事件处理 ============

/** 切换标签选中状态 */
function toggleTag(tag: string) {
  const current = [...selectedTags.value]
  const index = current.indexOf(tag)
  if (index >= 0) {
    current.splice(index, 1)
  }
  else {
    current.push(tag)
  }
  selectedTags.value = current
  emit('change', current)
}

/** 判断标签是否被选中 */
function isSelected(tag: string): boolean {
  return selectedTags.value.includes(tag)
}

/** 清除所有选中 */
function clearAll() {
  selectedTags.value = []
  emit('change', [])
}

/** 是否有选中的标签 */
const hasSelection = computed(() => selectedTags.value.length > 0)
</script>

<template>
  <div
    class="tag-filter"
    role="group"
    aria-label="标签筛选"
  >
    <div class="tag-filter-label">
      <div class="i-carbon-tag-group label-icon" aria-hidden="true" />
      <span>筛选</span>
    </div>
    <div class="tag-list">
      <n-tag
        v-for="tag in tags"
        :key="tag"
        :type="isSelected(tag) ? 'primary' : getTagType(tag)"
        :bordered="!isSelected(tag)"
        :checkable="true"
        :checked="isSelected(tag)"
        size="small"
        round
        class="filter-tag"
        :aria-pressed="isSelected(tag)"
        :tabindex="0"
        @update:checked="toggleTag(tag)"
        @keyup.space.prevent="toggleTag(tag)"
      >
        {{ tag }}
      </n-tag>

      <!-- 清除按钮 -->
      <n-button
        v-if="hasSelection"
        text
        type="tertiary"
        size="tiny"
        aria-label="清除所有标签筛选"
        class="clear-btn"
        @click="clearAll"
      >
        <template #icon>
          <div class="i-carbon-close" aria-hidden="true" />
        </template>
        清除
      </n-button>
    </div>
  </div>
</template>

<style scoped>
.tag-filter {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: 10px;
  background: var(--color-container, rgba(255, 255, 255, 0.3));
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.05));
}

:root.dark .tag-filter {
  background: rgba(24, 24, 30, 0.3);
  border-color: rgba(255, 255, 255, 0.03);
}

.tag-filter-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-on-surface-secondary, #94a3b8);
  white-space: nowrap;
  flex-shrink: 0;
}

.label-icon {
  font-size: 13px;
  color: rgba(20, 184, 166, 0.5);
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.filter-tag {
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
}

.filter-tag:hover {
  opacity: 0.85;
  transform: translateY(-1px);
}

.filter-tag:focus-visible {
  outline: 2px solid var(--color-primary, #3b82f6);
  outline-offset: 2px;
}

.clear-btn {
  opacity: 0.6;
  transition: opacity 0.2s;
}

.clear-btn:hover {
  opacity: 1;
}
</style>
