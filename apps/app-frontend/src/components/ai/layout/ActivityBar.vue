<template>
	<nav
		class="flex flex-col items-center gap-1 py-2 bg-bg-raised border-r border-divider w-12 shrink-0"
	>
		<ActivityBarItem
			v-for="item in items"
			:key="item.id"
			:icon="item.icon"
			:title="item.title"
			:active="store.activeActivity === item.id"
			@click="store.activeActivity = item.id"
		/>
		<div class="flex-1" />
		<button
			class="w-12 h-11 flex items-center justify-center text-xl text-primary hover:text-contrast transition-all cursor-pointer border-none bg-transparent"
			:title="formatMessage(messages.activitybarPosition)"
			@click="togglePosition"
		>
			<ArrowLeftRightIcon />
		</button>
	</nav>
</template>

<script setup lang="ts">
import {
	ArrowLeftRightIcon,
	BookTextIcon,
	FolderOpenIcon,
	GaugeIcon,
	MessagesSquareIcon,
	SettingsIcon,
	SparklesIcon,
	WrenchIcon,
} from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

import ActivityBarItem from './ActivityBarItem.vue'

const store = useAiWorkshopStore()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	activitybarPosition: {
		id: 'ai.activitybar.toggle-position',
		defaultMessage: 'Toggle activity bar position',
	},
})

const togglePosition = () => {
	store.layout.activitybarPosition = store.layout.activitybarPosition === 'left' ? 'right' : 'left'
	store.saveLayout()
}

const items = computed(() => [
	{ id: 'chat', icon: MessagesSquareIcon, title: '对话' },
	{ id: 'files', icon: FolderOpenIcon, title: '文件' },
	{ id: 'knowledge', icon: BookTextIcon, title: '知识' },
	{ id: 'skills', icon: SparklesIcon, title: '技能' },
	{ id: 'tools', icon: WrenchIcon, title: '工具' },
	{ id: 'console', icon: GaugeIcon, title: '控制台' },
	{ id: 'settings', icon: SettingsIcon, title: '设置' },
])
</script>
