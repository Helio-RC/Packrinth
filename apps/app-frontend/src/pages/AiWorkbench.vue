<template>
	<WorkbenchLayout>
		<template #sidebar>
			<ChatHistory v-if="store.activeActivity === 'chat'" />
			<SkillsView v-else-if="store.activeActivity === 'skills'" />
			<KnowledgeView v-else-if="store.activeActivity === 'knowledge'" />
			<ToolsView v-else-if="store.activeActivity === 'tools'" />
			<ConsoleView v-else-if="store.activeActivity === 'console'" />
			<AiProviderSettings v-else-if="store.activeActivity === 'settings'" />
			<div
				v-else
				class="flex flex-col items-center justify-center h-full gap-2 p-4 text-center bg-bg-raised"
			>
				<SparklesIcon class="text-3xl text-primary" />
				<p class="text-sm text-secondary">{{ placeholderMessage() }}</p>
			</div>
		</template>

		<template #main>
			<MainArea>
				<template #chat>
					<div class="flex flex-col h-full min-h-0">
						<div
							v-if="store.providerConfigured === false"
							class="flex items-center justify-between gap-3 border-b border-divider bg-brand/10 px-4 py-2"
						>
							<p class="text-sm text-contrast">
								{{ formatMessage(messages.providerBanner) }}
							</p>
							<Button type="outlined" size="sm" @click="store.activeActivity = 'settings'">
								{{ formatMessage(messages.providerBannerAction) }}
							</Button>
						</div>

						<div
							v-if="!store.currentConversationId || store.messages.length === 0"
							class="flex flex-col items-center justify-center flex-1 gap-2 p-8 text-center"
						>
							<SparklesIcon class="text-4xl text-primary" />
							<h1 class="text-lg font-semibold text-contrast">
								{{ formatMessage(messages.title) }}
							</h1>
							<p class="text-sm text-secondary max-w-md">
								{{ formatMessage(messages.description) }}
							</p>
						</div>

						<div v-else ref="scrollEl" class="flex-1 min-h-0 overflow-y-auto px-4 py-4">
							<div class="flex flex-col gap-3">
								<ChatMessage
									v-for="message in store.messages"
									:key="message.id"
									:message="message"
									:streaming="isStreamingMessage(message.id)"
								/>
							</div>
						</div>

						<div class="border-t border-divider px-4 py-3">
							<ChatInput />
						</div>
					</div>
				</template>
			</MainArea>
		</template>
	</WorkbenchLayout>
</template>

<script setup lang="ts">
import { SparklesIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { nextTick, onMounted, ref, watch } from 'vue'

import ChatInput from '@/components/ai/chat/ChatInput.vue'
import ChatMessage from '@/components/ai/chat/ChatMessage.vue'
import MainArea from '@/components/ai/layout/MainArea.vue'
import WorkbenchLayout from '@/components/ai/layout/WorkbenchLayout.vue'
import AiProviderSettings from '@/components/ai/settings/AiProviderSettings.vue'
import ChatHistory from '@/components/ai/sidebar/ChatHistory.vue'
import ConsoleView from '@/components/ai/sidebar/ConsoleView.vue'
import KnowledgeView from '@/components/ai/sidebar/KnowledgeView.vue'
import SkillsView from '@/components/ai/sidebar/SkillsView.vue'
import ToolsView from '@/components/ai/sidebar/ToolsView.vue'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiWorkbenchPage',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.workbench.title',
		defaultMessage: 'AI Workbench',
	},
	description: {
		id: 'ai.workbench.description',
		defaultMessage:
			'Use natural language to work with AI on mod installation, configuration changes, and content customization.',
	},
	filesPlaceholder: {
		id: 'ai.workbench.files-placeholder',
		defaultMessage: 'The instance tree arrives with stream C',
	},
	settingsPlaceholder: {
		id: 'ai.workbench.settings-placeholder',
		defaultMessage: 'Settings arrive with stream E',
	},
	providerBanner: {
		id: 'ai.workbench.provider-banner',
		defaultMessage: 'No AI provider configured yet — configure a provider to start chatting.',
	},
	providerBannerAction: {
		id: 'ai.workbench.provider-banner-action',
		defaultMessage: 'Open settings',
	},
})

const placeholderMessage = () => {
	if (store.activeActivity === 'files') return formatMessage(messages.filesPlaceholder)
	if (store.activeActivity === 'settings') return formatMessage(messages.settingsPlaceholder)
	return ''
}

const scrollEl = ref<HTMLElement | null>(null)

const isStreamingMessage = (id: string) => {
	if (!store.streaming) return false
	const last = store.messages[store.messages.length - 1]
	return last?.role === 'assistant' && last.id === id
}

watch(
	() => [
		store.messages.length,
		store.streaming,
		store.messages[store.messages.length - 1]?.content.length,
	],
	async () => {
		await nextTick()
		scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight })
	},
)

onMounted(() => {
	void store.init()
})
</script>
