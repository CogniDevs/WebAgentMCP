'use strict';

/**
 * WebAgentMCP - DeepSeek Auth Manager
 * Manages token acquisition, validation, and auto-import from local references or Chrome CDP.
 */

const fs = require('fs');
const path = require('path');

const LOCAL_AUTH_PATH = path.join(__dirname, 'deepseek-auth.json');
const REF_AUTH_PATHS = [
    path.join(__dirname, '..', '..', '..', 'references', 'FreeDeepseekAPI', 'deepseek-auth.json'),
    path.join(__dirname, '..', '..', '..', 'references', 'FreeDeepseekAPI', 'auth.json'),
];

function trySyncFromReferences() {
    for (const refPath of REF_AUTH_PATHS) {
        if (fs.existsSync(refPath)) {
            try {
                const data = JSON.parse(fs.readFileSync(refPath, 'utf8'));
                if (data.token) {
                    fs.writeFileSync(LOCAL_AUTH_PATH, JSON.stringify(data, null, 2), 'utf8');
                    console.log(`[DeepSeek Auth] Auto-synced auth from ${refPath}`);
                    return true;
                }
            } catch (err) {
                console.warn(`[DeepSeek Auth] Could not read reference ${refPath}: ${err.message}`);
            }
        }
    }
    return false;
}

function getAuthStatus() {
    trySyncFromReferences();
    if (!fs.existsSync(LOCAL_AUTH_PATH)) {
        return { authorized: false, reason: 'Auth file not found' };
    }
    try {
        const auth = JSON.parse(fs.readFileSync(LOCAL_AUTH_PATH, 'utf8'));
        if (!auth.token) {
            return { authorized: false, reason: 'Token missing in auth file' };
        }
        return {
            authorized: true,
            hasCookie: Boolean(auth.cookie),
            hasHifHeaders: Boolean(auth.hif_dliq && auth.hif_leim),
            wasmConfigured: Boolean(auth.wasmUrl),
        };
    } catch (e) {
        return { authorized: false, reason: e.message };
    }
}

if (require.main === module) {
    const status = getAuthStatus();
    console.log('[DeepSeek Auth Status]:', status);
}

module.exports = { getAuthStatus, trySyncFromReferences, LOCAL_AUTH_PATH };
