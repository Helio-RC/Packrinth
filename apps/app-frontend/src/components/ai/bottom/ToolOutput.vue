<template>
	<div class="flex h-full w-full flex-col overflow-y-auto bg-bg-raised p-3">
		<div
			v-if="store.toolOutputs.length === 0"
			class="flex flex-1 flex-col items-center justify-center gap-2 p-4 text-center"
		>
			<p class="text-sm text-secondary">{{ formatMessage(messages.empty) }}</p>
		</div>

		<div v-else class="flex flex-col gap-2">
			<div
				v-for="output in store.toolOutputs"
				:key="output.taskId || `${output.name}-${output.startedAt}`"
				class="rounded-lg border border-divider bg-bg p-3"
			>
				<div class="flex items-center justify-between gap-2">
					<span class="truncate text-sm font-medium text-contrast">{{ output.name }}</span>
					<span
						class="shrink-0 rounded-full px-2 py-0.5 text-xs"
						:class="statusBadge(output.status)"
					>
						{{ statusLabel(output.status) }}
					</span>
				</div>

				<div v-if="paramSummary(output.params)" class="mt-2">
					<p class="text-xs text-secondary">{{ formatMessage(messages.params) }}</p>
					<pre
						class="mt-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-bg-raised p-2 font-mono text-xs text-primary"
						>{{ paramSummary(output.params) }}</pre
					>
				</div>

				<div
					v-if="output.progress && progressPercent(output) !== null"
					class="mt-2 flex flex-col gap-1"
				>
					<div class="flex items-center justify-between gap-2 text-xs">
						<span class="truncate text-secondary">
							{{ output.progress.step }}
							<span v-if="output.progress.message">— {{ output.progress.message }}</span>
						</span>
						<span class="shrink-0 text-secondary tabular-nums">
							{{ progressPercent(output) }}%
						</span>
					</div>
					<div class="h-1.5 w-full overflow-hidden rounded-full bg-bg-raised">
						<div
							class="h-full rounded-full bg-brand transition-all"
							:style="{ width: `${progressPercent(output)}%` }"
						/>
					</div>
				</div>

				<div v-if="output.result !== undefined" class="mt-2">
					<p class="text-xs text-secondary">{{ formatMessage(messages.result) }}</p>
					<pre
						class="mt-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-bg-raised p-2 font-mono text-xs text-green-500"
						>{{ resultText(output.result) }}</pre
					>
				</div>

				<div v-if="output.error" class="mt-2">
					<p class="text-xs text-secondary">{{ formatMessage(messages.error) }}</p>
					<pre
						class="mt-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-bg-raised p-2 font-mono text-xs text-red-500"
						>{{ output.error }}</pre
					>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

import type { ToolOutput } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiToolOutput',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	empty: {
		id: 'ai.tool-output.empty',
		defaultMessage: '暂无工具输出',
	},
	params: {
		id: 'ai.tool-output.params',
		defaultMessage: '参数',
	},
	result: {
		id: 'ai.tool-output.result',
		defaultMessage: '结果',
	},
	error: {
		id: 'ai.tool-output.error',
		defaultMessage: '错误',
	},
	statusRunning: {
		id: 'ai.tool-output.status-running',
		defaultMessage: '运行中',
	},
	statusSuccess: {
		id: 'ai.tool-output.status-success',
		defaultMessage: '成功',
	},
	statusError: {
		id: 'ai.tool-output.status-error',
		defaultMessage: '失败',
	},
	statusCancelled: {
		id: 'ai.tool-output.status-cancelled',
		defaultMessage: '已取消',
	},
})

const statusBadge = (status: ToolOutput['status']) => {
	switch (status) {
		case 'running':
			return 'bg-blue-500/15 text-blue-500'
		case 'success':
			return 'bg-green-500/15 text-green-500'
		case 'error':
			return 'bg-red-500/15 text-red-500'
		case 'cancelled':
			return 'bg-gray-500/15 text-gray-500'
	}
}

const statusLabel = (status: ToolOutput['status']) => {
	switch (status) {
		case 'running':
			return formatMessage(messages.statusRunning)
		case 'success':
			return formatMessage(messages.statusSuccess)
		case 'error':
			return formatMessage(messages.statusError)
		case 'cancelled':
			return formatMessage(messages.statusCancelled)
	}
}

const paramSummary = (params: unknown) => {
	if (params === undefined || params === null || params === '') return ''
	const json = typeof params === 'string' ? params : JSON.stringify(params)
	return json.length > 160 ? `${json.slice(0, 160)}…` : json
}

const resultText = (result: unknown) => {
	if (result === null || result === undefined) return ''
	if (typeof result === 'string') return result
	try {
		return JSON.stringify(result, null, 2)
	} catch {
		return String(result)
	}
}

const progressPercent = (output: ToolOutput) => {
	const percent = output.progress?.percent
	if (percent === null || percent === undefined) return null
	return Math.min(Math.max(percent, 0), 100)
}
</script>