<template>
	<aside class="flex flex-col h-full w-full bg-bg-raised overflow-hidden">
		<slot />
	</aside>
</template>

<script setup lang="ts">
defineOptions({
	name: 'AiSidePanel',
})
</script>
