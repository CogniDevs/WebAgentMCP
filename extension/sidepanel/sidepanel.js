document.addEventListener('DOMContentLoaded', () => {
	const statusDot = document.getElementById('server-status')
	const platformBadge = document.getElementById('platform-badge')
	const activityBox = document.getElementById('activity-box')
	const initBtn = document.getElementById('init-btn')
	const mcpCountTag = document.getElementById('mcp-count-tag')
	const mcpList = document.getElementById('mcp-list')
	const workspacePath = document.getElementById('workspace-path')
	const pickWorkspaceBtn = document.getElementById('pick-workspace-btn')
	const mcpModal = document.getElementById('mcp-modal')
	const showAddMcpBtn = document.getElementById('show-add-mcp-btn')
	const closeMcpModalBtn = document.getElementById('close-mcp-modal')
	const saveMcpBtn = document.getElementById('save-mcp-btn')
	const mcpNameInput = document.getElementById('mcp-name')
	const mcpCommandInput = document.getElementById('mcp-command')
	const mcpArgsInput = document.getElementById('mcp-args')
	const authTokenInput = document.getElementById('auth-token-input')
	const saveTokenBtn = document.getElementById('save-token-btn')

	let currentAuthToken = ''
	let currentLang =
		navigator.language && navigator.language.startsWith('ru') ? 'ru' : 'en'
	let translations = {}

	function tr(key, vars = {}) {
		let text = translations[key] || key
		for (const [vKey, vVal] of Object.entries(vars)) {
			text = text.replace(`{${vKey}}`, vVal)
		}
		return text
	}

	function applyTranslations() {
		document.querySelectorAll('[data-i18n]').forEach(el => {
			const key = el.getAttribute('data-i18n')
			el.textContent = tr(key)
		})
		document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
			const key = el.getAttribute('data-i18n-placeholder')
			el.setAttribute('placeholder', tr(key))
		})
	}

	async function setLanguage(lang) {
		try {
			const url = chrome.runtime.getURL(`sidepanel/locales/${lang}.json`)
			const res = await fetch(url)
			if (res.ok) {
				translations = await res.json()
				currentLang = lang
				applyTranslations()
				updateActiveTabPlatform()
			}
		} catch (_) {
			if (lang !== 'en') {
				setLanguage('en')
			}
		}
	}

	function getAuthHeaders() {
		const headers = { 'Content-Type': 'application/json' }
		if (currentAuthToken) {
			headers['Authorization'] = `Bearer ${currentAuthToken}`
		}
		return headers
	}

	if (saveTokenBtn && authTokenInput) {
		saveTokenBtn.addEventListener('click', () => {
			const val = authTokenInput.value.trim()
			currentAuthToken = val
			chrome.storage.local.set({ authToken: val }, () => {
				saveTokenBtn.textContent = tr('saved_btn')
				setTimeout(() => {
					saveTokenBtn.textContent = tr('save_btn')
				}, 1500)
				checkServer()
			})
		})
	}

	function updateActiveTabPlatform() {
		chrome.tabs.query({ active: true, currentWindow: true }, tabs => {
			if (!tabs[0] || !tabs[0].url) {
				platformBadge.textContent = tr('unknown')
				return
			}
			try {
				const url = new URL(tabs[0].url)
				platformBadge.textContent = url.hostname
			} catch {
				platformBadge.textContent = tr('unknown')
			}
		})
	}

	function renderMcpServers(servers) {
		mcpCountTag.textContent = tr('mcp_count', { count: servers.length })
		const existingDynamicTags = mcpList.querySelectorAll('.mcp-tag-dynamic')
		existingDynamicTags.forEach(el => el.remove())

		servers.forEach(srv => {
			const tag = document.createElement('span')
			tag.className = `mcp-tag mcp-tag-dynamic ${srv.enabled ? 'active' : 'disabled'}`

			const nameText = document.createElement('span')
			nameText.className = 'mcp-name-btn'
			nameText.textContent = srv.name
			nameText.addEventListener('click', () => {
				fetch('http://127.0.0.1:5505/api/mcp/toggle', {
					method: 'POST',
					headers: getAuthHeaders(),
					body: JSON.stringify({ name: srv.name, enabled: !srv.enabled }),
				}).then(() => checkServer())
			})
			tag.appendChild(nameText)

			const removeBtn = document.createElement('button')
			removeBtn.className = 'mcp-remove-btn'
			removeBtn.innerHTML = '&times;'
			removeBtn.addEventListener('click', e => {
				e.stopPropagation()
				fetch('http://127.0.0.1:5505/api/mcp/remove', {
					method: 'POST',
					headers: getAuthHeaders(),
					body: JSON.stringify({ name: srv.name }),
				}).then(() => checkServer())
			})
			tag.appendChild(removeBtn)

			mcpList.appendChild(tag)
		})
	}

	function checkServer() {
		fetch('http://127.0.0.1:5505/api/status', {
			headers: getAuthHeaders(),
		})
			.then(res => {
				if (!res.ok) throw new Error()
				return res.json()
			})
			.then(data => {
				if (data.auth_valid === false) {
					statusDot.classList.remove('online')
					statusDot.style.backgroundColor = 'var(--yellow, #e3b341)'
					statusDot.style.boxShadow = '0 0 6px var(--yellow, #e3b341)'
				} else {
					statusDot.style.backgroundColor = ''
					statusDot.style.boxShadow = ''
					statusDot.classList.add('online')
				}

				if (data.language && data.language !== currentLang) {
					setLanguage(data.language)
				}
				if (data.workspace) {
					workspacePath.textContent = data.workspace
				}
				if (data.mcp_servers && Array.isArray(data.mcp_servers)) {
					renderMcpServers(data.mcp_servers)
				}
			})
			.catch(() => {
				statusDot.classList.remove('online')
				statusDot.style.backgroundColor = ''
				statusDot.style.boxShadow = ''
				mcpCountTag.textContent = tr('daemon_offline')
				workspacePath.textContent = tr('conn_error')
			})
	}

	pickWorkspaceBtn.addEventListener('click', () => {
		fetch('http://127.0.0.1:5505/api/workspace/pick', {
			method: 'POST',
			headers: getAuthHeaders(),
		})
			.then(res => res.json())
			.then(data => {
				if (data.success) {
					checkServer()
				}
			})
			.catch(() => {})
	})

	showAddMcpBtn.addEventListener('click', () => {
		mcpModal.classList.remove('hidden')
	})

	closeMcpModalBtn.addEventListener('click', () => {
		mcpModal.classList.add('hidden')
	})

	saveMcpBtn.addEventListener('click', () => {
		const name = mcpNameInput.value.trim()
		const command = mcpCommandInput.value.trim()
		const argsRaw = mcpArgsInput.value.trim()

		if (!name || !command) return

		const args = argsRaw.length > 0 ? argsRaw.split(/\s+/) : []

		fetch('http://127.0.0.1:5505/api/mcp/add', {
			method: 'POST',
			headers: getAuthHeaders(),
			body: JSON.stringify({ name, command, args, env: {} }),
		})
			.then(res => res.json())
			.then(data => {
				if (data.success) {
					mcpNameInput.value = ''
					mcpCommandInput.value = ''
					mcpArgsInput.value = ''
					mcpModal.classList.add('hidden')
					checkServer()
				}
			})
			.catch(() => {})
	})

	chrome.storage.local.get(['authToken'], res => {
		if (res.authToken) {
			currentAuthToken = res.authToken
			if (authTokenInput) {
				authTokenInput.value = res.authToken
			}
		}

		setLanguage(currentLang).then(() => {
			updateActiveTabPlatform()
			checkServer()
		})
	})

	setInterval(checkServer, 2000)

	chrome.tabs.onActivated.addListener(updateActiveTabPlatform)
	chrome.tabs.onUpdated.addListener(updateActiveTabPlatform)

	chrome.runtime.onMessage.addListener(msg => {
		if (msg.target !== 'sidepanel') return

		const placeholder = activityBox.querySelector('.placeholder')
		if (placeholder) {
			placeholder.remove()
		}

		if (msg.type === 'TOOL_CALL_DETECTED') {
			const entry = document.createElement('div')
			entry.className = 'log-entry'
			const snippet =
				msg.payload.length > 140
					? `${msg.payload.substring(0, 140)}...`
					: msg.payload
			entry.innerHTML = `<span class="tool-name">⚡ ${msg.tool}</span><br><span class="snippet">${snippet}</span>`
			activityBox.appendChild(entry)
			activityBox.scrollTop = activityBox.scrollHeight
		}

		if (msg.type === 'TOOL_RESULT') {
			const entry = document.createElement('div')
			entry.className = 'log-entry'
			entry.innerHTML = `<span class="success">${tr('tool_completed', { tool: msg.tool })}</span>`
			activityBox.appendChild(entry)
			activityBox.scrollTop = activityBox.scrollHeight
		}

		if (msg.type === 'TOOL_ERROR') {
			const entry = document.createElement('div')
			entry.className = 'log-entry'
			entry.innerHTML = `<span class="error">${tr('tool_error', { error: msg.error })}</span>`
			activityBox.appendChild(entry)
			activityBox.scrollTop = activityBox.scrollHeight
		}
	})

	initBtn.addEventListener('click', () => {
		chrome.tabs.query({ active: true, currentWindow: true }, tabs => {
			if (!tabs[0] || !tabs[0].id) return

			fetch('http://127.0.0.1:5505/api/system-prompt', {
				headers: getAuthHeaders(),
			})
				.then(res => {
					if (!res.ok) throw new Error()
					return res.json()
				})
				.then(data => {
					chrome.tabs.sendMessage(tabs[0].id, {
						action: 'INJECT_SYSTEM_PROMPT',
						prompt: data.prompt,
					})
				})
				.catch(() => {})
		})
	})
})
