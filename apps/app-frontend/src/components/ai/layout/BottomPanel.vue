<template>
	<section class="flex flex-col h-full w-full bg-bg-raised overflow-hidden">
		<header class="flex items-center gap-1 px-2 h-8 shrink-0 border-b border-divider">
			<button
				v-for="tab in tabs"
				:key="tab.id"
				class="h-full px-2 text-sm font-medium cursor-pointer border-none bg-transparent transition-colors"
				:class="activeTab === tab.id ? 'text-contrast' : 'text-secondary hover:text-contrast'"
				@click="activeTab = tab.id"
			>
				{{ tab.label }}
			</button>
			<div class="flex-1" />
			<button
				class="text-primary hover:text-contrast cursor-pointer border-none bg-transparent p-1"
				:title="formatMessage(messages.hide)"
				@click="toggle"
			>
				<ChevronDownIcon />
			</button>
		</header>
		<div class="flex-1 min-h-0 overflow-hidden">
			<LogViewer v-if="activeTab === 'logs'" />
			<ToolOutput v-else-if="activeTab === 'tools'" />
			<TroubleshootReport v-else />
		</div>
	</section>
</template>

<script setup lang="ts">
import { ChevronDownIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import LogViewer from '@/components/ai/bottom/LogViewer.vue'
import ToolOutput from '@/components/ai/bottom/ToolOutput.vue'
import TroubleshootReport from '@/components/ai/bottom/TroubleshootReport.vue'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiBottomPanel',
})

type BottomTabId = 'logs' | 'tools' | 'troubleshoot'

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	hide: {
		id: 'ai.bottom-panel.hide',
		defaultMessage: 'Hide bottom panel',
	},
	tabLogs: {
		id: 'ai.logs.title',
		defaultMessage: 'Logs',
	},
	tabTools: {
		id: 'ai.tool-output.title',
		defaultMessage: 'Tool output',
	},
	tabTroubleshoot: {
		id: 'ai.troubleshoot.title',
		defaultMessage: 'Troubleshoot report',
	},
})

const activeTab = ref<BottomTabId>('logs')

const tabs = computed<{ id: BottomTabId; label: string }[]>(() => [
	{ id: 'logs', label: formatMessage(messages.tabLogs) },
	{ id: 'tools', label: formatMessage(messages.tabTools) },
	{ id: 'troubleshoot', label: formatMessage(messages.tabTroubleshoot) },
])

const toggle = () => {
	store.layout.bottomVisible = !store.layout.bottomVisible
	store.saveLayout()
}
</script>
