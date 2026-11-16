window.WebAgentAdapters = {
	'aistudio.google.com': {
		name: 'Google AI Studio',
		inputSelectors: [
			'textarea[aria-label*="prompt" i]',
			'textarea[aria-label*="Enter a prompt" i]',
			'textarea[aria-label*="Type something" i]',
			'textarea.cdk-textarea-autosize',
			'ms-prompt-editor textarea',
			'ms-prompt-box textarea',
			'ms-chunk-editor textarea',
			'textarea[placeholder*="Start typing" i]',
			'div[contenteditable="true"]',
			'.ql-editor',
			'textarea',
		],
		sendMethod: 'ctrl_enter',
		sendButtonSelectors: [
			'footer button.run-button',
			'ms-prompt-box button.run-button',
			'.buttons-row button.run-button',
			'button.run-button',
			'button[data-test-id="run-button"]',
			'button[aria-label="Run prompt" i]',
			'button[aria-label="Run Ctrl+Enter" i]',
		],
		stopIndicator: 'button[aria-label="Stop" i], button[aria-label="Cancel" i], button[aria-label*="Остановить" i]',
		responseContainer:
			'ms-chat-turn.model-turn, ms-text-chunk, div[data-turn-role="model"], .model-turn, .chat-turn, ms-chat-turn',
		fileInputSelectors: [
			'ms-prompt-editor input[type="file"]',
			'ms-prompt-box input[type="file"]',
			'input[type="file"]',
		],
		attachButtonSelectors: [
			'button[aria-label*="Insert" i]',
			'button[mattooltip*="Insert" i]',
			'button[aria-label*="Add" i]',
			'button.insert-button',
			'button[aria-label*="File" i]',
		],
		dropZoneSelectors: [
			'ms-prompt-editor',
			'div.prompt-editor-container',
			'textarea',
			'div[role="region"]',
		],
	},
	'chat.deepseek.com': {
		name: 'DeepSeek',
		inputSelectors: [
			'textarea#chat-input',
			'textarea[placeholder*="DeepSeek" i]',
			'textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: [
			'div[role="button"][aria-disabled="false"]',
			'button[type="submit"]',
		],
		stopIndicator: '.ds-icon-button[aria-label="Stop"], [class*="stop"]',
		responseContainer:
			'.ds-markdown, [data-message-author="assistant"], [class*="message-content"], .chat-message, .markdown, .prose',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: [
			'div[class*="upload"]',
			'button[class*="upload"]',
			'[aria-label*="upload" i]',
		],
		dropZoneSelectors: ['textarea#chat-input', '.chat-input-container', 'textarea'],
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
		responseContainer:
			'.markdown, .prose, [data-message-author-role="assistant"]',
		fileInputSelectors: [
			'#file-upload-input',
			'input[type="file"][tabindex="-1"]',
			'input[type="file"]',
		],
		attachButtonSelectors: [
			'button[data-testid="attach-button"]',
			'button[aria-label*="Attach" i]',
			'button[aria-label*="Add content" i]',
		],
		dropZoneSelectors: ['#prompt-textarea', 'form.w-full', 'form'],
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
			'button[aria-label*="Send message" i]',
			'button[aria-label*="Send" i]',
			'button[aria-label*="Отправить" i]',
			'button[mattooltip*="Send" i]',
			'button[mattooltip*="Отправить" i]',
			'.send-button',
		],
		stopIndicator:
			'button[aria-label*="Stop generating" i], button[aria-label*="Stop" i], button[aria-label*="Остановить" i]',
		responseContainer:
			'message-content, model-response, .model-response-text, [class*="model-response"], .response-container',
		fileInputSelectors: ['uploader input[type="file"]', 'input[type="file"]'],
		attachButtonSelectors: [
			'button[aria-label*="Upload" i]',
			'button[aria-label*="Add files" i]',
			'button[mattooltip*="Upload" i]',
		],
		dropZoneSelectors: ['rich-textarea', 'div[role="textbox"]', 'textarea'],
	},
	'claude.ai': {
		name: 'Claude',
		inputSelectors: [
			'.ProseMirror[contenteditable="true"]',
			'div[contenteditable="true"]',
			'textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[aria-label*="Send Message" i]'],
		stopIndicator: 'button[aria-label*="Stop generating" i]',
		responseContainer:
			'.font-claude-response, .font-claude-message, [data-message-author="assistant"], .prose, .markdown',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: [
			'button[aria-label*="Add content" i]',
			'button[aria-label*="Attach" i]',
			'button[aria-label*="Upload" i]',
		],
		dropZoneSelectors: ['.ProseMirror', 'fieldset', 'div[data-drop-zone="true"]'],
	},
	'chat.qwen.ai': {
		name: 'Qwen',
		inputSelectors: [
			'textarea',
			'div[contenteditable="true"]',
			'[role="textbox"]',
			'.chat-input textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: [
			'button[type="submit"]',
			'.send-btn',
			'button[class*="send"]',
			'[aria-label*="send" i]',
		],
		stopIndicator: 'button.stop-btn, button[aria-label*="stop" i]',
		responseContainer:
			'.qwen-markdown, [data-message-author="assistant"], .message-assistant, [class*="message-content"], [class*="chat-message"], .markdown, .prose',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: [
			'button[class*="upload"]',
			'div[class*="attachment"]',
			'button[aria-label*="upload" i]',
		],
		dropZoneSelectors: ['textarea', 'div[contenteditable="true"]'],
	},
	'chat.qwenlm.ai': {
		name: 'Qwen',
		inputSelectors: [
			'textarea',
			'div[contenteditable="true"]',
			'[role="textbox"]',
			'.chat-input textarea',
		],
		sendMethod: 'enter',
		sendButtonSelectors: [
			'button[type="submit"]',
			'.send-btn',
			'button[class*="send"]',
			'[aria-label*="send" i]',
		],
		stopIndicator: 'button.stop-btn, button[aria-label*="stop" i]',
		responseContainer:
			'.qwen-markdown, [data-message-author="assistant"], .message-assistant, [class*="message-content"], [class*="chat-message"], .markdown, .prose',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: [
			'button[class*="upload"]',
			'div[class*="attachment"]',
			'button[aria-label*="upload" i]',
		],
		dropZoneSelectors: ['textarea', 'div[contenteditable="true"]'],
	},
	'kimi.moonshot.cn': {
		name: 'Kimi',
		inputSelectors: ['div[contenteditable="true"]', 'textarea'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[class*="send"]', 'button[type="submit"]'],
		stopIndicator: 'button[class*="stop"]',
		responseContainer:
			'[class*="chat-content"], [class*="markdown"], [class*="message"]',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: ['button[class*="upload"]', 'span[class*="upload"]'],
		dropZoneSelectors: ['div[contenteditable="true"]', 'textarea'],
	},
	'z.ai': {
		name: 'Z.ai',
		inputSelectors: ['textarea', 'div[contenteditable="true"]'],
		sendMethod: 'enter',
		sendButtonSelectors: ['button[type="submit"]'],
		stopIndicator: 'button[class*="stop"]',
		responseContainer:
			'[class*="markdown-body"], [class*="message"], [class*="content"]',
		fileInputSelectors: ['input[type="file"]'],
		attachButtonSelectors: [
			'button[class*="upload"]',
			'button[aria-label*="upload" i]',
		],
		dropZoneSelectors: ['textarea', 'div[contenteditable="true"]'],
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
				'button[aria-label*="Send" i]',
				'button[class*="send" i]',
			],
			stopIndicator: '[aria-label*="Stop" i], [class*="stop" i]',
			responseContainer:
				'[class*="message"], [class*="response"], [class*="markdown"], .prose, ms-chat-turn, message-content',
			fileInputSelectors: ['input[type="file"]'],
			attachButtonSelectors: [
				'button[aria-label*="upload" i]',
				'button[aria-label*="attach" i]',
				'button[aria-label*="file" i]',
			],
			dropZoneSelectors: ['textarea', 'div[contenteditable="true"]', '[role="textbox"]'],
		}
	},
}
