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
				v-for="(command, index) in filteredCommands"
				:key="command.name"
				class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-contrast"
				:class="index === highlightedIndex ? 'bg-bg' : 'hover:bg-bg'"
				type="button"
				@click="runCommand(command)"
				@mouseenter="highlightedIndex = index"
			>
				<span class="font-semibold text-brand">{{ command.name }}</span>
				<span
					v-if="confirmingName === command.name"
					class="text-red-500"
				>
					{{ formatMessage(messages.confirmClear) }}
				</span>
				<span v-else class="text-secondary">{{ command.description }}</span>
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
import { computed, nextTick, ref, watch } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiChatInput',
})

interface ChatCommand {
	name: string
	description: string
	run?: () => void | Promise<void>
	displayOnly?: boolean
	needsConfirm?: boolean
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
	confirmClear: {
		id: 'ai.chat.command.confirm-clear',
		defaultMessage: '确认清空全部会话？',
	},
	help: {
		id: 'ai.chat.command.help',
		defaultMessage: '显示命令说明',
	},
})

const input = ref('')
const textarea = ref<HTMLTextAreaElement | null>(null)
const highlightedIndex = ref(0)
const confirmingName = ref<string | null>(null)

const commands: ChatCommand[] = [
	{
		name: '/new',
		description: formatMessage(messages.newConversation),
		run: () => store.newConversation(),
	},
	{
		name: '/clear',
		description: formatMessage(messages.clear),
		needsConfirm: true,
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

watch(commandOpen, (open) => {
	if (!open) {
		highlightedIndex.value = 0
		confirmingName.value = null
	}
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
	if (highlightedIndex.value >= filteredCommands.value.length) {
		highlightedIndex.value = 0
	}
	if (confirmingName.value) confirmingName.value = null
}

const onKeydown = (event: KeyboardEvent) => {
	if (commandOpen.value && filteredCommands.value.length > 0) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault()
			const command = filteredCommands.value[highlightedIndex.value]
			if (command) void runCommand(command)
			return
		}
		if (event.key === 'ArrowDown') {
			event.preventDefault()
			highlightedIndex.value = Math.min(highlightedIndex.value + 1, filteredCommands.value.length - 1)
			return
		}
		if (event.key === 'ArrowUp') {
			event.preventDefault()
			highlightedIndex.value = Math.max(highlightedIndex.value - 1, 0)
			return
		}
	}
	if (event.key === 'Enter' && !event.shiftKey) {
		event.preventDefault()
		void send()
	}
}

const runCommand = async (command: ChatCommand) => {
	if (command.needsConfirm && confirmingName.value !== command.name) {
		confirmingName.value = command.name
		return
	}
	confirmingName.value = null
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
