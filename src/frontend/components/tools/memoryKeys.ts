/**
 * 记忆管理器 provide/inject 注入键
 *
 * 从 MemoryManager.vue 的 <script setup> 中提取，
 * 因为 <script setup> 不支持 ES module export。
 * 子组件（MemoryWorkspace、TagFilter 等）通过此文件导入注入键。
 */
import type { InjectionKey, Ref } from 'vue'

/** 当前选中的域路径 */
export const MEMORY_DOMAIN_KEY: InjectionKey<Ref<string | null>> = Symbol('memoryDomain')

/** 搜索关键词 */
export const MEMORY_SEARCH_KEY: InjectionKey<Ref<string>> = Symbol('memorySearch')

/** 当前选中的标签筛选 */
export const MEMORY_TAGS_KEY: InjectionKey<Ref<string[]>> = Symbol('memoryTags')

/** 是否处于多选模式 */
export const MEMORY_BATCH_MODE_KEY: InjectionKey<Ref<boolean>> = Symbol('memoryBatchMode')

/** 多选选中的记忆 ID 列表 */
export const MEMORY_SELECTED_IDS_KEY: InjectionKey<Ref<string[]>> = Symbol('memorySelectedIds')
