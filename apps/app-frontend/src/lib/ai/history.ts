/**
 * AI 工作台会话历史相关封装。
 * 所有调用失败时统一抛出带 message 的 `Error`。
 */
import { invoke } from '@tauri-apps/api/core'

import type { Conversation, Message } from './types'

function toErrorMessage(err: unknown): string {
	return err instanceof Error ? err.message : String(err)
}

/** 列出会话，按 `updated_at` 降序分页。 */
export async function listConversations(
	instanceId?: string,
	limit = 50,
	offset = 0,
): Promise<Conversation[]> {
	try {
		return await invoke<Conversation[]>('list_conversations', {
			instanceId,
			limit,
			offset,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 获取单个会话及其消息，返回 `{ conversation, messages }`；会话不存在时返回 `null`。 */
export async function getConversation(
	conversationId: string,
	limit = 50,
	offset = 0,
): Promise<{ conversation: Conversation; messages: Message[] } | null> {
	try {
		return await invoke<{ conversation: Conversation; messages: Message[] } | null>(
			'get_conversation',
			{ conversationId, limit, offset },
		)
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 新建会话。 */
export async function createConversation(
	title: string,
	instanceId?: string,
): Promise<Conversation> {
	try {
		return await invoke<Conversation>('create_conversation', {
			title,
			instanceId,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 重命名会话。 */
export async function renameConversation(conversationId: string, newTitle: string): Promise<void> {
	try {
		await invoke<void>('rename_conversation', {
			conversationId,
			newTitle,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 删除会话及其全部消息。 */
export async function deleteConversation(conversationId: string): Promise<void> {
	try {
		await invoke<void>('delete_conversation', { conversationId })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 导出会话为 `json` 或 `markdown`。 */
export async function exportConversation(
	conversationId: string,
	format: 'json' | 'markdown',
): Promise<string> {
	try {
		return await invoke<string>('export_conversation', {
			conversationId,
			format,
		})
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 清空全部会话（需 `confirm=true`），返回删除的会话数。 */
export async function clearAllConversations(confirm: boolean): Promise<number> {
	try {
		return await invoke<number>('clear_all_conversations', { confirm })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}
