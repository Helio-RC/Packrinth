<template>
	<div class="flex w-full" :class="isUser ? 'justify-end' : 'justify-start'">
		<div class="flex max-w-[80%] flex-col gap-1" :class="isUser ? 'items-end' : 'items-start'">
			<div
				v-if="message.role === 'assistant'"
				class="rounded-xl rounded-tl-sm bg-bg-raised border border-divider px-4 py-2.5 text-sm text-primary prose-markdown"
				v-html="renderedContent"
			/>
			<div v-else-if="message.role === 'tool'" class="w-full">
				<ToolCard :message="message" />
			</div>
			<div
				v-else
				class="rounded-xl rounded-tr-sm bg-brand px-4 py-2.5 text-sm text-white whitespace-pre-wrap break-words"
			>
				{{ message.content }}
			</div>

			<span
				v-if="streaming && message.role === 'assistant'"
				class="inline-block h-4 w-2 self-start rounded-sm bg-text-secondary cursor-blink"
				aria-hidden="true"
			/>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import { renderMarkdown } from '@/lib/ai/markdown'
import type { Message } from '@/lib/ai/types'

import ToolCard from './ToolCard.vue'

defineOptions({
	name: 'AiChatMessage',
})

const props = defineProps<{
	message: Message
	streaming?: boolean
}>()

const isUser = computed(() => props.message.role === 'user')

const renderedContent = computed(() => renderMarkdown(props.message.content))
</script>

<style scoped>
.cursor-blink {
	animation: blink 1s step-end infinite;
}

@keyframes blink {
	0%,
	100% {
		opacity: 1;
	}
	50% {
		opacity: 0;
	}
}

.prose-markdown :deep(p) {
	margin: 0.5em 0;
}

.prose-markdown :deep(p:first-child) {
	margin-top: 0;
}

.prose-markdown :deep(p:last-child) {
	margin-bottom: 0;
}

.prose-markdown :deep(h1),
.prose-markdown :deep(h2),
.prose-markdown :deep(h3),
.prose-markdown :deep(h4) {
	margin: 0.6em 0 0.3em;
	font-weight: 600;
	color: var(--color-contrast);
}

.prose-markdown :deep(ul),
.prose-markdown :deep(ol) {
	margin: 0.4em 0;
	padding-left: 1.25rem;
}

.prose-markdown :deep(ul) {
	list-style: disc;
}

.prose-markdown :deep(ol) {
	list-style: decimal;
}

.prose-markdown :deep(li) {
	margin: 0.2em 0;
}

.prose-markdown :deep(a) {
	color: var(--color-brand);
	text-decoration: underline;
}

.prose-markdown :deep(blockquote) {
	margin: 0.5em 0;
	padding-left: 0.75rem;
	border-left: 3px solid var(--color-divider);
	color: var(--color-secondary);
}

.prose-markdown :deep(pre) {
	margin: 0.5em 0;
	padding: 0.75rem;
	border-radius: 0.5rem;
	background: var(--color-bg);
	overflow: auto;
}

.prose-markdown :deep(code) {
	padding: 0.1em 0.35em;
	border-radius: 0.25rem;
	background: var(--color-bg);
	font-size: 0.9em;
}

.prose-markdown :deep(pre code) {
	padding: 0;
	background: transparent;
}

.prose-markdown :deep(hr) {
	margin: 0.75em 0;
	border: none;
	border-top: 1px solid var(--color-divider);
}
</style>
