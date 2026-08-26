<template>
	<div class="flex h-full w-full flex-col gap-3 overflow-y-auto bg-bg-raised p-4">
		<p class="text-sm text-secondary">{{ formatMessage(messages.placeholder) }}</p>

		<div class="flex gap-2">
			<button
				class="cursor-pointer rounded-lg border border-divider bg-bg px-3 py-1.5 text-sm font-medium text-secondary transition-colors hover:text-contrast"
				:disabled="loading"
				@click="analyze"
			>
				{{ loading ? formatMessage(messages.running) : formatMessage(messages.analyze) }}
			</button>
			<button
				class="cursor-pointer rounded-lg border border-divider bg-bg px-3 py-1.5 text-sm font-medium text-secondary"
				disabled
			>
				{{ formatMessage(messages.fix) }}
			</button>
		</div>

		<div
			v-if="error"
			class="rounded-lg border border-red-400/40 bg-red-400/10 px-3 py-2 text-sm text-red-300"
			role="alert"
		>
			{{ error }}
		</div>

		<div v-if="report" class="rounded-lg border border-divider bg-bg p-3">
			<div class="flex gap-6 text-sm">
				<span class="text-secondary">{{ formatMessage(messages.logLines) }}</span>
				<span class="font-medium text-contrast tabular-nums">{{ report.logLines }}</span>
			</div>
			<div class="mt-2 flex gap-6 text-sm">
				<span class="text-secondary">{{ formatMessage(messages.analysis) }}</span>
				<span class="font-medium text-secondary">
					{{ report.analysis ?? formatMessage(messages.analysisEmpty) }}
				</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiTroubleshootReport',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	placeholder: {
		id: 'ai.troubleshoot.placeholder',
		defaultMessage: '自动排障将在流 E 提供。当前可手动分析崩溃日志并查看后端返回的原始结果。',
	},
	analyze: {
		id: 'ai.troubleshoot.analyze',
		defaultMessage: '分析崩溃',
	},
	fix: {
		id: 'ai.troubleshoot.fix',
		defaultMessage: '获取修复建议',
	},
	running: {
		id: 'ai.troubleshoot.running',
		defaultMessage: '分析中…',
	},
	logLines: {
		id: 'ai.troubleshoot.log-lines',
		defaultMessage: '日志行数',
	},
	analysis: {
		id: 'ai.troubleshoot.analysis',
		defaultMessage: '分析',
	},
	analysisEmpty: {
		id: 'ai.troubleshoot.analysis-empty',
		defaultMessage: '（后端当前返回 null）',
	},
})

interface CrashReport {
	logLines: number
	analysis: string | null
}

const loading = ref(false)
const error = ref<string | null>(null)
const report = ref<CrashReport | null>(null)

const analyze = async () => {
	loading.value = true
	error.value = null
	try {
		const data = await store.runTroubleshoot()
		const record = (data ?? {}) as { log_lines?: unknown; analysis?: unknown }
		report.value = {
			logLines: Array.isArray(record.log_lines) ? record.log_lines.length : 0,
			analysis:
				record.analysis === null || record.analysis === undefined
					? null
					: String(record.analysis),
		}
	} catch (err) {
		error.value = err instanceof Error ? err.message : String(err)
	} finally {
		loading.value = false
	}
}
</script>