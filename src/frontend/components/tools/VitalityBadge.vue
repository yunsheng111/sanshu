<script setup lang="ts">
/**
 * 活力值徽章组件
 * 使用环形进度条展示记忆活力值
 * 颜色编码：绿(>2.0) / 黄(1.0-2.0) / 红(<1.0)
 * Tooltip 显示衰减趋势
 */
import { computed } from 'vue'
import { useVitalityDecay, type VitalityData } from '../../composables/useVitalityDecay'

const props = defineProps<{
  /** 活力值数据 */
  vitality: VitalityData
  /** 尺寸：small(24px) / medium(36px) / large(48px) */
  size?: 'small' | 'medium' | 'large'
  /** 是否显示趋势迷你折线图 */
  showTrend?: boolean
}>()

const {
  getColor,
  getPercentage,
  getDescription,
  formatScore,
  getTrend,
  getTrendIcon,
  getTrendColorClass,
} = useVitalityDecay()

// ============ 计算属性 ============

const sizeMap = { small: 24, medium: 36, large: 48 }
const strokeWidthMap = { small: 3, medium: 4, large: 5 }

const circleSize = computed(() => sizeMap[props.size ?? 'small'])
const strokeWidth = computed(() => strokeWidthMap[props.size ?? 'small'])
const color = computed(() => getColor(props.vitality.score))
const percentage = computed(() => getPercentage(props.vitality.score))
const description = computed(() => getDescription(props.vitality.score))
const formattedScore = computed(() => formatScore(props.vitality.score))

/** Tooltip 内容 */
const tooltipContent = computed(() => {
  const lines = [
    `活力值: ${formattedScore.value} / 3.0`,
    `状态: ${description.value}`,
  ]
  if (props.vitality.accessCount !== undefined) {
    lines.push(`访问次数: ${props.vitality.accessCount}`)
  }
  if (props.vitality.lastAccessed) {
    try {
      lines.push(`最后访问: ${new Date(props.vitality.lastAccessed).toLocaleString('zh-CN')}`)
    }
    catch {
      // 忽略日期解析错误
    }
  }
  if (props.vitality.trend && props.vitality.trend.length > 1) {
    const trend = getTrend(props.vitality.trend)
    const trendLabel = trend === 'up' ? '上升' : trend === 'down' ? '下降' : '平稳'
    lines.push(`趋势: ${trendLabel}`)
  }
  return lines.join('\n')
})

// ============ 迷你折线图（SVG） ============

/** 生成迷你 SVG 折线图路径 */
const trendSvgPath = computed(() => {
  if (!props.showTrend || !props.vitality.trend || props.vitality.trend.length < 2) {
    return ''
  }

  const data = props.vitality.trend.slice(-30) // 最近30天
  const width = 60
  const height = 20
  const maxVal = Math.max(...data, 3.0) // 最大值不低于3.0
  const minVal = 0

  const points = data.map((val, i) => {
    const x = (i / (data.length - 1)) * width
    const y = height - ((val - minVal) / (maxVal - minVal)) * height
    return `${x},${y}`
  })

  return `M ${points.join(' L ')}`
})

const trendLineColor = computed(() => {
  if (!props.vitality.trend)
    return '#9ca3af'
  return getColor(props.vitality.score)
})
</script>

<template>
  <n-tooltip trigger="hover" :style="{ whiteSpace: 'pre-line' }">
    <template #trigger>
      <div
        class="vitality-badge"
        role="status"
        :aria-label="`活力值 ${formattedScore}，${description}`"
      >
        <!-- 外层光晕 -->
        <div
          class="vitality-glow"
          :style="{ '--glow-color': color }"
        />

        <!-- 环形进度条 -->
        <n-progress
          type="circle"
          :percentage="percentage"
          :stroke-width="strokeWidth"
          :color="color"
          :show-indicator="false"
          :style="{ width: `${circleSize}px`, height: `${circleSize}px` }"
        />

        <!-- 数值覆盖层 -->
        <span
          class="vitality-score"
          :style="{ color, fontSize: size === 'large' ? '12px' : size === 'medium' ? '10px' : '8px' }"
        >
          {{ formattedScore }}
        </span>
      </div>
    </template>

    <!-- Tooltip 内容 -->
    <div class="vitality-tooltip">
      <div class="tooltip-header">
        <div class="tooltip-score" :style="{ color }">
          {{ formattedScore }}
        </div>
        <div class="tooltip-desc">
          {{ description }}
        </div>
      </div>
      <div class="tooltip-details">
        {{ tooltipContent }}
      </div>
      <!-- 迷你趋势图 -->
      <div v-if="showTrend && vitality.trend && vitality.trend.length >= 2" class="trend-chart">
        <svg
          :width="60"
          :height="20"
          viewBox="0 0 60 20"
          class="trend-svg"
          aria-label="活力趋势图"
          role="img"
        >
          <!-- 参考线（50%位置） -->
          <line
            x1="0" y1="10" x2="60" y2="10"
            stroke="rgba(255,255,255,0.1)"
            stroke-width="0.5"
            stroke-dasharray="2,2"
          />
          <path
            :d="trendSvgPath"
            fill="none"
            :stroke="trendLineColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <span class="trend-label">
          <span :class="getTrendColorClass(vitality.trend)">
            <span :class="getTrendIcon(vitality.trend)" aria-hidden="true" />
            {{ getTrend(vitality.trend) === 'up' ? '上升' : getTrend(vitality.trend) === 'down' ? '下降' : '平稳' }}
          </span>
        </span>
      </div>
    </div>
  </n-tooltip>
</template>

<style scoped>
.vitality-badge {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: default;
  transition: transform 0.2s ease;
}

.vitality-badge:hover {
  transform: scale(1.1);
}

/* 外层光晕效果 */
.vitality-glow {
  position: absolute;
  inset: -2px;
  border-radius: 50%;
  background: radial-gradient(circle, var(--glow-color, transparent) 0%, transparent 70%);
  opacity: 0;
  transition: opacity 0.3s ease;
  pointer-events: none;
}

.vitality-badge:hover .vitality-glow {
  opacity: 0.15;
}

.vitality-score {
  position: absolute;
  font-weight: 700;
  line-height: 1;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
}

/* Tooltip 样式重设计 */
.vitality-tooltip {
  font-size: 12px;
  line-height: 1.6;
  min-width: 140px;
}

.tooltip-header {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.tooltip-score {
  font-size: 16px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.tooltip-desc {
  font-size: 11px;
  opacity: 0.7;
}

.tooltip-details {
  font-size: 11px;
  opacity: 0.85;
  line-height: 1.6;
  white-space: pre-line;
}

.trend-chart {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.trend-svg {
  flex-shrink: 0;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.04);
  padding: 2px;
}

.trend-label {
  font-size: 11px;
  display: flex;
  align-items: center;
  gap: 3px;
  font-weight: 500;
}
</style>
