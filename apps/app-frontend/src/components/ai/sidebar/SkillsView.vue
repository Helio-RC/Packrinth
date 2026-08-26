<template>
	<div class="flex flex-col h-full w-full min-h-0 bg-bg-raised">
		<div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-divider">
			<h2 class="text-sm font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<Button type="quiet" size="sm" :loading="refreshing" @click="refresh">
				<RefreshCwIcon />
				{{ formatMessage(messages.refresh) }}
			</Button>
		</div>

		<div class="flex-1 min-h-0 overflow-y-auto">
			<div v-if="store.skills.length === 0" class="flex flex-col gap-2 px-3 py-6 text-center">
				<p class="text-sm text-secondary">{{ formatMessage(messages.empty) }}</p>
			</div>

			<div v-else class="flex flex-col gap-2 p-3">
				<div
					v-for="skill in store.skills"
					:key="skill.name"
					class="rounded-lg border border-divider bg-bg p-3"
				>
					<div class="flex items-start justify-between gap-2">
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm font-medium text-contrast">{{ skill.name }}</p>
							<p v-if="skill.author || skill.version" class="mt-0.5 text-xs text-secondary">
								<template v-if="skill.author">{{ skill.author }}</template>
								<template v-if="skill.author && skill.version"> · </template>
								<template v-if="skill.version">v{{ skill.version }}</template>
							</p>
						</div>
						<label class="flex shrink-0 cursor-pointer items-center">
							<input
								type="checkbox"
								class="peer sr-only"
								:checked="skill.enabled"
								@change="(e) => toggle(skill.name, (e.target as HTMLInputElement).checked)"
							/>
							<span
								class="relative inline-flex h-5 w-9 items-center rounded-full transition-colors peer-checked:bg-brand bg-divider"
							>
								<span
									class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform peer-checked:translate-x-4 translate-x-0.5"
								/>
							</span>
						</label>
					</div>

					<p v-if="skill.description" class="mt-2 text-sm text-primary">
						{{ skill.description }}
					</p>

					<div v-if="skill.keywords.length > 0" class="mt-2 flex flex-wrap gap-1">
						<span
							v-for="keyword in skill.keywords"
							:key="keyword"
							class="rounded-full bg-bg-raised px-2 py-0.5 text-xs text-secondary"
						>
							{{ keyword }}
						</span>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { RefreshCwIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { onMounted, ref } from 'vue'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiSkillsView',
})

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.skills.title',
		defaultMessage: '技能管理',
	},
	refresh: {
		id: 'ai.skills.refresh',
		defaultMessage: '刷新',
	},
	empty: {
		id: 'ai.skills.empty',
		defaultMessage: '暂无技能',
	},
})

const refreshing = ref(false)

const toggle = (name: string, enabled: boolean) => {
	void store.toggleSkill(name, enabled)
}

const refresh = async () => {
	refreshing.value = true
	try {
		await store.refreshSkills()
	} finally {
		refreshing.value = false
	}
}

onMounted(() => {
	if (store.skills.length === 0) void store.loadSkills()
})
</script>
