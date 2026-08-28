import { resolve } from 'path'
import { defineConfig } from 'vitest/config'

const projectRootDir = resolve(__dirname)

export default defineConfig({
	resolve: {
		alias: [
			{
				find: '@',
				replacement: resolve(projectRootDir, 'src'),
			},
		],
	},
	test: {
		include: ['tests/**/*.spec.ts'],
		environment: 'node',
	},
})
