<template>
	<main class="flex h-full w-full bg-bg min-w-0 min-h-0 overflow-hidden">
		<Splitpanes
			:horizontal="false"
			:dbl-click-splitter="false"
			class="ai-main-split flex h-full w-full"
		>
			<Pane v-for="view in store.layout.mainViews" :key="view" :min-size="15">
				<div class="flex flex-col h-full min-h-0">
					<div
						class="flex items-center justify-between gap-2 px-2 py-1.5 border-b border-divider bg-bg-raised"
					>
						<span class="text-xs font-semibold text-contrast">{{ label(view) }}</span>
						<div class="flex items-center gap-1">
							<button
								v-if="store.layout.mainViews.length > 1"
								class="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-secondary hover:text-contrast hover:bg-divider cursor-pointer border-none bg-transparent"
								:title="formatMessage(messages.swap)"
								@click="swap(view)"
							>
								<ArrowLeftRightIcon class="size-3.5" />
							</button>
							<button
								v-if="store.layout.mainViews.length > 1"
								class="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-secondary hover:text-contrast hover:bg-divider cursor-pointer border-none bg-transparent"
								:title="formatMessage(messages.close)"
								@click="close(view)"
							>
								<XIcon class="size-3.5" />
							</button>
						</div>
					</div>

					<div v-if="view === 'chat'" class="flex-1 min-h-0">
						<slot name="chat" />
					</div>
					<PreviewPanel v-else-if="view === 'preview'" class="flex-1 min-h-0" />
					<div v-else class="flex-1 min-h-0" />
				</div>
			</Pane>
		</Splitpanes>
	</main>
</template>

<script setup lang="ts">
import 'splitpanes/dist/splitpanes.css'

import { ArrowLeftRightIcon, XIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { Pane, Splitpanes } from 'splitpanes'

import PreviewPanel from '@/components/ai/preview/PreviewPanel.vue'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

const store = useAiWorkshopStore()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	chat: {
		id: 'ai.main.chat',
		defaultMessage: 'Chat',
	},
	preview: {
		id: 'ai.main.preview',
		defaultMessage: 'Preview',
	},
	close: {
		id: 'ai.main.close',
		defaultMessage: 'Close pane',
	},
	swap: {
		id: 'ai.main.swap',
		defaultMessage: 'Move to other pane',
	},
})

const label = (view: string) => {
	if (view === 'chat') return formatMessage(messages.chat)
	if (view === 'preview') return formatMessage(messages.preview)
	return view
}

const close = (view: string) => {
	store.layout.mainViews = store.layout.mainViews.filter((v) => v !== view)
	store.saveLayout()
}

const swap = (view: string) => {
	const views = [...store.layout.mainViews]
	const index = views.indexOf(view)
	if (index === -1) return
	const target = (index + 1) % views.length
	;[views[index], views[target]] = [views[target], views[index]]
	store.layout.mainViews = views
	store.saveLayout()
}
</script>

<style scoped>
.ai-main-split :deep(.splitpanes__splitter) {
	background-color: var(--color-divider);
	border: none;
}

.ai-main-split :deep(.splitpanes__splitter::after) {
	content: '';
	position: absolute;
	inset: 0;
	background: transparent;
	transition: background-color 0.15s;
}

.ai-main-split :deep(.splitpanes__splitter:hover::after) {
	background-color: var(--brand-gradient-bg);
	opacity: 0.6;
}
</style>
