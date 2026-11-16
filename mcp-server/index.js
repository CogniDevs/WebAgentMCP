#!/usr/bin/env node
'use strict';

/**
 * WebAgentMCP - Agent Delegation MCP Server
 * Standard JSON-RPC stdio MCP server enabling AI coding agents to delegate tasks to Web Chats.
 */

const readline = require('readline');
const { DeepSeekWebClient } = require('../providers/deepseek/client');
const { QwenWebClient } = require('../providers/qwen/client');

const deepSeekClient = new DeepSeekWebClient();
const qwenClient = new QwenWebClient();

const TOOLS = [
    {
        name: 'ask_web_model',
        description: 'Send a prompt or coding question to free web models (DeepSeek-R1 / Qwen 3.7) to save agent tokens. Supports reasoning extraction.',
        inputSchema: {
            type: 'object',
            properties: {
                model: {
                    type: 'string',
                    enum: ['deepseek-reasoner', 'deepseek-chat', 'qwen3.7-max', 'qwen3.7-plus'],
                    description: 'The target model. Default is deepseek-reasoner (R1 with thinking).',
                    default: 'deepseek-reasoner',
                },
                prompt: {
                    type: 'string',
                    description: 'The task, code block, or question to send to the web model.',
                },
                include_reasoning: {
                    type: 'boolean',
                    description: 'Whether to return the thinking/reasoning process (for DeepSeek-R1).',
                    default: true,
                },
            },
            required: ['prompt'],
        },
    },
    {
        name: 'delegate_vibe_coding',
        description: 'Offload heavy file refactoring, code drafting, or multi-step logic implementation to a free web model, returning ready-to-apply code diffs.',
        inputSchema: {
            type: 'object',
            properties: {
                task: {
                    type: 'string',
                    description: 'Description of the coding task or refactoring requirements.',
                },
                code_snippet: {
                    type: 'string',
                    description: 'Existing code context or file contents to refactor.',
                },
                model: {
                    type: 'string',
                    enum: ['deepseek-reasoner', 'qwen3.7-max'],
                    default: 'deepseek-reasoner',
                },
            },
            required: ['task'],
        },
    },
];

async function handleToolCall(name, args) {
    if (name === 'ask_web_model') {
        const model = args.model || 'deepseek-reasoner';
        const isQwen = model.startsWith('qwen');
        const client = isQwen ? qwenClient : deepSeekClient;

        const res = await client.chat({
            prompt: args.prompt,
            model: model,
        });

        let output = res.content;
        if (args.include_reasoning !== false && res.reasoning) {
            output = `[Thinking / Reasoning Process]:\n${res.reasoning}\n\n[Final Output]:\n${res.content}`;
        }

        return {
            content: [{ type: 'text', text: output }],
        };
    }

    if (name === 'delegate_vibe_coding') {
        const model = args.model || 'deepseek-reasoner';
        const isQwen = model.startsWith('qwen');
        const client = isQwen ? qwenClient : deepSeekClient;

        let fullPrompt = `Task: ${args.task}\n\n`;
        if (args.code_snippet) {
            fullPrompt += `Context Code:\n\`\`\`\n${args.code_snippet}\n\`\`\`\n\n`;
        }
        fullPrompt += 'Please provide clean, production-ready code with complete implementations (no placeholders).';

        const res = await client.chat({
            prompt: fullPrompt,
            model: model,
        });

        return {
            content: [{ type: 'text', text: res.content }],
        };
    }

    throw new Error(`Unknown tool: ${name}`);
}

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
});

rl.on('line', async (line) => {
    if (!line.trim()) return;
    try {
        const req = JSON.parse(line);
        const { id, method, params } = req;

        if (method === 'initialize') {
            const resp = {
                jsonrpc: '2.0',
                id,
                result: {
                    protocolVersion: '2024-11-05',
                    capabilities: { tools: {} },
                    serverInfo: { name: 'webagentmcp-agent-bridge', version: '1.0.0' },
                },
            };
            process.stdout.write(JSON.stringify(resp) + '\n');
            return;
        }

        if (method === 'notifications/initialized') {
            return;
        }

        if (method === 'tools/list') {
            const resp = {
                jsonrpc: '2.0',
                id,
                result: { tools: TOOLS },
            };
            process.stdout.write(JSON.stringify(resp) + '\n');
            return;
        }

        if (method === 'tools/call') {
            try {
                const res = await handleToolCall(params.name, params.arguments || {});
                const resp = {
                    jsonrpc: '2.0',
                    id,
                    result: res,
                };
                process.stdout.write(JSON.stringify(resp) + '\n');
            } catch (err) {
                const resp = {
                    jsonrpc: '2.0',
                    id,
                    result: {
                        isError: true,
                        content: [{ type: 'text', text: `Error: ${err.message}` }],
                    },
                };
                process.stdout.write(JSON.stringify(resp) + '\n');
            }
            return;
        }

        // Unknown method fallback
        const resp = {
            jsonrpc: '2.0',
            id,
            error: { code: -32601, message: 'Method not found' },
        };
        process.stdout.write(JSON.stringify(resp) + '\n');
    } catch {
        // Ignore unparseable lines
    }
});
