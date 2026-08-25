/**
 * 预留的 WebSocket 客户端。
 * 未来用于 MCP 远程连接与远程流式对话；当前流式对话使用 Tauri Channel（见 `client.ts` 的 `aiStream`）。
 */
export class AiWebSocket {
	/** 建立连接（预留，暂未实现）。 */
	async connect(): Promise<void> {
		// 预留：未来 MCP/远程流式
	}

	/** 断开连接（预留，暂未实现）。 */
	async disconnect(): Promise<void> {
		// 预留：未来 MCP/远程流式
	}
}
