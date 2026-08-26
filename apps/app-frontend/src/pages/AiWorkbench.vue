<template>
	<WorkbenchLayout>
		<template #sidebar>
			<ConsoleView v-if="store.activeActivity === 'console'" />
			<div
				v-else
				class="flex flex-col items-center justify-center h-full gap-2 p-4 text-center"
			>
				<SparklesIcon class="text-3xl text-primary" />
				<p class="text-sm text-secondary">{{ formatMessage(messages.sidebarPlaceholder) }}</p>
			</div>
		</template>

		<template #main>
			<div class="flex flex-col items-center justify-center h-full gap-2 p-8 text-center">
				<SparklesIcon class="text-4xl text-primary" />
				<h1 class="text-lg font-semibold text-contrast">
					{{ formatMessage(messages.title) }}
				</h1>
				<p class="text-sm text-secondary max-w-md">
					{{ formatMessage(messages.description) }}
				</p>
			</div>
		</template>

		<template #bottom>
			<div class="flex items-center justify-center h-full text-sm text-secondary">
				{{ formatMessage(messages.bottomPlaceholder) }}
			</div>
		</template>
	</WorkbenchLayout>
</template>

<script setup lang="ts">
import { SparklesIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { onMounted } from 'vue'

import ConsoleView from '@/components/ai/sidebar/ConsoleView.vue'
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
	sidebarPlaceholder: {
		id: 'ai.workbench.sidebar-placeholder',
		defaultMessage: '侧边面板内容将在此显示',
	},
	bottomPlaceholder: {
		id: 'ai.workbench.bottom-placeholder',
		defaultMessage: '底部面板：日志、工具输出与排障报告',
	},
})

onMounted(() => {
	void store.init()
})
</script>