import { beforeEach, describe, expect, it, vi } from 'vitest'

import { cancelTask, executeTool, listenToolProgress, listTools } from '@/lib/ai/tools'

const invokeMock = vi.fn()
const listenMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
	invoke: (...args: unknown[]) => invokeMock(...args),
}))

vi.mock('@tauri-apps/api/event', () => ({
	listen: (...args: unknown[]) => listenMock(...args),
}))

describe('lib/ai/tools', () => {
	beforeEach(() => {
		invokeMock.mockReset()
		listenMock.mockReset()
	})

	it('executeTool 调用 tool_execute 并透传 name/params', async () => {
		invokeMock.mockResolvedValue({ success: true, taskId: 't-1' })
		const result = await executeTool('search_mods', { query: 'JEI' })
		expect(result.success).toBe(true)
		expect(invokeMock).toHaveBeenCalledWith('tool_execute', {
			name: 'search_mods',
			params: { query: 'JEI' },
		})
	})

	it('cancelTask 调用 cancel_task 并透传 taskId', async () => {
		invokeMock.mockResolvedValue(undefined)
		await cancelTask('t-1')
		expect(invokeMock).toHaveBeenCalledWith('cancel_task', {
			taskId: 't-1',
		})
	})

	it('listTools 返回工具列表', async () => {
		invokeMock.mockResolvedValue([{ name: 'search_mods', requiresConfirmation: false }])
		const tools = await listTools()
		expect(tools.length).toBe(1)
		expect(tools[0]?.name).toBe('search_mods')
	})

	it('listenToolProgress 订阅 tool-progress 并转发 payload', async () => {
		const unsubscribe = vi.fn()
		listenMock.mockImplementation((_event, handler) => {
			handler({ payload: { step: 'install', percent: 10 } })
			return Promise.resolve(unsubscribe)
		})
		const cb = vi.fn()
		const result = await listenToolProgress(cb)
		expect(listenMock).toHaveBeenCalledWith('tool-progress', expect.any(Function))
		expect(cb).toHaveBeenCalledWith({ step: 'install', percent: 10 })
		expect(result).toBe(unsubscribe)
	})

	it('invoke 失败转换为 Error', async () => {
		invokeMock.mockRejectedValue('no-backend')
		await expect(executeTool('x', {})).rejects.toThrow('no-backend')
	})
})
