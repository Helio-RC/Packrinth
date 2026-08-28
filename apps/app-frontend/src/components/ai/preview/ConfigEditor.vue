<template>
	<div class="h-full min-h-0 w-full">
		<div ref="container" class="h-full w-full" />
	</div>
</template>

<script setup lang="ts">
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

/* 通过 worker 文件让 Monaco 在 Tauri WebView 中离线工作（无 CDN 依赖）。 */
;(self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
	getWorker(_, label: string) {
		if (label === 'json') return new jsonWorker()
		return new editorWorker()
	},
}

const props = withDefaults(
	defineProps<{
		modelValue: string
		language?: string
		readOnly?: boolean
	}>(),
	{
		language: 'json',
		readOnly: false,
	},
)

const emit = defineEmits<{
	'update:modelValue': [value: string]
}>()

const container = ref<HTMLElement | null>(null)
let editor: monaco.editor.IStandaloneCodeEditor | null = null
let updating = false

onMounted(() => {
	if (!container.value) return
	editor = monaco.editor.create(container.value, {
		value: props.modelValue,
		language: props.language,
		readOnly: props.readOnly,
		automaticLayout: true,
		theme: 'vs-dark',
		fontSize: 13,
		minimap: { enabled: false },
	})

	editor.onDidChangeModelContent(() => {
		if (updating) return
		emit('update:modelValue', editor?.getValue() ?? '')
	})
})

watch(
	() => props.modelValue,
	(value) => {
		if (!editor || editor.getValue() === value) return
		updating = true
		editor.setValue(value)
		updating = false
	},
)

watch(
	() => props.language,
	(language) => {
		if (editor && language) {
			monaco.editor.setModelLanguage(editor.getModel()!, language)
		}
	},
)

onBeforeUnmount(() => {
	editor?.dispose()
	editor = null
})
</script>
