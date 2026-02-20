/**
 * 活力值衰减计算
 * 根据记忆的访问频率和时间衰减计算活力值
 * 活力值用于判断记忆的"健康度"，指导自动清理策略
 */
import { computed, type Ref } from 'vue'

export interface VitalityData {
  /** 当前活力值（0.0 ~ 5.0） */
  score: number
  /** 最后访问时间 */
  lastAccessed?: string
  /** 访问次数 */
  accessCount?: number
  /** 过去30天每日活力值快照（用于趋势图） */
  trend?: number[]
}

export type VitalityLevel = 'high' | 'medium' | 'low'

/**
 * 活力值颜色编码和级别判定
 */
export function useVitalityDecay() {
  /** 根据活力值获取级别 */
  function getLevel(score: number): VitalityLevel {
    if (score > 2.0) return 'high'
    if (score >= 1.0) return 'medium'
    return 'low'
  }

  /** 根据活力值获取颜色（用于 NProgress 等组件） */
  function getColor(score: number): string {
    const level = getLevel(score)
    switch (level) {
      case 'high': return '#22c55e'   // 绿色
      case 'medium': return '#eab308' // 黄色
      case 'low': return '#ef4444'    // 红色
    }
  }

  /** 根据活力值获取 UnoCSS 颜色类名 */
  function getColorClass(score: number): string {
    const level = getLevel(score)
    switch (level) {
      case 'high': return 'text-green-500'
      case 'medium': return 'text-yellow-500'
      case 'low': return 'text-red-500'
    }
  }

  /** 获取活力值百分比（用于进度条，最大值 5.0 映射为 100%） */
  function getPercentage(score: number): number {
    return Math.min(Math.max((score / 5.0) * 100, 0), 100)
  }

  /** 获取活力值描述文本 */
  function getDescription(score: number): string {
    const level = getLevel(score)
    switch (level) {
      case 'high': return '活跃'
      case 'medium': return '一般'
      case 'low': return '低活跃'
    }
  }

  /** 格式化活力值显示 */
  function formatScore(score: number): string {
    return score.toFixed(1)
  }

  /** 计算衰减趋势（正值=上升，负值=下降） */
  function getTrend(trendData: number[]): 'up' | 'down' | 'stable' {
    if (!trendData || trendData.length < 2) return 'stable'
    const recent = trendData.slice(-7) // 最近7天
    const avg1 = recent.slice(0, Math.floor(recent.length / 2)).reduce((a, b) => a + b, 0) / Math.floor(recent.length / 2)
    const avg2 = recent.slice(Math.floor(recent.length / 2)).reduce((a, b) => a + b, 0) / (recent.length - Math.floor(recent.length / 2))
    const diff = avg2 - avg1
    if (diff > 0.1) return 'up'
    if (diff < -0.1) return 'down'
    return 'stable'
  }

  /** 获取趋势图标类名 */
  function getTrendIcon(trendData: number[]): string {
    const trend = getTrend(trendData)
    switch (trend) {
      case 'up': return 'i-carbon-arrow-up'
      case 'down': return 'i-carbon-arrow-down'
      case 'stable': return 'i-carbon-subtract'
    }
  }

  /** 获取趋势颜色类名 */
  function getTrendColorClass(trendData: number[]): string {
    const trend = getTrend(trendData)
    switch (trend) {
      case 'up': return 'text-green-500'
      case 'down': return 'text-red-500'
      case 'stable': return 'text-gray-400'
    }
  }

  return {
    getLevel,
    getColor,
    getColorClass,
    getPercentage,
    getDescription,
    formatScore,
    getTrend,
    getTrendIcon,
    getTrendColorClass,
  }
}
