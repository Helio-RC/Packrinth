/**
 * `plugin:ai_workshop|` 插件的 invoke 封装。
 * 所有调用失败时统一抛出带 message 的 `Error`。
 */
import { type Channel, invoke } from '@tauri-apps/api/core'

import type {
	AiStatus,
	AiWorkshopConfig,
	ChatResult,
	KnowledgeHit,
	SkillInfo,
	StreamEvent,
} from './types'

function toErrorMessage(err: unknown): string {
	return err instanceof Error ? err.message : String(err)
}

/** 单轮非流式对话，返回 `{ reply, usage }`。 */
export async function aiChat(conversationId: string, content: string): Promise<ChatResult> {
	try {
		return await invoke<ChatResult>('plugin:ai_workshop|ai_chat', { conversationId, content })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 流式多轮对话，事件经 Tauri Channel 推送给 `onEvent`。 */
export async function aiStream(
	conversationId: string,
	content: string,
	onEvent: (event: StreamEvent) => void,
): Promise<void> {
	const channel = new Channel<StreamEvent>()
	channel.onmessage = onEvent
	try {
		await invoke<void>('plugin:ai_workshop|ai_stream', {
			conversationId,
			content,
			onEvent: channel,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 记录用户对某次工具调用的确认结果。 */
export async function aiConfirmTool(
	conversationId: string,
	toolCallId: string,
	approved: boolean,
): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|ai_confirm_tool', {
			conversationId,
			toolCallId,
			approved,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 获取 AI 工作台运行状态摘要。 */
export async function getAiStatus(): Promise<AiStatus> {
	try {
		return await invoke<AiStatus>('plugin:ai_workshop|get_ai_status')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 获取 AI 工作台完整配置。 */
export async function getAiConfig(): Promise<AiWorkshopConfig> {
	try {
		return await invoke<AiWorkshopConfig>('plugin:ai_workshop|get_ai_config')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 保存 AI 工作台配置。 */
export async function setAiConfig(config: AiWorkshopConfig): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|set_ai_config', { config })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 分析崩溃日志（当前返回原始日志，AI 分析由高级场景层实现）。 */
export async function analyzeCrash(instanceId?: string): Promise<unknown> {
	try {
		return await invoke<unknown>('plugin:ai_workshop|analyze_crash', { instanceId })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 获取日志缓冲区内容（供 AI 分析）。 */
export async function getLogsForAi(limit?: number): Promise<string[]> {
	try {
		return await invoke<string[]>('plugin:ai_workshop|get_logs_for_ai', { limit })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 生成修复建议。 */
export async function suggestFix(crashLog?: string): Promise<unknown> {
	try {
		return await invoke<unknown>('plugin:ai_workshop|suggest_fix', { crashLog })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 应用修复建议。 */
export async function applyFix(fixId: string): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|apply_fix', { fixId })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 仅测试用：向日志缓冲区注入崩溃日志。后端命令带 `#[cfg(debug_assertions)]`，仅开发构建可用。 */
export async function injectCrashLog(logContent: string): Promise<void> {
	if (!import.meta.env.DEV) {
		throw new Error('仅开发构建可用')
	}
	try {
		await invoke<void>('plugin:ai_workshop|inject_crash_log', { logContent })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 列出全部技能（含启用状态）。 */
export async function listSkills(): Promise<SkillInfo[]> {
	try {
		return await invoke<SkillInfo[]>('plugin:ai_workshop|list_skills')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 启用技能。 */
export async function enableSkill(skillName: string): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|enable_skill', { skillName })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 禁用技能。 */
export async function disableSkill(skillName: string): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|disable_skill', { skillName })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 重新扫描技能目录，返回加载失败的技能名列表。 */
export async function refreshSkills(): Promise<string[]> {
	try {
		return await invoke<string[]>('plugin:ai_workshop|refresh_skills')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** BM25 知识检索。 */
export async function searchKnowledge(
	query: string,
	topK?: number,
	source?: string,
): Promise<KnowledgeHit[]> {
	try {
		return await invoke<KnowledgeHit[]>('plugin:ai_workshop|search_knowledge', {
			query,
			topK,
			source,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 手动刷新知识索引。 */
export async function refreshKnowledge(): Promise<void> {
	try {
		await invoke<void>('plugin:ai_workshop|refresh_knowledge')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}
