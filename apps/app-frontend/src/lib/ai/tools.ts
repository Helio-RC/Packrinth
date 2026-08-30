/**
 * AI 工作台工具执行相关封装。
 * 所有调用失败时统一抛出带 message 的 `Error`。
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { ProgressPayload, ToolInfo, ToolResponse } from './types'

function toErrorMessage(err: unknown): string {
	return err instanceof Error ? err.message : String(err)
}

/** 手动执行工具（供前端工具面板调用），返回 ToolResponse（含 task_id 便于取消）。 */
export async function executeTool(name: string, params: unknown): Promise<ToolResponse> {
	try {
		return await invoke<ToolResponse>('tool_execute', { name, params })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 通过 task_id 取消进行中的工具任务。 */
export async function cancelTask(taskId: string): Promise<void> {
	try {
		await invoke<void>('cancel_task', { taskId })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 列出全部已注册工具（含参数 Schema）。 */
export async function listTools(): Promise<ToolInfo[]> {
	try {
		return await invoke<ToolInfo[]>('list_tools')
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 获取单个工具的 JSON Schema（供前端动态渲染表单）。 */
export async function getToolSchema(name: string): Promise<unknown> {
	try {
		return await invoke<unknown>('get_tool_schema', { name })
	} catch (err) {
		throw new Error(toErrorMessage(err))
	}
}

/** 订阅工具任务进度事件，返回取消订阅函数。 */
export async function listenToolProgress(
	cb: (payload: ProgressPayload) => void,
): Promise<UnlistenFn> {
	return listen<ProgressPayload>('tool-progress', (event) => cb(event.payload))
}
