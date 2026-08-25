<template>
	<div class="flex h-full w-full items-center justify-center">
		<p class="text-lg text-primary">{{ formatMessage(messages.comingSoon) }}</p>
	</div>
</template>

<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

defineOptions({
	name: 'AiWorkbenchPage',
})

const { formatMessage } = useVIntl()

const messages = defineMessages({
	comingSoon: {
		id: 'ai.workbench.coming-soon',
		defaultMessage: 'AI 工作台开发中',
	},
})
</script>
