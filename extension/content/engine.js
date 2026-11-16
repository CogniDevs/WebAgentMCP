;(function () {
	if (window !== window.top) return
	if (window.__webAgentEngineInitialized) return
	window.__webAgentEngineInitialized = true

	const adapter = window.WebAgentAdapters.getActiveAdapter()
	let isExecuting = false
	let lastProcessedHash = ''
	let unchangedTicks = 0
	let previousLength = 0

	// Deep text extractor that pierces Shadow DOM boundaries (crucial for Gemini & AI Studio)
	function getDeepText(node) {
		if (!node) return ''
		let text = ''
		if (node.nodeType === Node.TEXT_NODE) {
			text += node.textContent
		} else if (node.nodeType === Node.ELEMENT_NODE) {
			if (node.shadowRoot) {
				text += getDeepText(node.shadowRoot)
			}
			for (let child of node.childNodes) {
				text += getDeepText(child)
			}
		}
		return text
	}

	// Passive discovery & calibration: capture manually used file inputs and drop zones
	document.addEventListener(
		'change',
		e => {
			if (e.target && e.target.type === 'file') {
				window.__webAgentActiveFileInput = e.target
				console.log('[WebAgentMCP] Calibrated file input element:', e.target)
			}
		},
		true,
	)

	document.addEventListener(
		'drop',
		e => {
			if (
				e.dataTransfer &&
				e.dataTransfer.files &&
				e.dataTransfer.files.length > 0
			) {
				window.__webAgentActiveDropZone = e.target
				console.log('[WebAgentMCP] Calibrated drop zone element:', e.target)
			}
		},
		true,
	)

	function findInput() {
		for (const sel of adapter.inputSelectors) {
			const el = document.querySelector(sel)
			if (el && el.offsetParent !== null) return el
		}
		return document.querySelector('textarea, div[contenteditable="true"]')
	}

	function injectText(input, text) {
		input.focus()
		if (input.tagName === 'TEXTAREA' || input.tagName === 'INPUT') {
			const lastVal = input.value
			const nativeSetter =
				Object.getOwnPropertyDescriptor(
					window.HTMLTextAreaElement.prototype,
					'value',
				)?.set ||
				Object.getOwnPropertyDescriptor(
					window.HTMLInputElement.prototype,
					'value',
				)?.set

			if (nativeSetter) {
				nativeSetter.call(input, text)
			} else {
				input.value = text
			}

			// Inform React 16+ internal state tracker
			const tracker = input._valueTracker
			if (tracker) {
				tracker.setValue(lastVal)
			}

			input.dispatchEvent(new Event('input', { bubbles: true, composed: true }))
			input.dispatchEvent(
				new InputEvent('input', {
					bubbles: true,
					composed: true,
					inputType: 'insertText',
					data: text,
				}),
			)
			input.dispatchEvent(
				new Event('change', { bubbles: true, composed: true }),
			)
			input.dispatchEvent(
				new KeyboardEvent('keyup', { bubbles: true, composed: true }),
			)
		} else {
			// Rich text & contenteditable (Lexical, Slate, ProseMirror)
			input.focus()
			const sel = window.getSelection()
			const range = document.createRange()
			range.selectNodeContents(input)
			sel.removeAllRanges()
			sel.addRange(range)

			let inserted = false
			try {
				inserted = document.execCommand('insertText', false, text)
			} catch (_) {}

			if (!inserted) {
				const dataTransfer = new DataTransfer()
				dataTransfer.setData('text/plain', text)
				const pasteEvent = new ClipboardEvent('paste', {
					clipboardData: dataTransfer,
					bubbles: true,
					cancelable: true,
				})
				input.dispatchEvent(pasteEvent)
				if (input.textContent.trim() === '') {
					input.innerText = text
				}
			}

			input.dispatchEvent(
				new InputEvent('input', {
					bubbles: true,
					composed: true,
					inputType: 'insertText',
					data: text,
				}),
			)
			input.dispatchEvent(new Event('input', { bubbles: true, composed: true }))
			input.dispatchEvent(new Event('change', { bubbles: true, composed: true }))
		}
	}

	function tryClickSend() {
		if (adapter.sendButtonSelectors) {
			for (const btnSel of adapter.sendButtonSelectors) {
				const buttons = document.querySelectorAll(btnSel)
				for (const btn of buttons) {
					if (
						btn &&
						!btn.disabled &&
						btn.getAttribute('aria-disabled') !== 'true'
					) {
						const aria = (btn.getAttribute('aria-label') || '').toLowerCase()
						const title = (btn.getAttribute('title') || '').toLowerCase()
						const cls = (btn.className || '').toLowerCase()
						if (aria.includes('setting') || title.includes('setting') || cls.includes('setting')) {
							continue
						}
						btn.click()
						return true
					}
				}
			}
		}
		return false
	}

	function triggerSubmit(input) {
		// Anti-Ban human-like jitter delay (350ms - 800ms) before submission
		const jitter = Math.floor(Math.random() * 450) + 350
		setTimeout(() => {
			if (tryClickSend()) return

			const isCtrl = adapter.sendMethod === 'ctrl_enter'
			const keyEventProps = {
				key: 'Enter',
				code: 'Enter',
				keyCode: 13,
				which: 13,
				ctrlKey: isCtrl,
				bubbles: true,
				cancelable: true,
				composed: true,
			}

			input.dispatchEvent(new KeyboardEvent('keydown', keyEventProps))
			input.dispatchEvent(new KeyboardEvent('keypress', keyEventProps))
			input.dispatchEvent(new KeyboardEvent('keyup', keyEventProps))
		}, jitter)
	}

	function injectAndSubmit(text) {
		const input = findInput()
		if (!input) {
			console.warn('[WebAgentMCP] Could not find prompt input on page')
			return
		}
		injectText(input, text)
		triggerSubmit(input)
	}

	function escapeXml(str) {
		return (str || '').replace(/[<>&'"]/g, c => {
			switch (c) {
				case '<':
					return '&lt;'
				case '>':
					return '&gt;'
				case '&':
					return '&amp;'
				case "'":
					return '&apos;'
				case '"':
					return '&quot;'
			}
		})
	}

	async function attachTextFile(fileName, content) {
		const blob = new Blob([content], { type: 'text/plain;charset=utf-8' })
		const file = new File([blob], fileName, {
			type: 'text/plain;charset=utf-8',
		})
		const dt = new DataTransfer()
		dt.items.add(file)

		// 1. Check dynamically calibrated file input from user actions
		let fileInput = window.__webAgentActiveFileInput

		// 2. Search configured adapter fileInputSelectors
		if (!fileInput && adapter.fileInputSelectors) {
			for (const sel of adapter.fileInputSelectors) {
				const el = document.querySelector(sel)
				if (el) {
					fileInput = el
					break
				}
			}
		}

		// 3. Fallback: query document for any input[type="file"]
		if (!fileInput) {
			fileInput = document.querySelector('input[type="file"]')
		}

		// 4. If input not rendered yet, trigger attach/plus button to mount it
		if (!fileInput && adapter.attachButtonSelectors) {
			for (const btnSel of adapter.attachButtonSelectors) {
				const btn = document.querySelector(btnSel)
				if (btn) {
					btn.click()
					await new Promise(r => setTimeout(r, 250))
					fileInput = document.querySelector('input[type="file"]')
					if (fileInput) break
				}
			}
		}

		// If input found, dispatch files
		if (fileInput) {
			try {
				fileInput.files = dt.files
				fileInput.dispatchEvent(
					new Event('change', { bubbles: true, composed: true }),
				)
				fileInput.dispatchEvent(
					new Event('input', { bubbles: true, composed: true }),
				)
				return true
			} catch (err) {
				console.warn('[WebAgentMCP] Error assigning to file input:', err)
			}
		}

		// 5. Fallback: Drag-and-drop onto drop zone
		let dropZone = window.__webAgentActiveDropZone
		if (!dropZone && adapter.dropZoneSelectors) {
			for (const sel of adapter.dropZoneSelectors) {
				const el = document.querySelector(sel)
				if (el && el.offsetParent !== null) {
					dropZone = el
					break
				}
			}
		}
		if (!dropZone) {
			dropZone = findInput()
		}

		if (dropZone) {
			try {
				const dragEnter = new DragEvent('dragenter', {
					dataTransfer: dt,
					bubbles: true,
					cancelable: true,
				})
				const dragOver = new DragEvent('dragover', {
					dataTransfer: dt,
					bubbles: true,
					cancelable: true,
				})
				const dropEvent = new DragEvent('drop', {
					dataTransfer: dt,
					bubbles: true,
					cancelable: true,
				})

				dropZone.dispatchEvent(dragEnter)
				dropZone.dispatchEvent(dragOver)
				dropZone.dispatchEvent(dropEvent)
				return true
			} catch (err) {
				console.warn('[WebAgentMCP] Error dispatching drop event:', err)
			}
		}

		return false
	}

	// Watchdog for stuck system messages
	setInterval(() => {
		const input = findInput()
		if (!input) return

		const text =
			input.tagName === 'TEXTAREA' || input.tagName === 'INPUT'
				? input.value
				: input.textContent

		if (
			text &&
			(text.includes('[System - Tool Execution') ||
				text.includes('[System - Image Saved]') ||
				text.includes('[System Notice]:'))
		) {
			const stopBtn = adapter.stopIndicator
				? document.querySelector(adapter.stopIndicator)
				: null
			if (!stopBtn) {
				if (!tryClickSend()) {
					input.dispatchEvent(
						new Event('input', { bubbles: true, composed: true }),
					)
					input.dispatchEvent(
						new KeyboardEvent('keydown', {
							bubbles: true,
							cancelable: true,
							key: 'Enter',
							code: 'Enter',
						}),
					)
				}
			}
		}
	}, 1000)

	function extractToolsFromContainer(container) {
		const tools = []
		const seen = new Set()

		const knownTools = [
			'terminal_run', 'terminal_bg', 'terminal_logs', 'terminal_kill',
			'cron_monitor', 'read', 'read_file', 'read_lines', 'view_dir',
			'search_code', 'write', 'patch', 'replace', 'mcp_call'
		]

		function pushTool(rawIdent, rawBody) {
			const cleanIdent = (rawIdent || '').trim()
			const cleanBody = (rawBody || '').trim()
			if (!cleanIdent && !cleanBody) return

			let baseName = cleanIdent
				.replace(/^[<`'"]+/, '')
				.replace(/^run:/i, '')
				.replace(/^tool:/i, '')
				.split(/[\s='":]+/)[0]
				.toLowerCase()

			if (knownTools.includes(baseName) || cleanIdent.startsWith('run:') || cleanIdent.startsWith('tool:') || cleanIdent.startsWith('tool=')) {
				const sig = `${baseName}:::${cleanBody}`
				if (!seen.has(sig)) {
					seen.add(sig)
					tools.push({
						identifier: cleanIdent || baseName,
						name: baseName,
						payload: cleanBody
					})
				}
			}
		}

		// 1. DOM Inspection: Google AI Studio (<ms-code-block>)
		const aiStudioBlocks = container.querySelectorAll('ms-code-block')
		for (const block of aiStudioBlocks) {
			const lang = block.getAttribute('data-test-language') || block.dataset.language || ''
			const pre = block.querySelector('pre') || block.querySelector('code')
			const codeText = pre ? pre.textContent : ''
			if (lang) {
				pushTool(lang, codeText)
			}
		}

		// 2. DOM Inspection: Generic Code Blocks (Gemini, Qwen, DeepSeek, ChatGPT)
		const codeBlocks = container.querySelectorAll('pre, div.code-block, div[class*="code-block"], .contain-inline-size')
		for (const cb of codeBlocks) {
			const codeEl = cb.tagName === 'PRE' ? (cb.querySelector('code') || cb) : (cb.querySelector('pre code') || cb.querySelector('code') || cb.querySelector('pre'))
			let lang = ''
			if (codeEl) {
				const classNames = (codeEl.className || '') + ' ' + (cb.className || '')
				const m = classNames.match(/language-([a-zA-Z0-9_\-:\.]+)/)
				if (m) lang = m[1]
			}
			const headerEl = cb.querySelector('[class*="header"], [class*="decoration"], [class*="infostring"], span.code-block-language')
			if (!lang && headerEl) {
				lang = headerEl.textContent.trim()
			}

			const codeContent = (codeEl ? codeEl.textContent : cb.textContent) || ''
			const firstLine = codeContent.trim().split('\n')[0].trim()
			if (firstLine.startsWith('run:') || firstLine.startsWith('tool:') || firstLine.startsWith('tool=')) {
				const rest = codeContent.trim().split('\n').slice(1).join('\n').trim()
				pushTool(firstLine, rest || firstLine)
			} else if (lang) {
				pushTool(lang, codeContent)
			}
		}

		// 3. Deep Text XML / Regex Parsing (fallback & handles raw text tools)
		const deepText = getDeepText(container)

		// 3a. Paired XML tags: <tool=terminal_run>...</tool>
		const pairedRegex = /<(?:tool|tool_call|agent_tool)(?:=|\s+name=)?['"]?([a-zA-Z0-9_\-\.\:]+)['"]?([^>]*)>([\s\S]*?)<\/(?:tool|tool_call|agent_tool)>/gi
		let match
		while ((match = pairedRegex.exec(deepText)) !== null) {
			const toolName = match[1]
			const rawAttrs = match[2]
			const payload = match[3].trim()
			pushTool((toolName + ' ' + rawAttrs).trim(), payload)
		}

		// 3b. Single / unclosed XML tags (Gemini: <tool='read' path='README.ru.md'> or <tool='terminal_run'>git status)
		const singleRegex = /<(?:tool|tool_call|agent_tool)(?:=|\s+name=)?['"]?([a-zA-Z0-9_\-\.\:]+)['"]?([^>]*?)(?:\/?>)([\s\S]*?)(?:$|(?=<tool)|(?=\n\n))/gi
		while ((match = singleRegex.exec(deepText)) !== null) {
			const toolName = match[1]
			const rawAttrs = match[2].trim()
			const rest = match[3] ? match[3].trim() : ''
			const payload = rest || rawAttrs
			pushTool((toolName + ' ' + rawAttrs).trim(), payload)
		}

		// 3c. Unrendered raw markdown fences: ```run:terminal_run ... ```
		const mdBlockRegex = /```(?:tool[:=\s]+([a-zA-Z0-9_\-\.\:]+)|run:([a-zA-Z0-9_\-\.\:]+)|([a-zA-Z0-9_\-\.\:]+))([^`]*?)```/gi
		while ((match = mdBlockRegex.exec(deepText)) !== null) {
			const rawName = (match[1] || match[2] || match[3] || '').trim()
			const body = (match[4] || '').trim()
			pushTool(rawName, body)
		}

		return tools
	}

	async function runToolsSequentially(tools) {
		isExecuting = true
		const inlineResults = []
		const xmlBlocks = []

		const storageRes = await new Promise(resolve => {
			chrome.storage.local.get(['authToken'], resolve)
		})
		const token = (
			storageRes && storageRes.authToken ? storageRes.authToken : ''
		).trim()

		for (const t of tools) {
			chrome.runtime.sendMessage({
				target: 'sidepanel',
				type: 'TOOL_CALL_DETECTED',
				tool: t.name,
				payload: t.payload.slice(0, 160),
			})

			if (!token) {
				const noTokenMsg =
					'Error 401: Auth Key is missing in WebAgentMCP side panel. Please enter your Auth Key and click Save.'
				inlineResults.push(
					`[System - Tool Execution: ${t.name}]\nStatus: Error\nOutput:\n${noTokenMsg}\n`,
				)
				xmlBlocks.push(
					`  <tool_result name="${escapeXml(t.name)}" call="${escapeXml(t.identifier)}" status="error">\n<![CDATA[\n${noTokenMsg}\n]]>\n  </tool_result>`,
				)
				chrome.runtime.sendMessage({
					target: 'sidepanel',
					type: 'TOOL_ERROR',
					tool: t.name,
					error: noTokenMsg,
				})
				continue
			}

			try {
				const headers = {
					'Content-Type': 'application/json',
					Authorization: `Bearer ${token}`,
				}

				const response = await fetch('http://127.0.0.1:5505/api/run-tool', {
					method: 'POST',
					headers: headers,
					body: JSON.stringify({ name: t.identifier, payload: t.payload }),
				})

				let outputText = ''
				const responseText = await response.text()

				if (responseText && responseText.trim().length > 0) {
					try {
						const jsonBody = JSON.parse(responseText)
						outputText = jsonBody.output || jsonBody.error || responseText
					} catch (_) {
						outputText = responseText
					}
				} else {
					outputText = `HTTP ${response.status} (Empty Response)`
				}

				const statusStr = response.ok ? 'Success' : 'Error'
				inlineResults.push(
					`[System - Tool Execution: ${t.name}]\nStatus: ${statusStr}\nOutput:\n${outputText}\n`,
				)

				const cleanCdata = outputText.replace(/]]>/g, ']]&gt;')
				xmlBlocks.push(
					`  <tool_result name="${escapeXml(t.name)}" call="${escapeXml(t.identifier)}" status="${statusStr.toLowerCase()}">\n<![CDATA[\n${cleanCdata}\n]]>\n  </tool_result>`,
				)

				if (!response.ok) {
					chrome.runtime.sendMessage({
						target: 'sidepanel',
						type: 'TOOL_ERROR',
						tool: t.name,
						error: outputText,
					})
				} else {
					chrome.runtime.sendMessage({
						target: 'sidepanel',
						type: 'TOOL_RESULT',
						tool: t.name,
						result: outputText,
					})
				}
			} catch (err) {
				const errorMsg = err.message || String(err)
				inlineResults.push(
					`[System - Tool Execution: ${t.name}]\nStatus: Error\nOutput:\n${errorMsg}\n`,
				)
				xmlBlocks.push(
					`  <tool_result name="${escapeXml(t.name)}" call="${escapeXml(t.identifier)}" status="error">\n<![CDATA[\n${errorMsg}\n]]>\n  </tool_result>`,
				)

				chrome.runtime.sendMessage({
					target: 'sidepanel',
					type: 'TOOL_ERROR',
					tool: t.name,
					error: errorMsg,
				})
			}
		}

		const MAX_INLINE_CHARS = 1500
		const combinedInlineText = inlineResults.join(
			'\n------------------------------------\n',
		)

		let responsePrompt = ''
		let attachedFile = false

		if (combinedInlineText.length <= MAX_INLINE_CHARS) {
			responsePrompt = `${combinedInlineText}\n\nInstruction: Review output above and proceed with next action.`
		} else {
			const xmlReport = `<?xml version="1.0" encoding="UTF-8"?>\n<system_execution_report timestamp="${new Date().toISOString()}">\n${xmlBlocks.join('\n')}\n</system_execution_report>`
			const attached = await attachTextFile('tool_results.txt', xmlReport)
			await new Promise(r => setTimeout(r, 500))

			if (attached) {
				attachedFile = true
				responsePrompt = `[System Notice]: Tool execution output exceeded chat inline limit and is attached as tool_results.txt. Review the attached XML report and proceed with the next step.`
			} else {
				responsePrompt = `${combinedInlineText}\n\nInstruction: Review output above and proceed with next action.`
			}
		}

		// Check mode: Auto-send vs Manual Confirmation
		const modeConfig = await new Promise(resolve => {
			chrome.storage.local.get(['autoSendMode'], resolve)
		})
		const isAutoMode = modeConfig.autoSendMode !== false

		if (isAutoMode) {
			injectAndSubmit(responsePrompt)
			setTimeout(() => {
				isExecuting = false
			}, 1200)
		} else {
			// Manual confirmation mode: send to sidepanel and wait for user button click
			chrome.runtime.sendMessage({
				target: 'sidepanel',
				type: 'TOOL_OUTPUT_PENDING',
				prompt: responsePrompt,
				preview: combinedInlineText.slice(0, 300),
			})
		}
	}

	setInterval(() => {
		if (isExecuting) return

		const containers = document.querySelectorAll(adapter.responseContainer)
		if (!containers || containers.length === 0) return

		const last = containers[containers.length - 1]
		if (last.hasAttribute('data-agent-processed')) return

		const currentText = getDeepText(last)
		const currentLength = currentText.length

		let isGenerating = false
		if (adapter.stopIndicator) {
			const stopBtn = document.querySelector(adapter.stopIndicator)
			isGenerating =
				!!stopBtn && (stopBtn.offsetWidth > 0 || stopBtn.offsetHeight > 0)
		}

		if (currentLength > previousLength) {
			previousLength = currentLength
			unchangedTicks = 0
		} else if (currentLength > 0) {
			unchangedTicks++
		}

		// Execute if generation stopped (at least 2 ticks = 1s without change)
		// OR safety fallback: if text unchanged for 5 ticks (2.5s) even if stop indicator has false match
		if ((!isGenerating && unchangedTicks >= 2) || unchangedTicks >= 5) {
			unchangedTicks = 0
			previousLength = 0
			last.setAttribute('data-agent-processed', 'true')
			const tools = extractToolsFromContainer(last)
			if (tools.length > 0) {
				runToolsSequentially(tools)
			}
		}
	}, 500)

	chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
		if (msg.action === 'INJECT_SYSTEM_PROMPT') {
			injectAndSubmit(msg.prompt)
		} else if (msg.action === 'SUBMIT_PENDING_PROMPT') {
			injectAndSubmit(msg.prompt)
			setTimeout(() => {
				isExecuting = false
			}, 1200)
			sendResponse({ ok: true })
		} else if (msg.action === 'DISCARD_PENDING_PROMPT') {
			isExecuting = false
			sendResponse({ ok: true })
		}
	})
})()
