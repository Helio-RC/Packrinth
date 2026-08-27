<template>
	<div class="flex flex-col gap-4 h-full w-full overflow-y-auto p-4 bg-bg-raised">
		<div
			v-if="error"
			class="rounded-lg border border-red-400/40 bg-red-400/10 px-3 py-2 text-sm text-red-300"
			role="alert"
		>
			{{ error }}
		</div>

		<section>
			<h2 class="mb-2 text-xs font-semibold uppercase tracking-wide text-secondary">
				{{ formatMessage(messages.dashboardTitle) }}
			</h2>
			<div class="grid grid-cols-2 gap-2">
				<div class="rounded-lg bg-bg border border-divider p-3">
					<p class="text-xs text-secondary">{{ formatMessage(messages.mods) }}</p>
					<p class="mt-1 text-lg font-semibold text-contrast tabular-nums">--</p>
					<p class="mt-1 text-xs text-secondary">{{ formatMessage(messages.flowC) }}</p>
				</div>
				<div class="rounded-lg bg-bg border border-divider p-3">
					<p class="text-xs text-secondary">{{ formatMessage(messages.disk) }}</p>
					<p class="mt-1 text-lg font-semibold text-contrast tabular-nums">--</p>
					<p class="mt-1 text-xs text-secondary">{{ formatMessage(messages.flowC) }}</p>
				</div>
				<div class="rounded-lg bg-bg border border-divider p-3">
					<p class="text-xs text-secondary">{{ formatMessage(messages.gitBranch) }}</p>
					<p class="mt-1 text-lg font-semibold text-contrast tabular-nums">--</p>
					<p class="mt-1 text-xs text-secondary">{{ formatMessage(messages.flowC) }}</p>
				</div>
				<div class="rounded-lg bg-bg border border-divider p-3">
					<p class="text-xs text-secondary">{{ formatMessage(messages.conversations) }}</p>
					<p class="mt-1 text-lg font-semibold text-contrast tabular-nums">
						{{ store.aiStatus?.conversationCount ?? '--' }}
					</p>
				</div>
				<div class="rounded-lg bg-bg border border-divider p-3">
					<p class="text-xs text-secondary">{{ formatMessage(messages.skills) }}</p>
					<p class="mt-1 text-lg font-semibold text-contrast tabular-nums">
						{{ store.aiStatus?.skillCount ?? '--' }}
					</p>
				</div>
			</div>
		</section>

		<section v-if="store.aiConfig">
			<h2 class="mb-2 text-xs font-semibold uppercase tracking-wide text-secondary">
				{{ formatMessage(messages.togglesTitle) }}
			</h2>
			<div class="flex flex-col gap-2">
				<SettingsToggleCard
					:model-value="store.aiConfig.enabled"
					:title="formatMessage(messages.enabled)"
					:description="formatMessage(messages.enabledDesc)"
					@update:model-value="(v) => save({ enabled: v })"
				/>
				<SettingsToggleCard
					:model-value="store.aiConfig.mockEnabled"
					:title="formatMessage(messages.mock)"
					:description="formatMessage(messages.mockDesc)"
					@update:model-value="(v) => save({ mockEnabled: v })"
				/>
			</div>

			<label class="mt-4 flex flex-col gap-1">
				<span class="text-sm font-medium text-contrast">{{
					formatMessage(messages.logLines)
				}}</span>
				<span class="text-xs text-secondary">{{ formatMessage(messages.logLinesDesc) }}</span>
				<input
					v-model.number="logLines"
					type="number"
					min="100"
					class="mt-1 w-28 rounded-lg border border-divider bg-bg px-3 py-1.5 text-sm text-contrast outline-none focus:border-brand"
					@input="clampLogLines"
					@change="saveLogLines"
				/>
			</label>
		</section>

		<section>
			<h2 class="mb-2 text-xs font-semibold uppercase tracking-wide text-secondary">
				{{ formatMessage(messages.advancedTitle) }}
			</h2>
			<SettingsToggleCard
				:model-value="false"
				:title="formatMessage(messages.troubleshoot)"
				:description="formatMessage(messages.troubleshootDesc)"
				:toggle-disabled="true"
				disabled
			/>
		</section>
	</div>
</template>

<script setup lang="ts">
import { defineMessages, SettingsToggleCard, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import type { AiWorkshopConfig } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiConsoleView',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	dashboardTitle: {
		id: 'ai.console.dashboard-title',
		defaultMessage: '数据仪表盘',
	},
	mods: {
		id: 'ai.console.mods',
		defaultMessage: 'Mod 数量',
	},
	disk: {
		id: 'ai.console.disk',
		defaultMessage: '磁盘占用',
	},
	gitBranch: {
		id: 'ai.console.git-branch',
		defaultMessage: 'Git 分支',
	},
	conversations: {
		id: 'ai.console.conversations',
		defaultMessage: '会话数',
	},
	skills: {
		id: 'ai.console.skills',
		defaultMessage: '技能数',
	},
	flowC: {
		id: 'ai.console.flow-c',
		defaultMessage: '由流 C 提供',
	},
	togglesTitle: {
		id: 'ai.console.toggles-title',
		defaultMessage: '核心开关',
	},
	enabled: {
		id: 'ai.console.enabled',
		defaultMessage: 'AI 主开关',
	},
	enabledDesc: {
		id: 'ai.console.enabled-desc',
		defaultMessage: '启用或停用 AI 工作台功能。',
	},
	mock: {
		id: 'ai.console.mock',
		defaultMessage: 'Mock 模式',
	},
	mockDesc: {
		id: 'ai.console.mock-desc',
		defaultMessage: '使用模拟数据运行，便于离线调试。',
	},
	logLines: {
		id: 'ai.console.log-lines',
		defaultMessage: '日志行数',
	},
	logLinesDesc: {
		id: 'ai.console.log-lines-desc',
		defaultMessage: 'AI 分析时可读取的日志缓冲行数。',
	},
	advancedTitle: {
		id: 'ai.console.advanced-title',
		defaultMessage: '高级',
	},
	troubleshoot: {
		id: 'ai.console.troubleshoot',
		defaultMessage: '自动排障',
	},
	troubleshootDesc: {
		id: 'ai.console.troubleshoot-desc',
		defaultMessage: '流 E 提供',
	},
})

const error = ref<string | null>(null)
const logLines = ref<number>(0)

watch(
	() => store.aiConfig?.logLines,
	(value) => {
		if (value !== undefined) logLines.value = value
	},
	{ immediate: true },
)

const save = async (patch: Partial<AiWorkshopConfig>) => {
	error.value = null
	try {
		await store.updateConfig(patch)
	} catch (err) {
		error.value = err instanceof Error ? err.message : String(err)
	}
}

const saveLogLines = () => {
	clampLogLines()
	void save({ logLines: logLines.value })
}

const clampLogLines = () => {
	if (typeof logLines.value !== 'number' || Number.isNaN(logLines.value) || logLines.value < 100) {
		logLines.value = 100
	}
}
</script>
