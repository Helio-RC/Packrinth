<template>
	<div class="flex flex-col h-full w-full bg-bg-raised">
		<div class="flex items-center justify-between px-3 py-1.5 shrink-0 border-b border-divider">
			<div class="flex items-center gap-1">
				<button
					class="text-primary hover:text-contrast cursor-pointer border-none bg-transparent p-1"
					:title="formatMessage(paused ? messages.resume : messages.pause)"
					@click="paused = !paused"
				>
					<PauseIcon v-if="!paused" />
					<PlayIcon v-else />
				</button>
				<button
					class="text-primary hover:text-contrast cursor-pointer border-none bg-transparent p-1"
					:title="formatMessage(messages.refresh)"
					@click="refresh"
				>
					<RefreshCwIcon />
				</button>
			</div>
			<span class="text-xs text-secondary tabular-nums">
				{{ store.logs.length }} {{ formatMessage(messages.lines) }}
			</span>
		</div>

		<div
			v-if="store.logs.length === 0"
			class="flex flex-1 flex-col items-center justify-center gap-2 p-4 text-center"
		>
			<p class="text-sm text-secondary">{{ formatMessage(messages.empty) }}</p>
			<button
				class="cursor-pointer border-none bg-transparent text-sm font-medium text-primary hover:text-contrast"
				@click="refresh"
			>
				{{ formatMessage(messages.refresh) }}
			</button>
		</div>

		<div
			v-else
			ref="scrollEl"
			class="flex-1 min-h-0 overflow-y-auto px-3 py-2 font-mono text-xs leading-5"
		>
			<div
				v-for="(line, index) in store.logs"
				:key="index"
				class="whitespace-pre-wrap break-words"
				:class="lineClass(line)"
			>
				{{ line }}
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { PauseIcon, PlayIcon, RefreshCwIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { nextTick, onMounted, ref, watch } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiLogViewer',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	empty: {
		id: 'ai.logs.empty',
		defaultMessage: '暂无日志',
	},
	refresh: {
		id: 'ai.logs.refresh',
		defaultMessage: '刷新日志',
	},
	pause: {
		id: 'ai.logs.pause',
		defaultMessage: '暂停自动滚动',
	},
	resume: {
		id: 'ai.logs.resume',
		defaultMessage: '恢复自动滚动',
	},
	lines: {
		id: 'ai.logs.lines',
		defaultMessage: '行',
	},
})

const scrollEl = ref<HTMLElement | null>(null)
const paused = ref(false)

const lineClass = (line: string) => {
	const lower = line.toLowerCase()
	if (lower.includes('error')) return 'text-red-500'
	if (lower.includes('warn')) return 'text-yellow-500'
	return 'text-secondary'
}

const refresh = () => {
	void store.loadLogs()
}

watch(
	() => store.logs.length,
	async () => {
		if (paused.value) return
		await nextTick()
		scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight })
	},
)

onMounted(() => {
	void store.loadLogs()
})
</script>