window.WebAgentAdapters = {
	'aistudio.google.com': {
		name: 'Google AI Studio',
		inputSelectors: [
			'textarea[placeholder*="Start typing"]',
			'ms-prompt-editor textarea',
			'textarea[aria-label*="prompt"]',
			'textarea[aria-label*="User prompt"]',
			'div[contenteditable="true"]',
			'.ql-editor',
			'textarea',
		],
		sendMethod: 'ctrl_enter',
		sendButtonSelectors: [
			'button[aria-label*="Run"]',
			'button[data-test-id="run-button"]',
			'button.run-button',
			'.run-button',
		],
		stopIndicator: 'button[aria-label*="Stop"], button[aria-label*="Cancel"]',
		responseContainer:
			'ms-chat-turn.model-turn, ms-text-chunk, div[data-turn-role="model"]',
	},
	'chat.deepseek.com': {
		name: 'DeepSeek',
		inputSelectors: [
			'textarea#chat-input',
			'textarea[placeholder*="DeepSeek"]',
			'textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: [
			'div[role="button"][aria-disabled="false"]',
			'button[type="submit"]',
		],
		stopIndicator: '.ds-icon-button[aria-label="Stop"], [class*="stop"]',
		responseContainer: '.ds-markdown',
	},
	'chatgpt.com': {
		name: 'ChatGPT',
		inputSelectors: [
			'#prompt-textarea',
			'div[contenteditable="true"]',
			'textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: ['[data-testid="send-button"]'],
		stopIndicator: '[data-testid="stop-button"]',
		responseContainer: '.markdown, .prose',
	},
	'gemini.google.com': {
		name: 'Gemini',
		inputSelectors: [
			'rich-textarea div[contenteditable="true"]',
			'div[role="textbox"][contenteditable="true"]',
			'.ql-editor',
			'textarea',
			'input',
		],
		sendMethod: 'enter',
		sendButtonSelectors: [
			'button[aria-label="Send message"]',
			'button[aria-label="Send"]',
		],
		stopIndicator:
			'button[aria-label="Stop generating"], button[aria-label="Stop"]',
		responseContainer: 'message-content, model-response, .model-response-text',
	},
	'claude.ai': {
		name: 'Claude',
		inputSelectors: [
			'.ProseMirror[contenteditable="true"]',
			'div[contenteditable="true"]',
		],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[aria-label="Send Message"]'],
		stopIndicator: 'button[aria-label="Stop generating"]',
		responseContainer: '.font-claude-response, .font-claude-message',
	},
	'chat.qwen.ai': {
		name: 'Qwen',
		inputSelectors: ['textarea', 'div[contenteditable="true"]'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[type="submit"]', '.send-btn'],
		stopIndicator: '.stop-btn',
		responseContainer: '.qwen-markdown',
	},
	'chat.qwenlm.ai': {
		name: 'Qwen',
		inputSelectors: ['textarea', 'div[contenteditable="true"]'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[type="submit"]', '.send-btn'],
		stopIndicator: '.stop-btn',
		responseContainer: '.qwen-markdown',
	},
	'kimi.moonshot.cn': {
		name: 'Kimi',
		inputSelectors: ['div[contenteditable="true"]', 'textarea'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[class*="send"]', 'button[type="submit"]'],
		stopIndicator: 'button[class*="stop"]',
		responseContainer: '[class*="chat-content"], [class*="markdown"]',
	},
	'z.ai': {
		name: 'Z.ai',
		inputSelectors: ['textarea', 'div[contenteditable="true"]'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[type="submit"]'],
		stopIndicator: 'button[class*="stop"]',
		responseContainer: '[class*="markdown-body"]',
	},
	getActiveAdapter() {
		const host = window.location.hostname
		for (const [domain, adapter] of Object.entries(this)) {
			if (typeof adapter === 'object' && host.includes(domain)) {
				return adapter
			}
		}
		return {
			name: 'Generic AI Chat',
			inputSelectors: [
				'textarea',
				'div[contenteditable="true"]',
				'[role="textbox"]',
			],
			sendMethod: 'enter',
			sendButtonSelectors: [
				'button[type="submit"]',
				'button[aria-label*="Send"]',
			],
			stopIndicator: '[aria-label*="Stop"]',
			responseContainer: '[class*="message"], [class*="markdown"], .prose',
		}
	},
}
