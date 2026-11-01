;(function () {
	if (window !== window.top) return
	if (window.__webAgentEngineInitialized) return
	window.__webAgentEngineInitialized = true

	const adapter = window.WebAgentAdapters.getActiveAdapter()
	let isExecuting = false
	let lastProcessedHash = ''
	let unchangedTicks = 0
	let previousLength = 0

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
			input.value = text
			input.dispatchEvent(new Event('input', { bubbles: true, composed: true }))
			input.dispatchEvent(
				new Event('change', { bubbles: true, composed: true }),
			)
		} else {
			const dataTransfer = new DataTransfer()
			dataTransfer.setData('text/plain', text)
			const pasteEvent = new ClipboardEvent('paste', {
				clipboardData: dataTransfer,
				bubbles: true,
				cancelable: true,
			})
			input.dispatchEvent(pasteEvent)
			if (input.textContent.trim() === '') {
				document.execCommand('selectAll', false, null)
				document.execCommand('insertText', false, text)
			}
			input.dispatchEvent(new Event('input', { bubbles: true, composed: true }))
		}
	}

	function tryClickSend() {
		if (adapter.sendButtonSelectors) {
			for (const btnSel of adapter.sendButtonSelectors) {
				const btn = document.querySelector(btnSel)
				if (
					btn &&
					!btn.disabled &&
					btn.getAttribute('aria-disabled') !== 'true'
				) {
					btn.click()
					return true
				}
			}
		}
		return false
	}

	function triggerSubmit(input) {
		setTimeout(() => {
			if (adapter.sendMethod === 'ctrl_enter') {
				input.dispatchEvent(
					new KeyboardEvent('keydown', {
						key: 'Enter',
						code: 'Enter',
						keyCode: 13,
						which: 13,
						ctrlKey: true,
						bubbles: true,
						cancelable: true,
					}),
				)
			} else {
				if (!tryClickSend()) {
					input.dispatchEvent(
						new KeyboardEvent('keydown', {
							key: 'Enter',
							code: 'Enter',
							keyCode: 13,
							which: 13,
							bubbles: true,
							cancelable: true,
						}),
					)
				}
			}
		}, 300)
	}

	function injectAndSubmit(text) {
		const input = findInput()
		if (!input) return
		injectText(input, text)
		triggerSubmit(input)
	}

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
				text.includes('[System - Image Saved]'))
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

	function parseAllTools(text) {
		const tools = []
		const regex =
			/<(?:tool|tool_call)(?:=|\s+name=)?['"]?([a-zA-Z0-9_\-\.\:]+)['"]?([^>]*)>([\s\S]*?)<\/(?:tool|tool_call)>/gi
		let match
		while ((match = regex.exec(text)) !== null) {
			const toolName = match[1]
			const rawAttrs = match[2]
			const payload = match[3].trim()
			const fullIdentifier = (toolName + ' ' + rawAttrs).trim()
			tools.push({
				identifier: fullIdentifier,
				name: toolName,
				payload: payload,
			})
		}
		return tools
	}

	async function runToolsSequentially(tools) {
		isExecuting = true
		const results = []

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
				results.push(
					`[System - Tool Execution: ${t.name}]\nStatus: Error\nOutput:\n${noTokenMsg}\n`,
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

				if (!response.ok) {
					results.push(
						`[System - Tool Execution: ${t.name}]\nStatus: Error\nOutput:\n${outputText}\n`,
					)

					chrome.runtime.sendMessage({
						target: 'sidepanel',
						type: 'TOOL_ERROR',
						tool: t.name,
						error: outputText,
					})
				} else {
					results.push(
						`[System - Tool Execution: ${t.name}]\nStatus: Success\nOutput:\n${outputText}\n`,
					)

					chrome.runtime.sendMessage({
						target: 'sidepanel',
						type: 'TOOL_RESULT',
						tool: t.name,
						result: outputText,
					})
				}
			} catch (err) {
				const errorMsg = err.message || String(err)
				results.push(
					`[System - Tool Execution: ${t.name}]\nStatus: Error\nOutput:\n${errorMsg}\n`,
				)

				chrome.runtime.sendMessage({
					target: 'sidepanel',
					type: 'TOOL_ERROR',
					tool: t.name,
					error: errorMsg,
				})
			}
		}

		const combinedText = results.join(
			'\n------------------------------------\n',
		)
		const responsePrompt = `${combinedText}\n\nInstruction: Review output above and proceed with next action.`

		injectAndSubmit(responsePrompt)

		setTimeout(() => {
			isExecuting = false
		}, 1200)
	}

	setInterval(() => {
		if (isExecuting) return

		const containers = document.querySelectorAll(adapter.responseContainer)
		if (!containers || containers.length === 0) return

		const last = containers[containers.length - 1]
		const currentText = last.innerText || last.textContent || ''
		const currentLength = currentText.length

		const stopBtn = adapter.stopIndicator
			? document.querySelector(adapter.stopIndicator)
			: null
		const isStillGenerating = !!stopBtn

		if (
			!isStillGenerating &&
			currentLength > 0 &&
			currentLength === previousLength
		) {
			unchangedTicks++
			if (unchangedTicks >= 3) {
				unchangedTicks = 0
				const signature = currentText.slice(-300)
				if (signature !== lastProcessedHash) {
					const tools = parseAllTools(currentText)
					if (tools.length > 0) {
						lastProcessedHash = signature
						runToolsSequentially(tools)
					}
				}
			}
		} else {
			unchangedTicks = 0
			previousLength = currentLength
		}
	}, 500)

	function sendImagePayload(base64) {
		chrome.storage.local.get(['authToken'], res => {
			const token = (res && res.authToken ? res.authToken : '').trim()
			if (!token) return

			const headers = {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${token}`,
			}

			fetch('http://127.0.0.1:5505/api/save-image', {
				method: 'POST',
				headers: headers,
				body: JSON.stringify({ base64: base64 }),
			})
				.then(res => res.json())
				.then(data => {
					if (data.status === 'ok') {
						injectAndSubmit(
							`[System - Image Saved]: An image or graphic you created was automatically downloaded and saved to: ${data.path}\nYou can reference this relative path in your code if needed.`,
						)
					}
				})
				.catch(() => {})
		})
	}

	setInterval(() => {
		try {
			const images = document.querySelectorAll('img')
			for (const img of images) {
				if (img.dataset.agentProcessedSrc === img.src) continue
				img.dataset.agentProcessedSrc = img.src

				if (
					img.src.includes('.svg') ||
					img.src.includes('avatar') ||
					img.src.includes('favicon')
				)
					continue

				const processImg = () => {
					if (
						!img.src.startsWith('blob:') &&
						(img.width < 100 || img.height < 100)
					)
						return

					try {
						const canvas = document.createElement('canvas')
						canvas.width = img.naturalWidth || img.width
						canvas.height = img.naturalHeight || img.height
						const ctx = canvas.getContext('2d')
						ctx.drawImage(img, 0, 0)
						const base64 = canvas.toDataURL('image/png')
						if (base64 !== 'data:,') {
							sendImagePayload(base64)
						}
					} catch (e) {
						fetch(img.src)
							.then(r => r.blob())
							.then(b => {
								const reader = new FileReader()
								reader.onloadend = () => sendImagePayload(reader.result)
								reader.readAsDataURL(b)
							})
							.catch(() => {})
					}
				}

				if (img.src.startsWith('blob:') || img.complete) {
					processImg()
				} else {
					img.addEventListener('load', processImg, { once: true })
				}
			}

			const svgs = document.querySelectorAll(
				'svg:not([data-agent-processed="true"])',
			)
			for (const svg of svgs) {
				if (svg.clientWidth < 100 || svg.clientHeight < 100) {
					svg.dataset.agentProcessed = 'true'
					continue
				}
				if (
					svg.classList.contains('icon') ||
					svg.getAttribute('aria-hidden') === 'true'
				) {
					svg.dataset.agentProcessed = 'true'
					continue
				}

				svg.dataset.agentProcessed = 'true'
				const svgData = new XMLSerializer().serializeToString(svg)
				const base64 =
					'data:image/svg+xml;base64,' +
					btoa(unescape(encodeURIComponent(svgData)))
				sendImagePayload(base64)
			}
		} catch (err) {}
	}, 2000)

	chrome.runtime.onMessage.addListener(msg => {
		if (msg.action === 'INJECT_SYSTEM_PROMPT') {
			injectAndSubmit(msg.prompt)
		}
	})
})()
