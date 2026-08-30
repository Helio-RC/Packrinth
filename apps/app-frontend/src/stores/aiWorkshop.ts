/**
 * AI 工作台 Pinia store：对话、工具、技能、知识、日志与布局状态管理。
 */
import type { UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'

import { toError } from '@/helpers/errors'
import {
	aiConfirmTool,
	aiStream,
	analyzeCrash,
	applyFix as applyFixRequest,
	disableSkill,
	enableSkill,
	getAiConfig,
	getAiStatus,
	getLogsForAi,
	listSkills,
	refreshKnowledge as refreshKnowledgeRequest,
	refreshSkills as refreshSkillsRequest,
	searchKnowledge as searchKnowledgeRequest,
	setAiConfig,
} from '@/lib/ai/client'
import {
	clearAllConversations,
	createConversation,
	deleteConversation,
	getConversation,
	listConversations,
	renameConversation as renameConversationRequest,
} from '@/lib/ai/history'
import {
	cancelTask,
	executeTool as executeToolRequest,
	listenToolProgress,
	listTools,
} from '@/lib/ai/tools'
import type {
	AiStatus,
	AiWorkshopConfig,
	Conversation,
	FailedSkill,
	KnowledgeHit,
	Message,
	SkillInfo,
	ToolInfo,
	ToolOutput,
} from '@/lib/ai/types'

/** 活动栏可切换的面板标识。 */
export type ActivityId =
	| 'chat'
	| 'files'
	| 'knowledge'
	| 'skills'
	| 'tools'
	| 'console'
	| 'settings'

/** AI 工作台布局状态（前四项与后端 `LayoutConfig` 对应，可见性标志仅存前端）。 */
export interface WorkshopLayout {
	activitybarPosition: 'left' | 'right'
	sidebarWidth: number
	bottomPanelHeight: number
	splitRatio: number
	sidebarVisible: boolean
	bottomVisible: boolean
	/** 主区域视图序列（chat / preview），支持拆分/合并/移动。 */
	mainViews: string[]
	/** 活动栏条目顺序（可拖拽调整）。 */
	activityOrder: string[]
}

/** 布局持久化使用的 localStorage 键。 */
const LAYOUT_STORAGE_KEY = 'ai-workshop-layout'

/** 消息分页大小。 */
const MESSAGE_PAGE_SIZE = 50

/** 工厂默认布局。 */
const FACTORY_LAYOUT: WorkshopLayout = {
	activitybarPosition: 'left',
	sidebarWidth: 280,
	bottomPanelHeight: 220,
	splitRatio: 0.6,
	sidebarVisible: true,
	bottomVisible: true,
	mainViews: ['chat', 'preview'],
	activityOrder: ['chat', 'files', 'knowledge', 'skills', 'tools', 'console', 'settings'],
}

/** 工具进度事件监听器（模块级单例，避免重复注册）。 */
let progressUnlisten: UnlistenFn | null = null

/** 本地临时消息 id 计数器，避免同毫秒内 id 冲突。 */
let localIdCounter = 0

function nextLocalId(): string {
	localIdCounter += 1
	return `local-${Date.now()}-${localIdCounter}`
}

export const useAiWorkshopStore = defineStore('aiWorkshop', {
	state: () => ({
		layout: { ...FACTORY_LAYOUT },
		activeActivity: 'chat' as ActivityId,
		/** 预览面板：当前编辑内容与 Diff 前后快照（由工具视图/对话写入）。 */
		previewText: '' as string,
		previewFileName: '' as string,
		previewLanguage: 'json' as string,
		previewBefore: '' as string,
		previewAfter: '' as string,
		conversations: [] as Conversation[],
		currentConversationId: null as string | null,
		messages: [] as Message[],
		messagesOffset: 0,
		hasMoreMessages: false,
		streaming: false,
		tokenUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
		aiStatus: null as AiStatus | null,
		aiConfig: null as AiWorkshopConfig | null,
		/** init() 各步骤的错误（键: status/tools/conversations/config），供 UI 展示真实原因。 */
		initErrors: {} as Record<string, string>,
		providerConfigured: false,
		tools: [] as ToolInfo[],
		toolOutputs: [] as ToolOutput[],
		skills: [] as SkillInfo[],
		failedSkills: [] as FailedSkill[],
		knowledgeResults: [] as KnowledgeHit[],
		logs: [] as string[],
	}),
	getters: {
		currentConversation(state): Conversation | null {
			return state.conversations.find((c) => c.id === state.currentConversationId) ?? null
		},
		totalTokens(state): number {
			return state.tokenUsage.totalTokens
		},
	},
	actions: {
		/** 初始化：并行加载状态/工具/会话，恢复布局并注册工具进度监听。
		 *  单项失败不阻塞其余（记录 initErrors 供 UI 展示真实原因）。 */
		async init() {
			const safe = async <T>(key: string, fn: () => Promise<T>): Promise<T | undefined> => {
				try {
					return await fn()
				} catch (err) {
					this.initErrors[key] = toError(err).message
					return undefined
				}
			}
			const [status, tools, conversations, config] = await Promise.all([
				safe('status', getAiStatus),
				safe('tools', listTools),
				safe('conversations', listConversations),
				safe('config', () => this.loadConfig()),
			])
			if (status) {
				this.aiStatus = status
				this.providerConfigured = status.providerConfigured
			}
			if (tools) this.tools = tools
			if (conversations) this.conversations = conversations

			if (config) await this.loadLayout(config)

			if (!progressUnlisten) {
				progressUnlisten = await listenToolProgress((payload) => {
					const output = this.toolOutputs.find((o) => o.taskId === payload.taskId)
					if (output) output.progress = payload
				})
			}
		},

		/** 重新加载会话列表。 */
		async loadConversations() {
			this.conversations = await listConversations()
		},

		/** 加载指定会话的最近消息并切换为当前会话。 */
		async loadConversation(id: string) {
			const data = await getConversation(id)
			if (!data) return
			this.currentConversationId = id
			this.messages = data.messages
			this.messagesOffset = data.messages.length
			this.hasMoreMessages = data.messages.length >= MESSAGE_PAGE_SIZE
		},

		/** 加载更早的消息（分页，向前追加）。 */
		async loadMoreMessages() {
			if (!this.currentConversationId || !this.hasMoreMessages || this.streaming) return
			const data = await getConversation(
				this.currentConversationId,
				MESSAGE_PAGE_SIZE,
				this.messagesOffset,
			)
			if (!data) return
			this.messages = [...data.messages, ...this.messages]
			this.messagesOffset += data.messages.length
			this.hasMoreMessages = data.messages.length >= MESSAGE_PAGE_SIZE
		},

		/** 新建会话并切换。 */
		async newConversation() {
			const conversation = await createConversation('新对话')
			this.conversations.unshift(conversation)
			this.currentConversationId = conversation.id
			this.messages = []
			this.messagesOffset = 0
			this.hasMoreMessages = false
		},

		/** 重命名会话。 */
		async renameConversation(id: string, title: string) {
			await renameConversationRequest(id, title)
			const conversation = this.conversations.find((c) => c.id === id)
			if (conversation) conversation.title = title
		},

		/** 删除会话；若为当前会话则清空消息区。 */
		async removeConversation(id: string) {
			await deleteConversation(id)
			this.conversations = this.conversations.filter((c) => c.id !== id)
			if (this.currentConversationId === id) {
				this.currentConversationId = null
				this.messages = []
				this.messagesOffset = 0
				this.hasMoreMessages = false
			}
		},

		/** 清空全部会话并刷新列表。 */
		async clearAll() {
			await clearAllConversations(true)
			await this.loadConversations()
			this.currentConversationId = null
			this.messages = []
			this.messagesOffset = 0
			this.hasMoreMessages = false
		},

		/** 发送用户消息并流式接收回复。 */
		async sendMessage(content: string) {
			if (!this.currentConversationId || this.streaming) return
			const conversationId = this.currentConversationId
			this.messages.push({
				id: nextLocalId(),
				conversationId,
				role: 'user',
				content,
				toolCalls: null,
				toolCallId: null,
				createdAt: Date.now(),
			})
			this.streaming = true
			try {
				await aiStream(conversationId, content, (event) => {
					if (event.delta) {
						const last = this.messages[this.messages.length - 1]
						if (last && last.role === 'assistant') {
							last.content += event.delta
						} else {
							this.messages.push({
								id: nextLocalId(),
								conversationId,
								role: 'assistant',
								content: event.delta,
								toolCalls: null,
								toolCallId: null,
								createdAt: Date.now(),
							})
						}
					}
					if (event.toolCalls) {
						for (const call of event.toolCalls) {
							this.messages.push({
								id: nextLocalId(),
								conversationId,
								role: 'tool',
								content: '',
								toolCalls: JSON.stringify(call.arguments),
								toolCallId: call.id,
								createdAt: Date.now(),
							})
						}
					}
					if (event.usage) {
						this.tokenUsage = event.usage
					}
					if (event.error) {
						this.messages.push({
							id: nextLocalId(),
							conversationId,
							role: 'assistant',
							content: `Error: ${event.error}`,
							toolCalls: null,
							toolCallId: null,
							createdAt: Date.now(),
						})
					}
					if (event.done) {
						this.streaming = false
						void this.loadConversations()
					}
				})
			} catch (err) {
				this.messages.push({
					id: nextLocalId(),
					conversationId,
					role: 'assistant',
					content: `Error: ${toError(err).message}`,
					toolCalls: null,
					toolCallId: null,
					createdAt: Date.now(),
				})
				this.streaming = false
			}
		},

		/** 确认/拒绝一次工具调用。 */
		async confirmTool(toolCallId: string, approved: boolean) {
			if (!this.currentConversationId) return
			await aiConfirmTool(this.currentConversationId, toolCallId, approved)
		},

		/** 手动执行工具并跟踪其输出与进度。 */
		async executeTool(name: string, params: unknown) {
			const output: ToolOutput = {
				taskId: '',
				name,
				params,
				status: 'running',
				startedAt: Date.now(),
			}
			this.toolOutputs.push(output)
			try {
				const response = await executeToolRequest(name, params)
				if (response.taskId) output.taskId = response.taskId
				if (response.success) {
					output.status = 'success'
					output.result = response.data
				} else {
					output.status = 'error'
					output.error = response.error?.message ?? 'Tool execution failed'
				}
			} catch (err) {
				output.status = 'error'
				output.error = toError(err).message
			} finally {
				output.finishedAt = Date.now()
			}
		},

		/** 取消进行中的工具任务。 */
		async cancelTool(taskId: string) {
			await cancelTask(taskId)
			const output = this.toolOutputs.find((o) => o.taskId === taskId)
			if (output) {
				output.status = 'cancelled'
				output.finishedAt = Date.now()
			}
		},

		/** 重新加载工具列表。 */
		async loadTools() {
			this.tools = await listTools()
		},

		/** 重新加载技能列表（含加载失败清单）。 */
		async loadSkills() {
			const response = await listSkills()
			this.skills = response.skills
			this.failedSkills = response.failed
		},

		/** 启用/禁用技能并刷新列表。 */
		async toggleSkill(name: string, enabled: boolean) {
			if (enabled) {
				await enableSkill(name)
			} else {
				await disableSkill(name)
			}
			await this.refreshSkills()
		},

		/** 重新扫描技能目录并刷新列表。 */
		async refreshSkills() {
			await refreshSkillsRequest()
			await this.loadSkills()
		},

		/** 知识检索。 */
		async searchKnowledge(query: string) {
			this.knowledgeResults = await searchKnowledgeRequest(query)
		},

		/** 手动刷新知识索引。 */
		async refreshKnowledge() {
			await refreshKnowledgeRequest()
		},

		/** 加载日志缓冲区内容。 */
		async loadLogs() {
			this.logs = await getLogsForAi()
		},

		/** 运行崩溃日志分析，返回分析结果。 */
		async runTroubleshoot() {
			return await analyzeCrash()
		},

		/** 应用修复建议。 */
		async applyFix(fixId: string) {
			await applyFixRequest(fixId)
		},

		/** 加载 AI 工作台完整配置。 */
		async loadConfig() {
			this.aiConfig = await getAiConfig()
		},

		/** 合并配置补丁并保存到后端；保存失败时回滚并重新抛出。 */
		async updateConfig(patch: Partial<AiWorkshopConfig>) {
			if (!this.aiConfig) await this.loadConfig()
			if (!this.aiConfig) throw new Error('AI 配置不可用')
			const previous = this.aiConfig
			const next = { ...this.aiConfig, ...patch }
			this.aiConfig = next
			try {
				await setAiConfig(next)
			} catch (err) {
				this.aiConfig = previous
				throw err
			}
		},

		/** 从 localStorage 恢复布局；无缓存时回退到传入的后端配置（未传入则拉取）。 */
		async loadLayout(config?: AiWorkshopConfig) {
			const saved = localStorage.getItem(LAYOUT_STORAGE_KEY)
			if (saved) {
				try {
					this.layout = { ...this.layout, ...(JSON.parse(saved) as Partial<WorkshopLayout>) }
					return
				} catch {
					// 缓存损坏时回退到后端配置
				}
			}
			const cfg = config ?? (await getAiConfig())
			this.layout = { ...this.layout, ...cfg.layout }
		},

		/** 将当前布局写入 localStorage。 */
		saveLayout() {
			localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(this.layout))
		},

		/** 从后端配置重载布局。 */
		async resetLayout() {
			const config = await getAiConfig()
			this.layout = { ...this.layout, ...config.layout }
		},

		/** 将当前布局保存为后端默认值。 */
		async saveLayoutAsDefault() {
			const config = await getAiConfig()
			config.layout = {
				activitybarPosition: this.layout.activitybarPosition,
				sidebarWidth: this.layout.sidebarWidth,
				bottomPanelHeight: this.layout.bottomPanelHeight,
				splitRatio: this.layout.splitRatio,
			}
			await setAiConfig(config)
		},

		/** 恢复工厂默认布局。 */
		restoreFactoryLayout() {
			this.layout = { ...FACTORY_LAYOUT }
		},

		/** 设置提供商是否已配置。 */
		setProviderConfigured(configured: boolean) {
			this.providerConfigured = configured
		},
	},
})
