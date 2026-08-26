<template>
	<div class="rounded-lg border border-divider bg-bg-raised p-3 flex flex-col gap-2">
		<div class="flex items-center justify-between gap-2">
			<div class="flex items-center gap-2 min-w-0">
				<WrenchIcon v-if="!approved" class="shrink-0 text-primary" />
				<CheckCheckIcon v-else class="shrink-0 text-green-500" />
				<span class="text-sm font-medium text-contrast truncate">
					{{ formatMessage(messages.title) }}
				</span>
			</div>
			<span
				v-if="approved"
				class="shrink-0 rounded-full bg-green-500/15 px-2 py-0.5 text-xs text-green-500"
			>
				{{ formatMessage(messages.approved) }}
			</span>
		</div>

		<details class="group">
			<summary class="cursor-pointer select-none text-xs text-secondary">
				{{ formatMessage(messages.arguments) }}
			</summary>
			<pre
				class="mt-2 max-h-48 overflow-auto rounded-md bg-bg p-2 text-xs text-primary whitespace-pre-wrap break-words"
				>{{ prettyArguments }}</pre
			>
		</details>

		<div v-if="!approved" class="flex gap-2 justify-end">
			<Button type="outlined" size="sm" @click="reject">
				<XIcon />
				{{ formatMessage(messages.reject) }}
			</Button>
			<Button type="colored" color="green" size="sm" @click="approve">
				<CheckIcon />
				{{ formatMessage(messages.approve) }}
			</Button>
		</div>
	</div>
</template>

<script setup lang="ts">
import { CheckCheckIcon, CheckIcon, WrenchIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { Message } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiToolCard',
})

const props = defineProps<{
	message: Message
}>()

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.chat.tool.title',
		defaultMessage: '工具调用',
	},
	arguments: {
		id: 'ai.chat.tool.arguments',
		defaultMessage: '参数',
	},
	approve: {
		id: 'ai.chat.tool.approve',
		defaultMessage: '允许',
	},
	reject: {
		id: 'ai.chat.tool.reject',
		defaultMessage: '拒绝',
	},
	approved: {
		id: 'ai.chat.tool.approved',
		defaultMessage: '已批准',
	},
})

const approved = computed(() => props.message.content.toLowerCase().includes('approved'))

const prettyArguments = computed(() => {
	if (!props.message.toolCalls) return ''
	try {
		return JSON.stringify(JSON.parse(props.message.toolCalls), null, 2)
	} catch {
		return props.message.toolCalls
	}
})

const approve = () => {
	if (props.message.toolCallId) void store.confirmTool(props.message.toolCallId, true)
}

const reject = () => {
	if (props.message.toolCallId) void store.confirmTool(props.message.toolCallId, false)
}
</script>
