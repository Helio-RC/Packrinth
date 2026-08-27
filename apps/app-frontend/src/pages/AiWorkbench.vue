<template>
	<WorkbenchLayout>
		<template #sidebar>
			<ChatHistory v-if="store.activeActivity === 'chat'" />
			<SkillsView v-else-if="store.activeActivity === 'skills'" />
			<KnowledgeView v-else-if="store.activeActivity === 'knowledge'" />
			<ToolsView v-else-if="store.activeActivity === 'tools'" />
			<ConsoleView v-else-if="store.activeActivity === 'console'" />
			<div
				v-else
				class="flex flex-col items-center justify-center h-full gap-2 p-4 text-center bg-bg-raised"
			>
				<SparklesIcon class="text-3xl text-primary" />
				<p class="text-sm text-secondary">{{ placeholderMessage() }}</p>
			</div>
		</template>

		<template #main>
			<div class="flex flex-col h-full min-h-0">
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
	</WorkbenchLayout>
</template>

<script setup lang="ts">
import { SparklesIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { nextTick, onMounted, ref, watch } from 'vue'

import ChatInput from '@/components/ai/chat/ChatInput.vue'
import ChatMessage from '@/components/ai/chat/ChatMessage.vue'
import WorkbenchLayout from '@/components/ai/layout/WorkbenchLayout.vue'
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
		defaultMessage: 'AI 工作台',
	},
	description: {
		id: 'ai.workbench.description',
		defaultMessage: '使用自然语言与 AI 协作，完成模组安装、配置修改与内容定制。',
	},
	filesPlaceholder: {
		id: 'ai.workbench.files-placeholder',
		defaultMessage: '实例树将在流 C 提供',
	},
	settingsPlaceholder: {
		id: 'ai.workbench.settings-placeholder',
		defaultMessage: '设置将在流 E 提供',
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
