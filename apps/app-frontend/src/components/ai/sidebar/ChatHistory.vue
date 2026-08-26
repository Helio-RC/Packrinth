<template>
	<div class="flex flex-col h-full w-full min-h-0 bg-bg-raised">
		<div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-divider">
			<h2 class="text-sm font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<Button type="quiet" size="sm" @click="newConversation">
				<PlusIcon />
				{{ formatMessage(messages.new) }}
			</Button>
		</div>

		<div class="flex-1 min-h-0 overflow-y-auto">
			<div
				v-for="conversation in store.conversations"
				:key="conversation.id"
				class="group flex w-full cursor-pointer items-center gap-2 px-3 py-2"
				:class="isActive(conversation.id) ? 'bg-bg' : 'hover:bg-bg'"
				@click="select(conversation.id)"
			>
				<input
					v-if="editingId === conversation.id"
					v-model="editTitle"
					class="flex-1 min-w-0 rounded border border-brand bg-bg px-1.5 py-0.5 text-sm text-contrast outline-none"
					@click.stop
					@keydown.enter="commitRename(conversation.id)"
					@keydown.esc="editingId = null"
					@blur="commitRename(conversation.id)"
				/>
				<template v-else>
					<span
						class="flex-1 min-w-0 truncate text-sm"
						:class="isActive(conversation.id) ? 'text-contrast font-medium' : 'text-primary'"
					>
						{{ conversation.title || formatMessage(messages.untitled) }}
					</span>
					<span
						v-if="confirmingDeleteId === conversation.id"
						class="flex items-center gap-1 text-xs text-red-500"
					>
						{{ formatMessage(messages.confirm) }}
					</span>
					<PencilIcon
						v-else-if="isActive(conversation.id)"
						class="h-4 w-4 shrink-0 text-secondary opacity-0 group-hover:opacity-100"
						@click.stop="startRename(conversation)"
					/>
					<TrashIcon
						v-if="confirmingDeleteId === conversation.id"
						class="h-4 w-4 shrink-0 text-red-500"
						@click.stop="confirmDelete(conversation.id)"
					/>
					<TrashIcon
						v-else
						class="h-4 w-4 shrink-0 text-secondary opacity-0 group-hover:opacity-100"
						@click.stop="confirmDelete(conversation.id)"
					/>
				</template>
			</div>

			<p
				v-if="store.conversations.length === 0"
				class="px-3 py-6 text-center text-sm text-secondary"
			>
				{{ formatMessage(messages.empty) }}
			</p>
		</div>

		<div class="border-t border-divider p-2">
			<Button
				type="outlined"
				color="red"
				size="sm"
				class="w-full"
				:disabled="store.conversations.length === 0"
				@click="confirmClear"
			>
				<TrashIcon />
				{{
					confirmingClear ? formatMessage(messages.confirmClear) : formatMessage(messages.clearAll)
				}}
			</Button>
		</div>
	</div>
</template>

<script setup lang="ts">
import { PencilIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import type { Conversation } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiChatHistory',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.history.title',
		defaultMessage: '会话历史',
	},
	new: {
		id: 'ai.history.new',
		defaultMessage: '新建',
	},
	untitled: {
		id: 'ai.history.untitled',
		defaultMessage: '未命名会话',
	},
	empty: {
		id: 'ai.history.empty',
		defaultMessage: '暂无会话',
	},
	confirm: {
		id: 'ai.history.confirm-delete',
		defaultMessage: '确认删除？',
	},
	confirmClear: {
		id: 'ai.history.confirm-clear',
		defaultMessage: '确认清空全部？',
	},
	clearAll: {
		id: 'ai.history.clear-all',
		defaultMessage: '清空全部会话',
	},
	renamePlaceholder: {
		id: 'ai.history.rename-placeholder',
		defaultMessage: '输入新名称…',
	},
})

const editingId = ref<string | null>(null)
const editTitle = ref('')
const confirmingDeleteId = ref<string | null>(null)
const confirmingClear = ref(false)

const isActive = (id: string) => store.currentConversationId === id

const select = (id: string) => {
	if (store.streaming) return
	void store.loadConversation(id)
}

const newConversation = () => {
	void store.newConversation()
}

const startRename = (conversation: Conversation) => {
	editingId.value = conversation.id
	editTitle.value = conversation.title
	confirmingDeleteId.value = null
	confirmingClear.value = false
}

const commitRename = (id: string) => {
	const title = editTitle.value.trim()
	if (editingId.value === id && title) {
		void store.renameConversation(id, title)
	}
	editingId.value = null
}

const confirmDelete = (id: string) => {
	if (confirmingDeleteId.value === id) {
		confirmingDeleteId.value = null
		void store.removeConversation(id)
		return
	}
	confirmingDeleteId.value = id
	confirmingClear.value = false
}

const confirmClear = () => {
	if (confirmingClear.value) {
		confirmingClear.value = false
		void store.clearAll()
		return
	}
	confirmingClear.value = true
	confirmingDeleteId.value = null
}
</script>
