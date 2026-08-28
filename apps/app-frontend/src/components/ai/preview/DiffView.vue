<template>
	<div class="flex flex-col gap-1 font-mono text-xs">
		<div
			v-for="(row, index) in rows"
			:key="index"
			class="flex gap-2 rounded px-2 py-0.5"
			:class="rowClass(row.kind)"
		>
			<span class="w-6 shrink-0 text-right text-secondary select-none">{{ row.kind }}</span>
			<pre class="whitespace-pre-wrap break-all flex-1 text-contrast">{{ row.text }}</pre>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface DiffRow {
	kind: '+' | '-' | ' '
	text: string
}

const props = defineProps<{
	before: string
	after: string
}>()

/** 朴素线性 diff：以 before 为基准逐行对比 after，输出左侧删除/右侧新增的近邻行序列。 */
function computeRows(before: string, after: string): DiffRow[] {
	const left = before.split('\n')
	const right = after.split('\n')
	const rows: DiffRow[] = []
	let i = 0
	let j = 0
	while (i < left.length && j < right.length) {
		if (left[i] === right[j]) {
			rows.push({ kind: ' ', text: left[i] })
			i += 1
			j += 1
		} else {
			// 向前查找匹配行，匹配前的左侧为删除、右侧为新增。
			let lookAhead = 1
			while (
				lookAhead < Math.min(left.length - i, right.length - j) &&
				left[i + lookAhead] !== right[j]
			) {
				lookAhead += 1
			}
			const matchedLeft = i + lookAhead < left.length && left[i + lookAhead] === right[j]
			if (matchedLeft) {
				for (let k = i; k < i + lookAhead; k++) rows.push({ kind: '-', text: left[k] })
				i += lookAhead
			} else {
				rows.push({ kind: '-', text: left[i] })
				i += 1
			}
		}
	}
	while (i < left.length) {
		rows.push({ kind: '-', text: left[i] })
		i += 1
	}
	while (j < right.length) {
		rows.push({ kind: '+', text: right[j] })
		j += 1
	}
	return rows
}

const rows = computed(() => computeRows(props.before, props.after))

const rowClass = (kind: DiffRow['kind']) => {
	if (kind === '+') return 'bg-green-500/10'
	if (kind === '-') return 'bg-red-500/10'
	return ''
}
</script>
