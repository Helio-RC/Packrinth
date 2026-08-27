<template>
	<div class="flex flex-col h-full w-full min-h-0 bg-bg-raised">
		<div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-divider">
			<h2 class="text-sm font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
		</div>

		<div class="flex-1 min-h-0 overflow-y-auto">
			<div v-if="store.tools.length === 0" class="px-3 py-6 text-center">
				<p class="text-sm text-secondary">{{ formatMessage(messages.empty) }}</p>
			</div>

			<div v-else class="flex flex-col gap-2 p-3">
				<div
					v-for="tool in store.tools"
					:key="tool.name"
					class="rounded-lg border border-divider bg-bg p-3"
				>
					<div class="flex items-start justify-between gap-2">
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2">
								<p class="truncate text-sm font-medium text-contrast">{{ tool.name }}</p>
								<span
									v-if="tool.requiresConfirmation"
									class="shrink-0 rounded-full bg-orange-500/15 px-2 py-0.5 text-xs text-orange-500"
								>
									{{ formatMessage(messages.confirmBadge) }}
								</span>
							</div>
							<p v-if="tool.domain" class="mt-0.5 text-xs text-secondary">{{ tool.domain }}</p>
						</div>
						<Button type="outlined" size="sm" @click="toggleForm(tool)">
							{{ isOpen(tool) ? formatMessage(messages.cancel) : formatMessage(messages.execute) }}
						</Button>
					</div>

					<p v-if="tool.description" class="mt-2 text-sm text-primary">{{ tool.description }}</p>

					<form v-if="isOpen(tool)" class="mt-3 flex flex-col gap-3" @submit.prevent="submit(tool)">
						<div v-for="field in schemaFields(tool)" :key="field.key" class="flex flex-col gap-1">
							<label class="flex items-center gap-1 text-sm text-contrast">
								<span>{{ field.label }}</span>
								<span v-if="field.required" class="text-red-500">*</span>
							</label>

							<input
								v-if="field.type === 'boolean'"
								type="checkbox"
								:checked="booleanValues[formKey(tool, field.key)] ?? false"
								class="h-4 w-4 accent-brand"
								@change="
									(e) =>
										(booleanValues[formKey(tool, field.key)] = (
											e.target as HTMLInputElement
										).checked)
								"
							/>

							<textarea
								v-else-if="
									field.type === 'array' || field.type === 'object' || isUnknown(field.type)
								"
								v-model="formValues[formKey(tool, field.key)]"
								:placeholder="formatMessage(messages.jsonPlaceholder)"
								rows="3"
								class="rounded-lg border border-divider bg-bg px-3 py-1.5 font-mono text-sm text-contrast outline-none focus:border-brand"
							/>

							<input
								v-else
								v-model="formValues[formKey(tool, field.key)]"
								:type="field.type === 'number' ? 'number' : 'text'"
								class="rounded-lg border border-divider bg-bg px-3 py-1.5 text-sm text-contrast outline-none focus:border-brand"
							/>

							<p v-if="field.description" class="text-xs text-secondary">
								{{ field.description }}
							</p>
						</div>

						<div v-if="error[tool.name]" class="text-xs text-red-500" role="alert">
							{{ error[tool.name] }}
						</div>

						<Button type="colored" size="sm" :loading="executing[tool.name]" native-type="submit">
							{{ formatMessage(messages.run) }}
						</Button>
					</form>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { reactive } from 'vue'

import type { ToolInfo } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

defineOptions({
	name: 'AiToolsView',
})

interface ParamField {
	key: string
	label: string
	type: string
	required: boolean
	description?: string
}

const { formatMessage } = useVIntl()
const store = useAiWorkshopStore()

const messages = defineMessages({
	title: {
		id: 'ai.tools.title',
		defaultMessage: '工具面板',
	},
	empty: {
		id: 'ai.tools.empty',
		defaultMessage: '暂无工具',
	},
	confirmBadge: {
		id: 'ai.tools.confirm-badge',
		defaultMessage: '需确认',
	},
	execute: {
		id: 'ai.tools.execute',
		defaultMessage: '执行',
	},
	cancel: {
		id: 'ai.tools.cancel',
		defaultMessage: '取消',
	},
	run: {
		id: 'ai.tools.run',
		defaultMessage: '运行',
	},
	jsonPlaceholder: {
		id: 'ai.tools.json-placeholder',
		defaultMessage: 'JSON 数组/对象…',
	},
	invalidJson: {
		id: 'ai.tools.invalid-json',
		defaultMessage: '无效的 JSON',
	},
	requiredField: {
		id: 'ai.tools.required-field',
		defaultMessage: '此项为必填',
	},
})

const openTool = reactive(new Set<string>())
const formValues = reactive<Record<string, string>>({})
const booleanValues = reactive<Record<string, boolean>>({})
const executing = reactive<Record<string, boolean>>({})
const error = reactive<Record<string, string>>({})

const isOpen = (tool: ToolInfo) => openTool.has(tool.name)

const formKey = (tool: ToolInfo, key: string) => `${tool.name}.${key}`

const isUnknown = (type: string) =>
	type !== 'string' &&
	type !== 'number' &&
	type !== 'boolean' &&
	type !== 'array' &&
	type !== 'object'

const schemaFields = (tool: ToolInfo): ParamField[] => {
	const schema = tool.paramsSchema as
		| { properties?: Record<string, unknown>; required?: string[] }
		| null
		| undefined
	if (!schema || typeof schema !== 'object') return []
	const properties = (schema.properties ?? {}) as Record<
		string,
		{ type?: string; title?: string; description?: string }
	>
	const required = Array.isArray(schema.required) ? schema.required : []
	return Object.entries(properties).map(([key, prop]) => ({
		key,
		label: prop.title ?? key,
		type: prop.type ?? 'string',
		required: required.includes(key),
		description: prop.description,
	}))
}

const toggleForm = (tool: ToolInfo) => {
	const key = tool.name
	error[key] = ''
	if (executing[key]) return
	if (openTool.has(key)) {
		openTool.delete(key)
		return
	}
	openTool.add(key)
	if (schemaFields(tool).length === 0) {
		void submit(tool)
		openTool.delete(key)
	}
}

const parseValue = (field: ParamField, raw: string): unknown => {
	const trimmed = raw.trim()
	if (field.type === 'number') {
		if (trimmed === '') return undefined
		const value = Number(trimmed)
		return Number.isNaN(value) ? undefined : value
	}
	if (field.type === 'array' || field.type === 'object') {
		if (trimmed === '') return undefined
		try {
			return JSON.parse(trimmed)
		} catch {
			throw new Error('invalid-json')
		}
	}
	if (isUnknown(field.type)) {
		if (trimmed === '') return undefined
		try {
			return JSON.parse(trimmed)
		} catch {
			return trimmed
		}
	}
	return raw
}

const submit = async (tool: ToolInfo) => {
	const fields = schemaFields(tool)
	const params: Record<string, unknown> = {}
	error[tool.name] = ''

	for (const field of fields) {
		if (field.type === 'boolean') {
			params[field.key] = booleanValues[formKey(tool, field.key)] ?? false
			continue
		}
		const raw = formValues[formKey(tool, field.key)] ?? ''
		try {
			const value = parseValue(field, raw)
			if (value !== undefined) {
				params[field.key] = value
			} else if (field.required) {
				error[tool.name] = formatMessage(messages.requiredField)
				return
			}
		} catch {
			error[tool.name] = formatMessage(messages.invalidJson)
			return
		}
	}

	executing[tool.name] = true
	try {
		await store.executeTool(tool.name, fields.length === 0 ? undefined : params)
		openTool.delete(tool.name)
	} finally {
		executing[tool.name] = false
	}
}
</script>
