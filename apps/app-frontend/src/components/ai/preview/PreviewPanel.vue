<template>
	<div class="flex flex-col h-full min-h-0 bg-bg">
		<div
			class="flex items-center justify-between gap-2 border-b border-divider bg-bg-raised px-2 py-1.5 [&:has(.ai-editor)]:border-t-0"
		>
			<div class="flex items-center gap-1">
				<Button type="quiet" size="sm" :active="mode === 'editor'" @click="mode = 'editor'">
					{{ formatMessage(messages.editor) }}
				</Button>
				<Button type="quiet" size="sm" :active="mode === 'diff'" @click="mode = 'diff'">
					{{ formatMessage(messages.diff) }}
				</Button>
			</div>
			<span class="text-xs text-secondary truncate max-w-[40%]">
				{{ store.previewFileName || formatMessage(messages.noFile) }}
			</span>
		</div>

		<div class="flex-1 min-h-0">
			<ConfigEditor
				v-if="mode === 'editor'"
				:model-value="store.previewText"
				:language="store.previewLanguage"
				@update:model-value="store.previewText = $event"
			/>
			<div v-else class="h-full overflow-y-auto p-3">
				<DiffView
					v-if="store.previewBefore || store.previewAfter"
					:before="store.previewBefore"
					:after="store.previewAfter"
				/>
				<p v-else class="pt-10 text-center text-sm text-secondary">
					{{ formatMessage(messages.noDiff) }}
				</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

import ConfigEditor from './ConfigEditor.vue'
import DiffView from './DiffView.vue'

const store = useAiWorkshopStore()
const { formatMessage } = useVIntl()
const mode = ref<'editor' | 'diff'>('editor')

const messages = defineMessages({
	editor: {
		id: 'ai.preview.editor',
		defaultMessage: 'Editor',
	},
	diff: {
		id: 'ai.preview.diff',
		defaultMessage: 'Diff',
	},
	noFile: {
		id: 'ai.preview.no-file',
		defaultMessage: 'No file selected',
	},
	noDiff: {
		id: 'ai.preview.no-diff',
		defaultMessage: 'No diff available yet',
	},
})
</script>
