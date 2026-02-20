// 活力值衰减计算 composable 单元测试
import { describe, expect, it } from 'vitest'
import { useVitalityDecay } from '../useVitalityDecay'

describe('useVitalityDecay', () => {
  // --- getLevel 级别判定 ---
  describe('getLevel 级别判定', () => {
    it('score > 2.0 应返回 high', () => {
      const { getLevel } = useVitalityDecay()
      expect(getLevel(2.1)).toBe('high')
      expect(getLevel(3.0)).toBe('high')
      expect(getLevel(5.0)).toBe('high')
    })

    it('1.0 <= score <= 2.0 应返回 medium', () => {
      const { getLevel } = useVitalityDecay()
      expect(getLevel(1.0)).toBe('medium')
      expect(getLevel(1.5)).toBe('medium')
      expect(getLevel(2.0)).toBe('medium')
    })

    it('score < 1.0 应返回 low', () => {
      const { getLevel } = useVitalityDecay()
      expect(getLevel(0.0)).toBe('low')
      expect(getLevel(0.5)).toBe('low')
      expect(getLevel(0.99)).toBe('low')
    })
  })

  // --- getColor 颜色映射 ---
  describe('getColor 颜色映射', () => {
    it('high 级别应返回绿色', () => {
      const { getColor } = useVitalityDecay()
      expect(getColor(2.5)).toBe('#22c55e')
    })

    it('medium 级别应返回黄色', () => {
      const { getColor } = useVitalityDecay()
      expect(getColor(1.5)).toBe('#eab308')
    })

    it('low 级别应返回红色', () => {
      const { getColor } = useVitalityDecay()
      expect(getColor(0.3)).toBe('#ef4444')
    })
  })

  // --- getColorClass UnoCSS 类名 ---
  describe('getColorClass 类名映射', () => {
    it('high 应返回 text-green-500', () => {
      const { getColorClass } = useVitalityDecay()
      expect(getColorClass(3.0)).toBe('text-green-500')
    })

    it('medium 应返回 text-yellow-500', () => {
      const { getColorClass } = useVitalityDecay()
      expect(getColorClass(1.5)).toBe('text-yellow-500')
    })

    it('low 应返回 text-red-500', () => {
      const { getColorClass } = useVitalityDecay()
      expect(getColorClass(0.1)).toBe('text-red-500')
    })
  })

  // --- getPercentage 百分比计算 ---
  describe('getPercentage 百分比计算', () => {
    it('score 5.0 应映射为 100%', () => {
      const { getPercentage } = useVitalityDecay()
      expect(getPercentage(5.0)).toBe(100)
    })

    it('score 0.0 应映射为 0%', () => {
      const { getPercentage } = useVitalityDecay()
      expect(getPercentage(0.0)).toBe(0)
    })

    it('score 2.5 应映射为 50%', () => {
      const { getPercentage } = useVitalityDecay()
      expect(getPercentage(2.5)).toBe(50)
    })

    it('score 超过 5.0 应被钳位到 100%', () => {
      const { getPercentage } = useVitalityDecay()
      expect(getPercentage(10.0)).toBe(100)
    })

    it('score 为负数应被钳位到 0%', () => {
      const { getPercentage } = useVitalityDecay()
      expect(getPercentage(-1.0)).toBe(0)
    })
  })

  // --- getDescription 描述文本 ---
  describe('getDescription 描述文本', () => {
    it('high 应返回"活跃"', () => {
      const { getDescription } = useVitalityDecay()
      expect(getDescription(2.5)).toBe('活跃')
    })

    it('medium 应返回"一般"', () => {
      const { getDescription } = useVitalityDecay()
      expect(getDescription(1.5)).toBe('一般')
    })

    it('low 应返回"低活跃"', () => {
      const { getDescription } = useVitalityDecay()
      expect(getDescription(0.3)).toBe('低活跃')
    })
  })

  // --- formatScore 格式化 ---
  describe('formatScore 格式化', () => {
    it('应保留一位小数', () => {
      const { formatScore } = useVitalityDecay()
      expect(formatScore(1.5)).toBe('1.5')
      expect(formatScore(2.0)).toBe('2.0')
      expect(formatScore(0.0)).toBe('0.0')
    })

    it('应对多位小数进行四舍五入', () => {
      const { formatScore } = useVitalityDecay()
      expect(formatScore(1.55)).toBe('1.6')
      expect(formatScore(1.54)).toBe('1.5')
      expect(formatScore(2.999)).toBe('3.0')
    })

    it('整数应显示为 x.0 格式', () => {
      const { formatScore } = useVitalityDecay()
      expect(formatScore(3)).toBe('3.0')
      expect(formatScore(0)).toBe('0.0')
    })
  })

  // --- getTrend 趋势判定 ---
  describe('getTrend 趋势判定', () => {
    it('空数组应返回 stable', () => {
      const { getTrend } = useVitalityDecay()
      expect(getTrend([])).toBe('stable')
    })

    it('单元素数组应返回 stable', () => {
      const { getTrend } = useVitalityDecay()
      expect(getTrend([1.5])).toBe('stable')
    })

    it('null/undefined 应返回 stable', () => {
      const { getTrend } = useVitalityDecay()
      expect(getTrend(null as any)).toBe('stable')
      expect(getTrend(undefined as any)).toBe('stable')
    })

    it('递增序列应返回 up', () => {
      const { getTrend } = useVitalityDecay()
      // 前半部分均值 < 后半部分均值，差值 > 0.1
      expect(getTrend([1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0])).toBe('up')
    })

    it('递减序列应返回 down', () => {
      const { getTrend } = useVitalityDecay()
      // 前半部分均值 > 后半部分均值，差值 < -0.1
      expect(getTrend([2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])).toBe('down')
    })

    it('平稳序列应返回 stable', () => {
      const { getTrend } = useVitalityDecay()
      // 所有值相同，差值 = 0
      expect(getTrend([1.5, 1.5, 1.5, 1.5, 1.5, 1.5, 1.5])).toBe('stable')
    })

    it('超过 7 天的数据应只取最近 7 天', () => {
      const { getTrend } = useVitalityDecay()
      // 前 23 天稳定，最近 7 天递增 -> 应返回 up
      const data = Array(23).fill(1.0).concat([1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0])
      expect(getTrend(data)).toBe('up')
    })

    it('微小差异（<= 0.1）应返回 stable', () => {
      const { getTrend } = useVitalityDecay()
      // 差值恰好 <= 0.1，应视为稳定
      expect(getTrend([1.5, 1.5, 1.5, 1.55, 1.55, 1.55, 1.55])).toBe('stable')
    })
  })

  // --- getTrendIcon 图标类名 ---
  describe('getTrendIcon 图标类名', () => {
    it('上升趋势应返回 arrow-up 图标', () => {
      const { getTrendIcon } = useVitalityDecay()
      expect(getTrendIcon([1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0])).toBe('i-carbon-arrow-up')
    })

    it('下降趋势应返回 arrow-down 图标', () => {
      const { getTrendIcon } = useVitalityDecay()
      expect(getTrendIcon([2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])).toBe('i-carbon-arrow-down')
    })

    it('稳定趋势应返回 subtract 图标', () => {
      const { getTrendIcon } = useVitalityDecay()
      expect(getTrendIcon([1.5, 1.5, 1.5, 1.5])).toBe('i-carbon-subtract')
    })
  })

  // --- getTrendColorClass 趋势颜色 ---
  describe('getTrendColorClass 趋势颜色', () => {
    it('上升应为 text-green-500', () => {
      const { getTrendColorClass } = useVitalityDecay()
      expect(getTrendColorClass([1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0])).toBe('text-green-500')
    })

    it('下降应为 text-red-500', () => {
      const { getTrendColorClass } = useVitalityDecay()
      expect(getTrendColorClass([2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])).toBe('text-red-500')
    })

    it('稳定应为 text-gray-400', () => {
      const { getTrendColorClass } = useVitalityDecay()
      expect(getTrendColorClass([1.5, 1.5, 1.5, 1.5])).toBe('text-gray-400')
    })
  })

  // --- 边界值组合 ---
  describe('边界值组合', () => {
    it('score 恰好为 2.0 的边界行为应一致', () => {
      const { getLevel, getColor, getColorClass, getDescription } = useVitalityDecay()
      // 2.0 在 medium 范围内（>= 1.0 且 <= 2.0）
      expect(getLevel(2.0)).toBe('medium')
      expect(getColor(2.0)).toBe('#eab308')
      expect(getColorClass(2.0)).toBe('text-yellow-500')
      expect(getDescription(2.0)).toBe('一般')
    })

    it('score 恰好为 1.0 的边界行为应一致', () => {
      const { getLevel, getColor, getColorClass, getDescription } = useVitalityDecay()
      // 1.0 在 medium 范围内
      expect(getLevel(1.0)).toBe('medium')
      expect(getColor(1.0)).toBe('#eab308')
      expect(getColorClass(1.0)).toBe('text-yellow-500')
      expect(getDescription(1.0)).toBe('一般')
    })

    it('score 为 0 的极端情况', () => {
      const { getLevel, getColor, getPercentage, formatScore } = useVitalityDecay()
      expect(getLevel(0)).toBe('low')
      expect(getColor(0)).toBe('#ef4444')
      expect(getPercentage(0)).toBe(0)
      expect(formatScore(0)).toBe('0.0')
    })
  })
})
