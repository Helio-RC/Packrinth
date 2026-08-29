<template>
	<div class="flex flex-col gap-4 p-4">
		<div v-if="store.aiConfig" class="flex flex-col gap-4">
			<section>
				<h3 class="mb-2 text-sm font-semibold text-contrast">
					{{ formatMessage(messages.providers) }}
				</h3>
				<div class="flex flex-col gap-2">
					<label
						v-for="[name, config] in providerEntries"
						:key="name"
						class="flex flex-col gap-1 rounded-lg border border-divider bg-bg p-3"
					>
						<div class="flex items-center justify-between gap-2">
							<span class="text-sm font-medium text-contrast">{{ name }}</span>
							<input
								type="checkbox"
								:checked="config.enabled"
								:disabled="name === 'custom' && !config.baseUrl"
								class="size-4 accent-[var(--brand-color)]"
								@change="(e) => setEnabled(name, (e.target as HTMLInputElement).checked)"
							/>
						</div>
						<div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
							<input
								v-model="config.model"
								class="rounded-lg border border-divider bg-bg px-2 py-1.5 text-sm text-contrast outline-none focus:border-brand disabled:opacity-50"
								:placeholder="formatMessage(messages.model)"
								@change="saveProvider(name)"
							/>
							<input
								v-if="name === 'custom'"
								v-model="config.baseUrl"
								class="rounded-lg border border-divider bg-bg px-2 py-1.5 text-sm text-contrast outline-none focus:border-brand"
								:placeholder="formatMessage(messages.baseUrl)"
								@change="saveProvider(name)"
							/>
							<input
								v-if="name === 'ollama'"
								v-model="config.baseUrl"
								class="rounded-lg border border-divider bg-bg px-2 py-1.5 text-sm text-contrast outline-none focus:border-brand"
								:placeholder="formatMessage(messages.ollamaUrl)"
								@change="saveProvider(name)"
							/>
						</div>
						<div class="flex items-center gap-2">
							<input
								v-model="apiKeys[name]"
								type="password"
								class="flex-1 rounded-lg border border-divider bg-bg px-2 py-1.5 text-sm text-contrast outline-none focus:border-brand"
								:placeholder="
									config.apiKeyHint
										? `${formatMessage(messages.key)} (${config.apiKeyHint})`
										: formatMessage(messages.key)
								"
							/>
							<Button type="outlined" size="sm" @click="saveKey(name)">
								{{ formatMessage(messages.saveKey) }}
							</Button>
							<Button type="quiet" size="sm" @click="test(name)">
								{{ formatMessage(messages.test) }}
							</Button>
						</div>
						<p
							v-if="testResults[name]"
							class="text-xs"
							:class="testResults[name]?.ok ? 'text-green-600' : 'text-red-600'"
						>
							{{
								testResults[name]?.ok
									? formatMessage(messages.testOk)
									: (testResults[name]?.error ?? formatMessage(messages.testFail))
							}}
						</p>
					</label>
				</div>
			</section>

			<section>
				<h3 class="mb-2 text-sm font-semibold text-contrast">
					{{ formatMessage(messages.defaultProvider) }}
				</h3>
				<select
					v-model="defaultProvider"
					class="w-64 rounded-lg border border-divider bg-bg px-2 py-1.5 text-sm text-contrast outline-none focus:border-brand"
					@change="saveDefaultProvider"
				>
					<option :value="null">{{ formatMessage(messages.none) }}</option>
					<option v-for="name in providerNames" :key="name" :value="name">{{ name }}</option>
				</select>
			</section>
		</div>
		<div v-else class="flex flex-col gap-2">
			<p class="text-sm text-secondary">{{ formatMessage(messages.loading) }}</p>
			<p v-if="errorMessages.length > 0" class="text-xs text-red-600">
				{{ errorMessages.join('; ') }}
			</p>
			<Button v-if="errorMessages.length > 0" type="outlined" size="sm" @click="retry">
				{{ formatMessage(messages.retry) }}
			</Button>
		</div>
	</div>
</template>

<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, reactive, watch } from 'vue'

import { setProviderApiKey, testProviderConnection } from '@/lib/ai/client'
import type { ProviderConfig } from '@/lib/ai/types'
import { useAiWorkshopStore } from '@/stores/aiWorkshop'

const store = useAiWorkshopStore()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	providers: {
		id: 'ai.settings.providers',
		defaultMessage: 'Providers',
	},
	model: {
		id: 'ai.settings.model',
		defaultMessage: 'Model',
	},
	baseUrl: {
		id: 'ai.settings.base-url',
		defaultMessage: 'Base URL',
	},
	ollamaUrl: {
		id: 'ai.settings.ollama-url',
		defaultMessage: 'http://localhost:11434',
	},
	key: {
		id: 'ai.settings.key',
		defaultMessage: 'API Key',
	},
	saveKey: {
		id: 'ai.settings.save-key',
		defaultMessage: 'Save key',
	},
	test: {
		id: 'ai.settings.test',
		defaultMessage: 'Test connection',
	},
	testOk: {
		id: 'ai.settings.test-ok',
		defaultMessage: 'Connection OK',
	},
	testFail: {
		id: 'ai.settings.test-fail',
		defaultMessage: 'Connection failed',
	},
	defaultProvider: {
		id: 'ai.settings.default-provider',
		defaultMessage: 'Default provider',
	},
	none: {
		id: 'ai.settings.none',
		defaultMessage: 'None',
	},
	loading: {
		id: 'ai.settings.loading',
		defaultMessage: 'Loading AI configuration…',
	},
	retry: {
		id: 'ai.settings.retry',
		defaultMessage: 'Retry',
	},
})

const errorMessages = computed(() => Object.values(store.initErrors))

const retry = async () => {
	store.initErrors = {}
	await store.init()
}

const providerNames = computed(() => Object.keys(store.aiConfig?.providers ?? {}))
const providerEntries = computed(
	() => Object.entries(store.aiConfig?.providers ?? {}) as [string, ProviderConfig][],
)
const defaultProvider = computed<null | string>({
	get: () => store.aiConfig?.defaultProvider ?? null,
	set: (value) => void store.updateConfig({ defaultProvider: value }),
})

const apiKeys = reactive<Record<string, string>>({})
const testResults = reactive<Record<string, { ok: boolean; error?: string }>>({})

watch(
	providerEntries,
	() => {
		for (const name of providerNames.value) {
			apiKeys[name] ??= ''
		}
	},
	{ immediate: true, deep: true },
)

const setEnabled = async (name: string, enabled: boolean) => {
	const providers = store.aiConfig?.providers
	if (!providers) return
	await store.updateConfig({
		providers: { ...providers, [name]: { ...providers[name], enabled } },
	})
}

const saveProvider = async (name: string) => {
	const providers = store.aiConfig?.providers
	if (!providers) return
	await store.updateConfig({
		providers: { ...providers, [name]: { ...providers[name] } },
	})
}

const saveKey = async (name: string) => {
	const key = apiKeys[name]
	if (!key) return
	try {
		await setProviderApiKey(name, key)
		apiKeys[name] = ''
		await store.loadConfig()
	} catch (err) {
		testResults[name] = { ok: false, error: err instanceof Error ? err.message : String(err) }
	}
}

const test = async (name: string) => {
	testResults[name] = { ok: false }
	try {
		const result = await testProviderConnection(name)
		testResults[name] = result.ok ? { ok: true } : { ok: false, error: result.error }
	} catch (err) {
		testResults[name] = {
			ok: false,
			error: err instanceof Error ? err.message : String(err),
		}
	}
}

const saveDefaultProvider = () => {
	void store.updateConfig({ defaultProvider: defaultProvider.value })
}
</script>
