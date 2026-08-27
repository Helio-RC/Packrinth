<template>
	<main class="flex h-full w-full bg-bg min-w-0 min-h-0 overflow-hidden">
		<slot />
	</main>
</template>

<script setup lang="ts">
defineOptions({
	name: 'AiMainArea',
})
</script>
