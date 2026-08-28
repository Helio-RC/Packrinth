/** AI 对话会话。 */
export interface Conversation {
	id: string
	title: string
	instanceId: string | null
	createdAt: number
	updatedAt: number
}

/** 会话中的单条消息。 */
export interface Message {
	id: string
	conversationId: string
	role: 'user' | 'assistant' | 'tool'
	content: string
	toolCalls: string | null
	toolCallId: string | null
	createdAt: number
}

/** 模型发起的工具调用。 */
export interface ToolCall {
	id: string
	name: string
	arguments: Record<string, unknown>
}

/** 流式对话事件（对应后端 `StreamEvent`）。 */
export interface StreamEvent {
	delta: string | null
	toolCalls: ToolCall[] | null
	usage: { promptTokens: number; completionTokens: number; totalTokens: number } | null
	done: boolean
	error: string | null
}

/** 已注册工具的描述信息。 */
export interface ToolInfo {
	name: string
	description: string
	domain: string
	requiresConfirmation: boolean
	isReadonly: boolean
	paramsSchema: unknown
}

/** 工具执行响应。 */
export interface ToolResponse<T = unknown> {
	success: boolean
	data?: T
	error?: { code: string; message: string; details?: unknown }
	toolCallId?: string
	taskId?: string
}

/** 技能描述信息。 */
export interface SkillInfo {
	name: string
	description: string
	keywords: string[]
	priority: number
	version: string
	author: string
	enabled: boolean
}

/** 加载失败的技能条目（整个技能被跳过，见 §7.4）。 */
export interface FailedSkill {
	dirName: string
	reason: string
}

/** list_skills 响应：技能列表 + 加载失败清单。 */
export interface SkillsListResponse {
	skills: SkillInfo[]
	failed: FailedSkill[]
}

/** 知识检索命中结果。 */
export interface KnowledgeHit {
	title: string
	snippet: string
	score: number
	source: string
}

/** AI 工作台运行状态摘要。 */
export interface AiStatus {
	enabled: boolean
	mockEnabled: boolean
	defaultProvider: string | null
	providerConfigured: boolean
	skillCount: number
	conversationCount: number
	logBufferCapacity: number
}

/** 单个 AI 提供商的配置。 */
export interface ProviderConfig {
	apiKeyHint: string | null
	model: string
	baseUrl: string | null
	enabled: boolean
}

/** AI 工作台完整配置（对应后端 `AiWorkshopConfig`）。 */
export interface AiWorkshopConfig {
	enabled: boolean
	logLines: number
	logFlushIntervalSecs: number
	mockEnabled: boolean
	autoTroubleshoot: boolean
	maxToolIterations: number
	tokenWarningThreshold: number
	defaultProvider: string | null
	providers: Record<string, ProviderConfig>
	knowledge: { allowedDomains: string[] }
	skills: { autoLoad: boolean; maxInjectCount: number }
	mcp: { enabled: boolean; command: string; args: string[]; healthCheckIntervalSecs: number }
	chatHistory: { maxConversationsPerInstance: number; retentionDays: number }
	layout: {
		activitybarPosition: 'left' | 'right'
		sidebarWidth: number
		bottomPanelHeight: number
		splitRatio: number
	}
}

/** 工具任务进度事件。 */
export interface ProgressPayload {
	taskId: string
	step: string
	percent: number | null
	message: string | null
}

/** 工具执行输出（工具面板展示用）。 */
export interface ToolOutput {
	taskId: string
	name: string
	params: unknown
	status: 'running' | 'success' | 'error' | 'cancelled'
	result?: unknown
	error?: string
	progress?: ProgressPayload
	startedAt: number
	finishedAt?: number
}

/** 单轮对话结果。 */
export interface ChatResult {
	reply: string
	usage: { promptTokens: number; completionTokens: number; totalTokens: number }
}
