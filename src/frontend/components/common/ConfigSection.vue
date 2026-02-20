<script setup lang="ts">
/**
 * 配置区域组件
 * 提供统一的卡片式配置区块样式，支持主题自适应
 */
defineProps<{
  title?: string
  description?: string
  noCard?: boolean
}>()
</script>

<template>
  <div class="config-section">
    <!-- 标题区域 -->
    <div v-if="title || description" class="section-header">
      <h3 v-if="title" class="section-title">
        <slot name="icon" />
        {{ title }}
      </h3>
      <p v-if="description" class="section-description">
        {{ description }}
      </p>
    </div>

    <!-- 卡片内容 -->
    <div v-if="!noCard" class="section-card">
      <slot />
    </div>
    <div v-else class="section-content">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.config-section {
  margin-bottom: 8px;
}

/* 标题区域 */
.section-header {
  margin-bottom: 14px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  color: var(--color-on-surface, #111827);
  margin: 0;
  letter-spacing: -0.01em;
}

:root.dark .section-title {
  color: #f3f4f6;
}

.section-description {
  font-size: 12px;
  color: var(--color-on-surface-secondary, #6b7280);
  margin: 6px 0 0 0;
  line-height: 1.5;
}

:root.dark .section-description {
  color: #9ca3af;
}

/* 卡片样式 */
.section-card {
  padding: 18px;
  border-radius: 12px;
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
  background: var(--color-container, rgba(255, 255, 255, 0.7));
  backdrop-filter: blur(12px);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04), 0 1px 2px rgba(0, 0, 0, 0.02);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

:root.dark .section-card {
  background: rgba(30, 30, 36, 0.7);
  border-color: rgba(255, 255, 255, 0.06);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2), 0 1px 2px rgba(0, 0, 0, 0.1);
}

.section-card:hover {
  border-color: rgba(20, 184, 166, 0.3);
  box-shadow: 0 4px 12px rgba(20, 184, 166, 0.06), 0 1px 3px rgba(0, 0, 0, 0.04);
  transform: translateY(-1px);
}

:root.dark .section-card:hover {
  border-color: rgba(20, 184, 166, 0.35);
  box-shadow: 0 4px 12px rgba(20, 184, 166, 0.08), 0 1px 3px rgba(0, 0, 0, 0.2);
}

/* 无卡片内容 */
.section-content {
  padding: 0;
}
</style>
