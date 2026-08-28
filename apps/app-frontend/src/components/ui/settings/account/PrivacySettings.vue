<script setup lang="ts">
import { defineMessages, Toggle, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import { optInAnalytics, optOutAnalytics } from '@/helpers/analytics'
import { get, set } from '@/helpers/settings.ts'

const { formatMessage } = useVIntl()
const settings = ref(await get())

const messages = defineMessages({
	telemetryTitle: {
		id: 'app.settings.privacy.telemetry.title',
		defaultMessage: 'Telemetry',
	},
	telemetryDescription: {
		id: 'app.settings.privacy.telemetry.description',
		defaultMessage:
			'Packrinth collects anonymized analytics and usage data to improve our user experience and customize your experience. By disabling this option, you opt out and your data will no longer be collected.',
	},
})

watch(
	settings,
	async () => {
		if (settings.value.telemetry) {
			optInAnalytics()
		} else {
			optOutAnalytics()
		}

		await set(settings.value)
	},
	{ deep: true },
)
</script>

<template>
	<div class="mt-8 first:mt-0 flex items-center justify-between gap-4">
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.telemetryTitle) }}
			</h2>
			<p class="m-0 mt-1">
				{{ formatMessage(messages.telemetryDescription) }}
			</p>
		</div>
		<Toggle id="opt-out-analytics" v-model="settings.telemetry" />
	</div>
</template>
