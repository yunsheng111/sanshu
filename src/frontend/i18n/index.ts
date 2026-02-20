// HC-13: Vue I18n 配置
import { createI18n } from 'vue-i18n'
import en from './en'
import zh from './zh'

export type MessageSchema = typeof zh

export const i18n = createI18n<[MessageSchema], 'zh' | 'en'>({
  legacy: false, // 使用 Composition API 模式
  locale: 'zh', // 默认语言
  fallbackLocale: 'en', // 回退语言
  messages: {
    zh,
    en,
  },
})

export default i18n
