<template>
	<header
		class="flex items-center gap-4 px-4 h-8 shrink-0 bg-bg-raised border-b border-divider text-xs text-secondary"
	>
		<span class="flex items-center gap-2">
			<span
				class="w-2 h-2 rounded-full"
				:class="store.providerConfigured ? 'bg-green-500' : 'bg-yellow-500'"
			/>
			{{
				store.providerConfigured
					? formatMessage(messages.aiReady)
					: formatMessage(messages.aiUnconfigured)
			}}
		</span>
		<span class="flex-1" />
		<span v-if="store.totalTokens > 0" class="tabular-nums">
			{{ formatMessage(messages.tokens, { count: store.totalTokens }) }}
		</span>
		<span v-if="store.aiStatus" class="flex items-center gap-1">
			<SparklesIcon class="text-sm" />
			{{ store.aiStatus.skillCount }} {{ formatMessage(messages.skills) }}
		</span>
	</header>
</template>

<script setup lang="ts">
import { SparklesIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiStatusBar',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	aiReady: {
		id: 'ai.status.ready',
		defaultMessage: 'AI connected',
	},
	aiUnconfigured: {
		id: 'ai.status.unconfigured',
		defaultMessage: 'AI not configured',
	},
	tokens: {
		id: 'ai.status.tokens',
		defaultMessage: '{count} tokens',
	},
	skills: {
		id: 'ai.status.skills',
		defaultMessage: 'skills',
	},
})
</script>
