<template>
	<div class="relative flex flex-col">
		<div
			v-if="commandOpen"
			class="absolute bottom-full left-0 right-0 z-10 mb-2 overflow-hidden rounded-lg border border-divider bg-bg-raised shadow-lg"
		>
			<p class="px-3 py-1.5 text-xs font-medium text-secondary">
				{{ formatMessage(messages.commandsHint) }}
			</p>
			<button
				v-for="command in filteredCommands"
				:key="command.name"
				class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-contrast hover:bg-bg"
				type="button"
				@click="runCommand(command)"
			>
				<span class="font-semibold text-brand">{{ command.name }}</span>
				<span class="text-secondary">{{ command.description }}</span>
			</button>
			<p v-if="filteredCommands.length === 0" class="px-3 py-2 text-sm text-secondary">
				{{ formatMessage(messages.noCommands) }}
			</p>
		</div>

		<div
			class="flex items-end gap-2 rounded-xl border border-divider bg-bg-raised p-2 focus-within:border-brand"
		>
			<textarea
				ref="textarea"
				v-model="input"
				rows="1"
				:placeholder="formatMessage(messages.placeholder)"
				class="max-h-40 flex-1 resize-none bg-transparent px-2 py-1.5 text-sm text-contrast placeholder:text-secondary outline-none"
				@keydown="onKeydown"
				@input="onInput"
			/>
			<Button
				type="colored"
				color="brand"
				size="sm"
				:disabled="!canSend"
				:loading="store.streaming"
				@click="send"
			>
				<SendIcon v-if="!store.streaming" />
			</Button>
		</div>
	</div>
</template>

<script setup lang="ts">
import { SendIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiChatInput',
})

interface ChatCommand {
	name: string
	description: string
	run?: () => void | Promise<void>
	displayOnly?: boolean
}

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	placeholder: {
		id: 'ai.chat.input.placeholder',
		defaultMessage: '输入消息，/ 查看命令…',
	},
	send: {
		id: 'ai.chat.input.send',
		defaultMessage: '发送',
	},
	commandsHint: {
		id: 'ai.chat.input.commands-hint',
		defaultMessage: '命令',
	},
	noCommands: {
		id: 'ai.chat.input.no-commands',
		defaultMessage: '没有匹配的命令',
	},
	newConversation: {
		id: 'ai.chat.command.new',
		defaultMessage: '新建会话',
	},
	clear: {
		id: 'ai.chat.command.clear',
		defaultMessage: '清空全部会话',
	},
	help: {
		id: 'ai.chat.command.help',
		defaultMessage: '显示命令说明',
	},
})

const input = ref('')
const textarea = ref<HTMLTextAreaElement | null>(null)

const commands: ChatCommand[] = [
	{
		name: '/new',
		description: formatMessage(messages.newConversation),
		run: () => store.newConversation(),
	},
	{
		name: '/clear',
		description: formatMessage(messages.clear),
		run: () => store.clearAll(),
	},
	{
		name: '/help',
		description: formatMessage(messages.help),
		displayOnly: true,
	},
]

const commandOpen = computed(() => input.value.startsWith('/') && input.value.length > 1)

const filteredCommands = computed(() => {
	const query = input.value.slice(1).toLowerCase()
	if (!query) return commands
	return commands.filter((c) => c.name.slice(1).toLowerCase().includes(query))
})

const canSend = computed(() => input.value.trim().length > 0 && !store.streaming)

const autoGrow = () => {
	const el = textarea.value
	if (!el) return
	el.style.height = 'auto'
	el.style.height = `${Math.min(el.scrollHeight, 160)}px`
}

const onInput = () => {
	autoGrow()
}

const onKeydown = (event: KeyboardEvent) => {
	if (event.key === 'Enter' && !event.shiftKey) {
		event.preventDefault()
		void send()
	}
}

const runCommand = async (command: ChatCommand) => {
	input.value = ''
	autoGrow()
	if (command.run) await command.run()
}

const send = async () => {
	const content = input.value.trim()
	if (!content || store.streaming) return

	if (!store.currentConversationId) {
		await store.newConversation()
	}
	input.value = ''
	autoGrow()
	await store.sendMessage(content)
	await nextTick()
	textarea.value?.focus()
}
</script>
