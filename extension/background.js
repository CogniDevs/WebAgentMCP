chrome.sidePanel
	.setPanelBehavior({ openPanelOnActionClick: true })
	.catch(error => console.error(error))

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
	if (message.target === 'content' && sender.tab === undefined) {
		chrome.tabs.query({ active: true, currentWindow: true }, tabs => {
			if (tabs[0] && tabs[0].id) {
				chrome.tabs.sendMessage(tabs[0].id, message).catch(() => {})
			}
		})
	}
	return true
})
