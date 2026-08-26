<template>
	<div ref="container" class="flex flex-col h-full w-full bg-bg overflow-hidden">
		<StatusBar />
		<div class="flex-1 flex min-h-0">
			<Splitpanes
				:horizontal="true"
				:dbl-click-splitter="false"
				class="ai-workbench-split flex-1 min-h-0"
				@resized="onVerticalResized"
			>
				<Pane :size="mainPaneRatio" :min-size="30">
					<div class="flex h-full min-h-0">
						<ActivityBar v-if="activitybarPosition === 'left'" />
						<Splitpanes
							:horizontal="false"
							:dbl-click-splitter="false"
							class="ai-workbench-split flex-1 min-w-0"
							@resized="onHorizontalResized"
						>
							<Pane v-if="store.layout.sidebarVisible" :size="sidebarRatio" :min-size="15">
								<SidePanel>
									<slot name="sidebar" />
								</SidePanel>
							</Pane>
							<Pane :size="mainRatio" :min-size="30">
								<MainArea>
									<slot name="main" />
								</MainArea>
							</Pane>
						</Splitpanes>
						<ActivityBar v-if="activitybarPosition === 'right'" />
					</div>
				</Pane>
				<Pane v-if="store.layout.bottomVisible" :size="bottomRatio" :min-size="10">
					<BottomPanel>
						<slot name="bottom" />
					</BottomPanel>
				</Pane>
			</Splitpanes>
		</div>
	</div>
</template>

<script setup lang="ts">
import { useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Pane, Splitpanes } from 'splitpanes'
import 'splitpanes/dist/splitpanes.css'

import { useAiWorkshopStore } from '@/stores/aiWorkshop'
import ActivityBar from './ActivityBar.vue'
import BottomPanel from './BottomPanel.vue'
import MainArea from './MainArea.vue'
import SidePanel from './SidePanel.vue'
import StatusBar from './StatusBar.vue'

useVIntl()

const store = useAiWorkshopStore()

const container = ref<HTMLElement | null>(null)
const containerWidth = ref(0)
const containerHeight = ref(0)

const activitybarPosition = computed(() => store.layout.activitybarPosition)

const sidebarRatio = computed(() =>
	containerWidth.value > 0
		? Math.min(Math.max((store.layout.sidebarWidth / containerWidth.value) * 100, 15), 40)
		: 30,
)

const mainRatio = computed(() => Math.max(100 - sidebarRatio.value, 30))

const bottomRatio = computed(() =>
	containerHeight.value > 0
		? Math.min(Math.max((store.layout.bottomPanelHeight / containerHeight.value) * 100, 10), 60)
		: 25,
)

const mainPaneRatio = computed(() => Math.max(100 - bottomRatio.value, 40))

const onHorizontalResized = (panes: { size: number }[]) => {
	store.layout.sidebarWidth = Math.round((panes[0].size / 100) * containerWidth.value)
	store.saveLayout()
}

const onVerticalResized = (panes: { size: number }[]) => {
	const bottom = panes[1]
	if (bottom) {
		store.layout.bottomPanelHeight = Math.round((bottom.size / 100) * containerHeight.value)
		store.saveLayout()
	}
}

let observer: ResizeObserver | null = null

onMounted(() => {
	if (!container.value) return
	observer = new ResizeObserver((entries) => {
		const rect = entries[0].contentRect
		containerWidth.value = rect.width
		containerHeight.value = rect.height
	})
	observer.observe(container.value)
})

onBeforeUnmount(() => {
	observer?.disconnect()
})
</script>

<style scoped>
.ai-workbench-split :deep(.splitpanes__splitter) {
	background-color: var(--color-divider);
	border: none;
	position: relative;
}

.ai-workbench-split :deep(.splitpanes__splitter:hover) {
	background-color: var(--brand-gradient-bg);
	transition: background-color 0.15s;
}

.ai-workbench-split :deep(.splitpanes__splitter::after) {
	content: '';
	position: absolute;
	inset: 0;
	background: transparent;
	transition: background-color 0.15s;
}

.ai-workbench-split :deep(.splitpanes__splitter:hover::after) {
	background-color: var(--brand-gradient-bg);
	opacity: 0.6;
}
</style>