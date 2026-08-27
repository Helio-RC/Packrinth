<template>
	<button
		class="group relative w-12 h-11 flex items-center justify-center text-xl transition-all cursor-pointer border-none bg-transparent"
		:class="active ? 'text-contrast' : 'text-primary hover:text-contrast'"
		:title="title"
		@click="$emit('click')"
	>
		<component :is="icon" />
		<span
			class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-6 rounded-r transition-all"
			:class="active ? 'bg-brand' : 'bg-transparent'"
		/>
	</button>
</template>

<script setup lang="ts">
import type { Component } from 'vue'

defineProps<{
	icon: Component
	title: string
	active: boolean
}>()

defineEmits<{
	click: []
}>()
</script>
