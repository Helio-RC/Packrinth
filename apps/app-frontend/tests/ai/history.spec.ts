import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
	createConversation,
	deleteConversation,
	getConversation,
	listConversations,
	renameConversation,
} from '@/lib/ai/history'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
	invoke: (...args: unknown[]) => invokeMock(...args),
}))

describe('lib/ai/history', () => {
	beforeEach(() => {
		invokeMock.mockReset()
	})

	it('listConversations 传递分页参数（默认 50/0）', async () => {
		invokeMock.mockResolvedValue([])
		await listConversations()
		expect(invokeMock).toHaveBeenCalledWith('list_conversations', {
			instanceId: undefined,
			limit: 50,
			offset: 0,
		})
	})

	it('listConversations 支持显式 instanceId/limit/offset', async () => {
		invokeMock.mockResolvedValue([])
		await listConversations('inst-1', 10, 20)
		expect(invokeMock).toHaveBeenCalledWith('list_conversations', {
			instanceId: 'inst-1',
			limit: 10,
			offset: 20,
		})
	})

	it('getConversation 传递 conversationId 与分页参数', async () => {
		invokeMock.mockResolvedValue(null)
		const result = await getConversation('conv-1', 25, 50)
		expect(result).toBeNull()
		expect(invokeMock).toHaveBeenCalledWith('get_conversation', {
			conversationId: 'conv-1',
			limit: 25,
			offset: 50,
		})
	})

	it('createConversation / rename / delete 参数正确', async () => {
		invokeMock.mockResolvedValueOnce({ id: 'c1' })
		await createConversation('新会话', 'inst-2')
		expect(invokeMock).toHaveBeenCalledWith('create_conversation', {
			title: '新会话',
			instanceId: 'inst-2',
		})

		await renameConversation('c1', '新标题')
		expect(invokeMock).toHaveBeenCalledWith('rename_conversation', {
			conversationId: 'c1',
			newTitle: '新标题',
		})

		await deleteConversation('c1')
		expect(invokeMock).toHaveBeenCalledWith('delete_conversation', {
			conversationId: 'c1',
		})
	})

	it('invoke 抛错时统一转换为 Error 透传 message', async () => {
		invokeMock.mockRejectedValue('backend-exploded')
		await expect(listConversations()).rejects.toThrow('backend-exploded')
	})
})
