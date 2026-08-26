<template>
	<section class="flex flex-col h-full w-full bg-bg-raised overflow-hidden">
		<header class="flex items-center gap-2 px-3 h-8 shrink-0 border-b border-divider">
			<h2 class="text-sm font-semibold text-contrast flex-1 truncate">
				{{ formatMessage(messages.title) }}
			</h2>
			<button
				class="text-primary hover:text-contrast cursor-pointer border-none bg-transparent p-1"
				:title="formatMessage(messages.hide)"
				@click="toggle"
			>
				<ChevronDownIcon />
			</button>
		</header>
		<div class="flex-1 min-h-0 overflow-hidden">
			<slot />
		</div>
	</section>
</template>

<script setup lang="ts">
import { ChevronDownIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiBottomPanel',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.bottom-panel.title',
		defaultMessage: '底部面板',
	},
	hide: {
		id: 'ai.bottom-panel.hide',
		defaultMessage: '收起底部面板',
	},
})

const toggle = () => {
	store.layout.bottomVisible = !store.layout.bottomVisible
	store.saveLayout()
}
</script>