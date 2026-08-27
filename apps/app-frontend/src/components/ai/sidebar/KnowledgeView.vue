<template>
	<div class="flex flex-col h-full w-full min-h-0 bg-bg-raised">
		<div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-divider">
			<h2 class="text-sm font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
		</div>

		<div class="flex gap-2 border-b border-divider px-3 py-2">
			<input
				v-model="query"
				type="text"
				:placeholder="formatMessage(messages.placeholder)"
				class="flex-1 min-w-0 rounded-lg border border-divider bg-bg px-3 py-1.5 text-sm text-contrast outline-none focus:border-brand"
				@keydown.enter="search"
			/>
			<Button type="outlined" size="sm" :disabled="!query.trim() || searching" @click="search">
				<SearchIcon />
				{{ formatMessage(messages.search) }}
			</Button>
		</div>

		<div class="flex-1 min-h-0 overflow-y-auto">
			<p v-if="searching" class="px-3 py-6 text-center text-sm text-secondary">
				{{ formatMessage(messages.loading) }}
			</p>

			<div
				v-else-if="searched && store.knowledgeResults.length === 0"
				class="px-3 py-6 text-center"
			>
				<p class="text-sm text-secondary">{{ formatMessage(messages.noResults) }}</p>
			</div>

			<div v-else-if="!searched" class="px-3 py-6 text-center">
				<p class="text-sm text-secondary">{{ formatMessage(messages.prompt) }}</p>
			</div>

			<div v-else class="flex flex-col gap-2 p-3">
				<div
					v-for="(hit, index) in store.knowledgeResults"
					:key="`${index}-${hit.title}`"
					class="rounded-lg border border-divider bg-bg p-3"
				>
					<div class="flex items-start justify-between gap-2">
						<p class="min-w-0 flex-1 truncate text-sm font-medium text-contrast">
							{{ hit.title }}
						</p>
						<span
							v-if="hit.score !== undefined && hit.score !== null"
							class="shrink-0 rounded-full bg-bg-raised px-2 py-0.5 text-xs text-secondary tabular-nums"
						>
							{{ scoreLabel(hit.score) }}
						</span>
					</div>
					<p v-if="hit.snippet" class="mt-1 text-sm text-primary">{{ hit.snippet }}</p>
					<p v-if="hit.source" class="mt-2 text-xs text-secondary">{{ hit.source }}</p>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiKnowledgeView',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.knowledge.title',
		defaultMessage: '知识检索',
	},
	placeholder: {
		id: 'ai.knowledge.placeholder',
		defaultMessage: '搜索知识库…',
	},
	search: {
		id: 'ai.knowledge.search',
		defaultMessage: '搜索',
	},
	loading: {
		id: 'ai.knowledge.loading',
		defaultMessage: '搜索中…',
	},
	noResults: {
		id: 'ai.knowledge.no-results',
		defaultMessage: '未找到相关结果',
	},
	prompt: {
		id: 'ai.knowledge.prompt',
		defaultMessage: '输入关键词开始检索',
	},
})

const query = ref('')
const searching = ref(false)
const searched = ref(false)

const search = async () => {
	const q = query.value.trim()
	if (!q) return
	searching.value = true
	searched.value = true
	try {
		await store.searchKnowledge(q)
	} finally {
		searching.value = false
	}
}

const scoreLabel = (score: number) => {
	if (typeof score !== 'number') return ''
	return score.toFixed(2)
}
</script>
