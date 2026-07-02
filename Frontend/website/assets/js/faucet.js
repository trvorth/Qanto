/**
 * QANTO Testnet Faucet — Backend Logic
 * 
 * Provides requestTestnetFunds(address) for dispensing mock Testnet QNTO.
 * Rate-limited to 1 request per 24 hours per address (client-side mock).
 * Drop amount: 10 QNTO per request.
 * 
 * Network truth: 21B supply · 1.0s blocks · 2.5 QNTO block reward
 */

const FAUCET_CONFIG = Object.freeze({
    DROP_AMOUNT: 10,
    COOLDOWN_MS: 24 * 60 * 60 * 1000, // 24 hours
    STORAGE_KEY: 'qanto_faucet_claims',
    TREASURY_ADDRESS: '0x0000000000000000000000000000000000000001',
    CHAIN_ID: 'qanto-testnet-1',
    BLOCK_TIME: '1.0s',
    BLOCK_REWARD: 2.5,
    TOTAL_SUPPLY: '21,000,000,000 QNTO',
});

/**
 * Validate a 0x-prefixed Ethereum-style wallet address.
 * @param {string} address
 * @returns {{ valid: boolean, reason?: string }}
 */
function validateAddress(address) {
    if (typeof address !== 'string') {
        return { valid: false, reason: 'Address must be a string.' };
    }

    const trimmed = address.trim();

    if (trimmed.length === 0) {
        return { valid: false, reason: 'Address cannot be empty.' };
    }

    if (!trimmed.startsWith('0x')) {
        return { valid: false, reason: 'Address must start with 0x prefix.' };
    }

    if (trimmed.length !== 42) {
        return { valid: false, reason: 'Address must be exactly 42 characters (0x + 40 hex chars).' };
    }

    if (!/^0x[0-9a-fA-F]{40}$/.test(trimmed)) {
        return { valid: false, reason: 'Address contains invalid characters. Only hex digits (0-9, a-f) allowed after 0x.' };
    }

    return { valid: true };
}

/**
 * Retrieve claim history from localStorage.
 * @returns {Object} Map of address → last claim timestamp (ms)
 */
function getClaimHistory() {
    try {
        const raw = localStorage.getItem(FAUCET_CONFIG.STORAGE_KEY);
        return raw ? JSON.parse(raw) : {};
    } catch {
        return {};
    }
}

/**
 * Persist claim history to localStorage.
 * @param {Object} history
 */
function saveClaimHistory(history) {
    try {
        localStorage.setItem(FAUCET_CONFIG.STORAGE_KEY, JSON.stringify(history));
    } catch {
        // localStorage unavailable — silently fail
    }
}

/**
 * Check if an address is rate-limited.
 * @param {string} address
 * @returns {{ limited: boolean, retryAfterMs?: number, lastClaim?: string }}
 */
function checkRateLimit(address) {
    const history = getClaimHistory();
    const normalised = address.trim().toLowerCase();
    const lastClaim = history[normalised];

    if (!lastClaim) {
        return { limited: false };
    }

    const elapsed = Date.now() - lastClaim;

    if (elapsed < FAUCET_CONFIG.COOLDOWN_MS) {
        const retryAfterMs = FAUCET_CONFIG.COOLDOWN_MS - elapsed;
        return {
            limited: true,
            retryAfterMs,
            lastClaim: new Date(lastClaim).toISOString(),
        };
    }

    return { limited: false };
}

/**
 * Generate a mock transaction hash.
 * @returns {string}
 */
function generateSyntheticTxHash() {
    const hex = Array.from({ length: 64 }, () =>
        Math.floor(Math.random() * 16).toString(16)
    ).join('');
    return `0x${hex}`;
}

/**
 * Request Testnet QNTO from the faucet.
 * 
 * @param {string} address — A 0x-prefixed wallet address
 * @returns {Promise<Object>} JSON success or error response
 * 
 * @example Success:
 * {
 *   "success": true,
 *   "data": {
 *     "txHash": "0xabc123...",
 *     "from": "0x0000000000000000000000000000000000000001",
 *     "to": "0x742d35Cc...",
 *     "amount": "10 QNTO",
 *     "network": "qanto-testnet-1",
 *     "blockTime": "1.0s",
 *     "timestamp": "2026-04-19T07:30:00.000Z"
 *   }
 * }
 * 
 * @example Error:
 * {
 *   "success": false,
 *   "error": {
 *     "code": "RATE_LIMITED",
 *     "message": "Rate limit exceeded. Try again in 23h 45m.",
 *     "retryAfterMs": 85500000
 *   }
 * }
 */
async function requestTestnetFunds(address) {
    // 1. Validate address
    const validation = validateAddress(address);
    if (!validation.valid) {
        return {
            success: false,
            error: {
                code: 'INVALID_ADDRESS',
                message: validation.reason,
            },
        };
    }

    const normalised = address.trim().toLowerCase();

    // 2. Check rate limit
    const rateCheck = checkRateLimit(address);
    if (rateCheck.limited) {
        const hours = Math.floor(rateCheck.retryAfterMs / (1000 * 60 * 60));
        const minutes = Math.floor((rateCheck.retryAfterMs % (1000 * 60 * 60)) / (1000 * 60));
        return {
            success: false,
            error: {
                code: 'RATE_LIMITED',
                message: `Rate limit exceeded. 1 request per 24 hours. Try again in ${hours}h ${minutes}m.`,
                retryAfterMs: rateCheck.retryAfterMs,
                lastClaim: rateCheck.lastClaim,
            },
        };
    }

    // 3. Simulate network delay (300-800ms)
    await new Promise(resolve => setTimeout(resolve, 300 + Math.random() * 500));

    // 4. Record the claim
    const history = getClaimHistory();
    history[normalised] = Date.now();
    saveClaimHistory(history);

    // 5. Build success response
    const txHash = generateSyntheticTxHash();
    const timestamp = new Date().toISOString();

    return {
        success: true,
        data: {
            txHash,
            from: FAUCET_CONFIG.TREASURY_ADDRESS,
            to: address.trim(),
            amount: `${FAUCET_CONFIG.DROP_AMOUNT} QNTO`,
            amountRaw: FAUCET_CONFIG.DROP_AMOUNT,
            network: FAUCET_CONFIG.CHAIN_ID,
            blockTime: FAUCET_CONFIG.BLOCK_TIME,
            timestamp,
            message: `Successfully sent ${FAUCET_CONFIG.DROP_AMOUNT} Testnet QNTO to ${address.trim()}. The transaction will be confirmed within ~${FAUCET_CONFIG.BLOCK_TIME}.`,
        },
    };
}

// Expose globally for the faucet UI
window.requestTestnetFunds = requestTestnetFunds;
window.FAUCET_CONFIG = FAUCET_CONFIG;
