'use strict';

/**
 * WebAgentMCP - DeepSeek Web API Client
 * Direct protocol-level client for DeepSeek Web with PoW solver and Anti-Ban guards.
 */

const fs = require('fs');
const path = require('path');
const { solvePOW } = require('./pow');

const DEFAULT_WASM_URL = 'https://fe-static.deepseek.com/chat/static/sha3_wasm_bg.7b9ca65ddd.wasm';
const BASE_URL = 'https://chat.deepseek.com';

class DeepSeekWebClient {
    constructor(options = {}) {
        this.authFile = options.authFile || path.join(__dirname, 'deepseek-auth.json');
        this.auth = this.loadAuth();
        this.activeSessions = new Map(); // sessionKey -> { sessionId, messageCount, lastActive }
        this.minJitterMs = options.minJitterMs || 600;
        this.maxJitterMs = options.maxJitterMs || 1800;
    }

    loadAuth() {
        if (fs.existsSync(this.authFile)) {
            try {
                return JSON.parse(fs.readFileSync(this.authFile, 'utf8'));
            } catch (err) {
                console.warn(`[DeepSeek] Failed to parse auth file: ${err.message}`);
            }
        }
        return {
            token: process.env.DEEPSEEK_TOKEN || '',
            cookie: process.env.DEEPSEEK_COOKIE || '',
            wasmUrl: process.env.DEEPSEEK_WASM_URL || DEFAULT_WASM_URL,
            hif_dliq: process.env.DEEPSEEK_HIF_DLIQ || '',
            hif_leim: process.env.DEEPSEEK_HIF_LEIM || '',
        };
    }

    saveAuth(authData) {
        this.auth = { ...this.auth, ...authData };
        fs.writeFileSync(this.authFile, JSON.stringify(this.auth, null, 2), 'utf8');
    }

    isAuthorized() {
        return Boolean(this.auth.token || this.auth.cookie);
    }

    getHeaders() {
        return {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36',
            'Accept': '*/*',
            'Accept-Language': 'ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7',
            'x-client-platform': 'web',
            'x-client-version': '2.0.0',
            'x-app-version': '2.0.0',
            'x-client-locale': 'ru',
            'x-client-timezone-offset': '14400',
            'Authorization': `Bearer ${this.auth.token}`,
            'x-hif-dliq': this.auth.hif_dliq || '',
            'x-hif-leim': this.auth.hif_leim || '',
            'Origin': BASE_URL,
            'Referer': `${BASE_URL}/`,
            'Cookie': this.auth.cookie || '',
            'Content-Type': 'application/json',
        };
    }

    async applyAntiBanJitter() {
        const jitter = Math.floor(Math.random() * (this.maxJitterMs - this.minJitterMs + 1)) + this.minJitterMs;
        await new Promise(resolve => setTimeout(resolve, jitter));
    }

    async getOrCreateSession(sessionKey = 'default') {
        const existing = this.activeSessions.get(sessionKey);
        const now = Date.now();
        // Reuse session if exists, message count < 60, and was active within last 90 minutes
        if (existing && existing.messageCount < 60 && (now - existing.lastActive < 90 * 60 * 1000)) {
            existing.lastActive = now;
            existing.messageCount++;
            return existing.sessionId;
        }

        // Create new session
        const resp = await fetch(`${BASE_URL}/api/v0/chat_session/create`, {
            method: 'POST',
            headers: this.getHeaders(),
            body: JSON.stringify({ agent: 'chat' }),
        });

        if (!resp.ok) {
            const errText = await resp.text();
            throw new Error(`[DeepSeek] Session creation failed (HTTP ${resp.status}): ${errText}`);
        }

        const data = await resp.json();
        const sessionId = data?.data?.biz_data?.id;
        if (!sessionId) {
            throw new Error('[DeepSeek] Invalid session response: missing biz_data.id');
        }

        this.activeSessions.set(sessionKey, {
            sessionId,
            messageCount: 1,
            lastActive: now,
        });

        return sessionId;
    }

    async solveChallenge(targetPath = '/api/v0/chat/completion') {
        const resp = await fetch(`${BASE_URL}/api/v0/chat/create_pow_challenge`, {
            method: 'POST',
            headers: this.getHeaders(),
            body: JSON.stringify({ target_path: targetPath }),
        });

        if (!resp.ok) {
            throw new Error(`[DeepSeek] PoW challenge request failed (HTTP ${resp.status})`);
        }

        const json = await resp.json();
        const challenge = json?.data?.biz_data?.challenge;
        if (!challenge) {
            throw new Error('[DeepSeek] Challenge missing from response data');
        }

        const wasmUrl = this.auth.wasmUrl || DEFAULT_WASM_URL;
        const answer = await solvePOW(challenge, wasmUrl);

        return Buffer.from(JSON.stringify({
            algorithm: challenge.algorithm,
            challenge: challenge.challenge,
            salt: challenge.salt,
            answer: answer,
            signature: challenge.signature,
            target_path: targetPath,
        })).toString('base64');
    }

    async chat({ prompt, model = 'deepseek-reasoner', search = false, sessionKey = 'default', onChunk = null }) {
        if (!this.isAuthorized()) {
            throw new Error('[DeepSeek] Not authorized. Token or cookie missing.');
        }

        await this.applyAntiBanJitter();

        const sessionId = await this.getOrCreateSession(sessionKey);
        const powResponse = await this.solveChallenge('/api/v0/chat/completion');

        const thinkingEnabled = model.includes('reasoner') || model.includes('r1');

        const body = {
            chat_session_id: sessionId,
            parent_message_id: null,
            prompt: prompt,
            ref_file_ids: [],
            thinking_enabled: thinkingEnabled,
            search_enabled: Boolean(search),
        };

        const headers = {
            ...this.getHeaders(),
            'x-ds-pow-response': powResponse,
        };

        const resp = await fetch(`${BASE_URL}/api/v0/chat/completion`, {
            method: 'POST',
            headers: headers,
            body: JSON.stringify(body),
        });

        if (!resp.ok) {
            const errText = await resp.text();
            throw new Error(`[DeepSeek] Completion request failed (HTTP ${resp.status}): ${errText}`);
        }

        let fullContent = '';
        let reasoningContent = '';
        const reader = resp.body.getReader();
        const decoder = new TextDecoder('utf-8');
        let buffer = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n');
            buffer = lines.pop(); // keep trailing incomplete chunk

            for (const line of lines) {
                const trimmed = line.trim();
                if (!trimmed.startsWith('data:')) continue;
                const jsonStr = trimmed.slice(5).trim();
                if (!jsonStr || jsonStr === '[DONE]') continue;

                try {
                    const parsed = JSON.parse(jsonStr);
                    const fragment = parsed?.choices?.[0]?.delta || parsed?.data?.response;
                    
                    // Handle thinking / reasoning chunk
                    if (fragment?.type === 'THINK' || fragment?.reasoning_content) {
                        const chunk = fragment.content || fragment.reasoning_content || '';
                        reasoningContent += chunk;
                        if (onChunk) onChunk({ type: 'reasoning', text: chunk });
                    }
                    // Handle output text chunk
                    else if (fragment?.type === 'RESPONSE' || fragment?.content || typeof fragment === 'string') {
                        const chunk = fragment.content || fragment;
                        if (typeof chunk === 'string') {
                            fullContent += chunk;
                            if (onChunk) onChunk({ type: 'content', text: chunk });
                        }
                    }
                } catch {
                    // Ignore non-json lines
                }
            }
        }

        return {
            content: fullContent,
            reasoning: reasoningContent,
            sessionId,
            model,
        };
    }
}

module.exports = { DeepSeekWebClient };
