'use strict';

/**
 * WebAgentMCP - Qwen Web API Client
 * Connects to Qwen Chat (chat.qwen.ai / chat.qwenlm.ai) with anti-bot evasion and session reuse.
 */

const fs = require('fs');
const path = require('path');

const DEFAULT_BASE_URL = 'https://chat.qwen.ai';
const AUTH_FILE = path.join(__dirname, 'qwen-auth.json');

class QwenWebClient {
    constructor(options = {}) {
        this.authFile = options.authFile || AUTH_FILE;
        this.auth = this.loadAuth();
        this.activeSessions = new Map();
        this.minJitterMs = options.minJitterMs || 700;
        this.maxJitterMs = options.maxJitterMs || 2200;
    }

    loadAuth() {
        if (fs.existsSync(this.authFile)) {
            try {
                return JSON.parse(fs.readFileSync(this.authFile, 'utf8'));
            } catch (err) {
                console.warn(`[Qwen] Failed to parse auth file: ${err.message}`);
            }
        }
        return {
            token: process.env.QWEN_TOKEN || '',
            cookie: process.env.QWEN_COOKIE || '',
            baseUrl: process.env.QWEN_BASE_URL || DEFAULT_BASE_URL,
        };
    }

    saveAuth(authData) {
        this.auth = { ...this.auth, ...authData };
        fs.writeFileSync(this.authFile, JSON.stringify(this.auth, null, 2), 'utf8');
    }

    isAuthorized() {
        return Boolean(this.auth.token);
    }

    getHeaders() {
        return {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36',
            'Accept': 'application/json, text/plain, */*',
            'Accept-Language': 'ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7',
            'Authorization': `Bearer ${this.auth.token}`,
            'Origin': this.auth.baseUrl,
            'Referer': `${this.auth.baseUrl}/`,
            'Cookie': this.auth.cookie || '',
            'Content-Type': 'application/json',
            'Sec-Fetch-Dest': 'empty',
            'Sec-Fetch-Mode': 'cors',
            'Sec-Fetch-Site': 'same-origin',
        };
    }

    async applyAntiBanJitter() {
        const jitter = Math.floor(Math.random() * (this.maxJitterMs - this.minJitterMs + 1)) + this.minJitterMs;
        await new Promise(resolve => setTimeout(resolve, jitter));
    }

    async chat({ prompt, model = 'qwen3.7-max', search = false, sessionKey = 'default', onChunk = null }) {
        if (!this.isAuthorized()) {
            throw new Error('[Qwen] Not authorized. Qwen token is missing in qwen-auth.json or env.');
        }

        await this.applyAntiBanJitter();

        const apiUrl = `${this.auth.baseUrl}/api/v1/chat/completions`;
        const payload = {
            model: model,
            messages: [
                { role: 'user', content: prompt }
            ],
            stream: Boolean(onChunk),
            incremental: true,
            chat_type: 't2t',
            search: Boolean(search),
        };

        const resp = await fetch(apiUrl, {
            method: 'POST',
            headers: this.getHeaders(),
            body: JSON.stringify(payload),
        });

        if (!resp.ok) {
            const err = await resp.text();
            throw new Error(`[Qwen] Chat completion request failed (HTTP ${resp.status}): ${err}`);
        }

        if (!onChunk) {
            const json = await resp.json();
            const text = json?.choices?.[0]?.message?.content || json?.data?.content || '';
            return {
                content: text,
                model,
            };
        }

        let fullContent = '';
        const reader = resp.body.getReader();
        const decoder = new TextDecoder('utf-8');
        let buffer = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n');
            buffer = lines.pop();

            for (const line of lines) {
                const trimmed = line.trim();
                if (!trimmed.startsWith('data:')) continue;
                const chunkStr = trimmed.slice(5).trim();
                if (!chunkStr || chunkStr === '[DONE]') continue;

                try {
                    const parsed = JSON.parse(chunkStr);
                    const delta = parsed?.choices?.[0]?.delta?.content || '';
                    if (delta) {
                        fullContent += delta;
                        onChunk({ type: 'content', text: delta });
                    }
                } catch {
                    // Ignore malformed sse frames
                }
            }
        }

        return {
            content: fullContent,
            model,
        };
    }
}

module.exports = { QwenWebClient };
