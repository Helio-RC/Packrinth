/**
 * markdown-it 单例：将模型输出的 Markdown 渲染为受信任的 HTML。
 * 通过关闭 raw HTML、限制链接协议与清洗事件属性做基础 XSS 防护。
 */
import MarkdownIt from 'markdown-it'

const md = new MarkdownIt({
	html: false,
	linkify: true,
	breaks: true,
})

md.validateLink = (url: string) => {
	const normalized = url.toLowerCase()
	return normalized.startsWith('https://') || normalized.startsWith('http://')
}

/** 移除可能残留的内联事件处理器。 */
function stripEventHandlers(html: string): string {
	return html.replace(/\son\w+\s*=\s*(?:"[^"]*"|'[^']*'|\S+)/gi, '')
}

/**
 * 将 Markdown 源文本渲染为安全的 HTML 字符串。
 * @param html 待渲染的 Markdown 源文本
 */
export function renderMarkdown(source: string): string {
	return stripEventHandlers(md.render(source))
}
