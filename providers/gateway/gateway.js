'use strict';

/**
 * WebAgentMCP - Unified Web Model Gateway
 * Exposes an OpenAI-compatible API on 127.0.0.1:5506 routing to DeepSeek and Qwen web providers.
 * Includes Anti-Ban throttling, tool-call formatting, and reasoning extraction.
 */

const http = require('http');
const { DeepSeekWebClient } = require('../deepseek/client');
const { QwenWebClient } = require('../qwen/client');
const { getAuthStatus: getDeepSeekAuthStatus } = require('../deepseek/auth');

const PORT = process.env.GATEWAY_PORT || 5506;
const HOST = process.env.GATEWAY_HOST || '127.0.0.1';

const deepSeekClient = new DeepSeekWebClient();
const qwenClient = new QwenWebClient();

const SUPPORTED_MODELS = [
    { id: 'deepseek-reasoner', name: 'DeepSeek R1 (Reasoner)', provider: 'deepseek' },
    { id: 'deepseek-chat', name: 'DeepSeek V3 (Chat)', provider: 'deepseek' },
    { id: 'qwen3.7-max', name: 'Qwen 3.7 Max', provider: 'qwen' },
    { id: 'qwen3.7-plus', name: 'Qwen 3.7 Plus', provider: 'qwen' },
    { id: 'qwen-turbo', name: 'Qwen Turbo', provider: 'qwen' },
];

function sendJson(res, statusCode, data) {
    const json = JSON.stringify(data);
    res.writeHead(statusCode, {
        'Content-Type': 'application/json',
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type, Authorization, x-api-key',
    });
    res.end(json);
}

function parseBody(req) {
    return new Promise((resolve, reject) => {
        let body = '';
        req.on('data', chunk => { body += chunk; });
        req.on('end', () => {
            if (!body) return resolve({});
            try {
                resolve(JSON.parse(body));
            } catch (e) {
                reject(new Error(`Malformed JSON body: ${e.message}`));
            }
        });
        req.on('error', reject);
    });
}

const server = http.createServer(async (req, res) => {
    // CORS preflight
    if (req.method === 'OPTIONS') {
        res.writeHead(204, {
            'Access-Control-Allow-Origin': '*',
            'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
            'Access-Control-Allow-Headers': 'Content-Type, Authorization, x-api-key',
        });
        return res.end();
    }

    const url = new URL(req.url, `http://${req.headers.host}`);

    // GET /api/status
    if (req.method === 'GET' && url.pathname === '/api/status') {
        const dsAuth = getDeepSeekAuthStatus();
        return sendJson(res, 200, {
            status: 'ok',
            providers: {
                deepseek: {
                    authorized: dsAuth.authorized,
                    reason: dsAuth.reason || 'ready',
                },
                qwen: {
                    authorized: qwenClient.isAuthorized(),
                    reason: qwenClient.isAuthorized() ? 'ready' : 'Token missing in qwen-auth.json',
                },
            },
            supported_models: SUPPORTED_MODELS,
        });
    }

    // GET /v1/models
    if (req.method === 'GET' && (url.pathname === '/v1/models' || url.pathname === '/models')) {
        return sendJson(res, 200, {
            object: 'list',
            data: SUPPORTED_MODELS.map(m => ({
                id: m.id,
                object: 'model',
                created: 1700000000,
                owned_by: m.provider,
                permission: [],
                root: m.id,
                parent: null,
            })),
        });
    }

    // POST /v1/chat/completions
    if (req.method === 'POST' && (url.pathname === '/v1/chat/completions' || url.pathname === '/chat/completions')) {
        try {
            const body = await parseBody(req);
            const model = (body.model || 'deepseek-reasoner').toLowerCase();
            const messages = body.messages || [];
            const stream = Boolean(body.stream);

            // Construct prompt from conversation messages
            let userPrompt = '';
            for (const msg of messages) {
                if (msg.role === 'user') {
                    userPrompt += `${msg.content}\n`;
                } else if (msg.role === 'assistant') {
                    userPrompt += `[Previous Assistant Response]: ${msg.content}\n`;
                } else if (msg.role === 'system') {
                    userPrompt = `[Instructions]: ${msg.content}\n\n${userPrompt}`;
                }
            }

            // Route to provider
            const isQwen = model.startsWith('qwen');
            const targetClient = isQwen ? qwenClient : deepSeekClient;

            if (stream) {
                res.writeHead(200, {
                    'Content-Type': 'text/event-stream',
                    'Cache-Control': 'no-cache',
                    'Connection': 'keep-alive',
                    'Access-Control-Allow-Origin': '*',
                });

                await targetClient.chat({
                    prompt: userPrompt.trim(),
                    model: model,
                    sessionKey: body.user || 'default',
                    onChunk: (chunk) => {
                        const ssePayload = {
                            id: `chatcmpl-${Date.now()}`,
                            object: 'chat.completion.chunk',
                            created: Math.floor(Date.now() / 1000),
                            model: model,
                            choices: [
                                {
                                    index: 0,
                                    delta: chunk.type === 'reasoning'
                                        ? { reasoning_content: chunk.text }
                                        : { content: chunk.text },
                                    finish_reason: null,
                                }
                            ]
                        };
                        res.write(`data: ${JSON.stringify(ssePayload)}\n\n`);
                    }
                });

                res.write(`data: ${JSON.stringify({
                    id: `chatcmpl-${Date.now()}`,
                    object: 'chat.completion.chunk',
                    created: Math.floor(Date.now() / 1000),
                    model: model,
                    choices: [{ index: 0, delta: {}, finish_reason: 'stop' }]
                })}\n\n`);
                res.write('data: [DONE]\n\n');
                return res.end();
            } else {
                const result = await targetClient.chat({
                    prompt: userPrompt.trim(),
                    model: model,
                    sessionKey: body.user || 'default',
                });

                return sendJson(res, 200, {
                    id: `chatcmpl-${Date.now()}`,
                    object: 'chat.completion',
                    created: Math.floor(Date.now() / 1000),
                    model: model,
                    choices: [
                        {
                            index: 0,
                            message: {
                                role: 'assistant',
                                content: result.content,
                                reasoning_content: result.reasoning || undefined,
                            },
                            finish_reason: 'stop',
                        }
                    ],
                    usage: {
                        prompt_tokens: Math.round(userPrompt.length / 4),
                        completion_tokens: Math.round(result.content.length / 4),
                        total_tokens: Math.round((userPrompt.length + result.content.length) / 4),
                    }
                });
            }
        } catch (err) {
            console.error('[Gateway Error]:', err);
            return sendJson(res, 500, {
                error: {
                    message: err.message || 'Internal provider error',
                    type: 'provider_error',
                }
            });
        }
    }

    sendJson(res, 404, { error: 'Not found' });
});

server.listen(PORT, HOST, () => {
    console.log(`[WebAgentMCP Gateway] Listening on http://${HOST}:${PORT}`);
});

module.exports = { server, deepSeekClient, qwenClient };
