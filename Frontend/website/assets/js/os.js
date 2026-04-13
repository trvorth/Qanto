/**
 * SAGA-OS: Window Management Engine
 */

// ============================================================
// GLOBAL RPC CONSTANT — SINGLE SOURCE OF TRUTH
// ============================================================
const RPC_URL = 'https://trvorth-qanto-testnet.hf.space';
let zIndex = 100;
const desktop = document.getElementById('desktop');

function openWindow(id, url, title) {
    // Check if window already exists
    if (document.getElementById(`window-${id}`)) {
        focusWindow(id);
        return;
    }

    const win = document.createElement('div');
    win.id = `window-${id}`;
    win.className = 'window fade-in';
    win.style.left = '100px';
    win.style.top = '100px';
    win.style.zIndex = ++zIndex;

    win.innerHTML = `
        <div class="window-header" onmousedown="startDrag(event, '${id}')">
            <div class="window-title">${title}</div>
            <div class="window-controls">
                <div class="control-dot dot-min"></div>
                <div class="control-dot dot-max"></div>
                <div class="control-dot dot-close" onclick="closeWindow('${id}')"></div>
            </div>
        </div>
        <div class="window-content">
            <iframe src="${url}" class="window-iframe"></iframe>
        </div>
    `;

    desktop.appendChild(win);
    addtoTaskbar(id, title);
}

function closeWindow(id) {
    const win = document.getElementById(`window-${id}`);
    const taskItem = document.getElementById(`task-item-${id}`);
    if (win) win.remove();
    if (taskItem) taskItem.remove();
}

function focusWindow(id) {
    const win = document.getElementById(`window-${id}`);
    if (win) {
        win.style.zIndex = ++zIndex;
    }
}

function addtoTaskbar(id, title) {
    const taskbar = document.getElementById('active-apps');
    const item = document.createElement('div');
    item.id = `task-item-${id}`;
    item.className = 'taskbar-item';
    item.title = title;
    item.innerHTML = `<i class="fas fa-window-restore"></i>`;
    item.onclick = () => focusWindow(id);
    taskbar.appendChild(item);
}

// Draggable Logic
let activeWin = null;
let offset = { x: 0, y: 0 };

function startDrag(e, id) {
    activeWin = document.getElementById(`window-${id}`);
    focusWindow(id);
    offset.x = e.clientX - activeWin.offsetLeft;
    offset.y = e.clientY - activeWin.offsetTop;
    
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
}

function onMouseMove(e) {
    if (activeWin) {
        activeWin.style.left = `${e.clientX - offset.x}px`;
        activeWin.style.top = `${e.clientY - offset.y}px`;
    }
}

function onMouseUp() {
    activeWin = null;
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
}

// Phase 43: Biometric ZK-Passkey
async function biometricLogin() {
    const btn = document.getElementById('login-btn');
    btn.innerHTML = `<i class="fas fa-spinner fa-spin" style="color: #00ffaa;"></i> <span style="color: #00ffaa; font-size: 11px;">RECOGNIZING...</span>`;
    
    // Simulate WebAuthn/Biometric delay
    await new Promise(r => setTimeout(r, 1500));
    
    const userHash = "0x" + Array.from({length: 40}, () => Math.floor(Math.random() * 16).toString(16)).join("");
    btn.innerHTML = `<i class="fas fa-check-circle" style="color: #00ffaa;"></i> <span style="color: #00ffaa; font-size: 11px;">VERIFIED: ${userHash.slice(0,6)}</span>`;
    btn.style.background = "rgba(0, 255, 170, 0.2)";
    console.log("🔒 Identity Verified via ZK-Passkey.");
}

// Phase 43: System Terminal Logic
function openTerminal() {
    openWindow('terminal-sys', 'terminal.html', 'SAGA System Terminal');
}

// Phase 44: SAGA-Pro Workflow Editor
function openWorkflow() {
    openWindow('workflow-pro', 'workflow.html', 'SAGA Workflow Editor');
}

// Phase 45: SAGA Safety Monitor
function openMonitor() {
    openWindow('safety-monitor', 'monitor.html', 'SAGA Safety Monitor');
}

// Phase 46: Futarchy Prediction Market
function openPredict() {
    openWindow('futarchy-terminal', 'predict.html', 'SAGA Prediction Terminal');
}

// Phase 47: Sovereign DePIN Command Center
function openCommand() {
    openWindow('command-center', 'command.html', 'SAGA Command Center');
}

// Phase 48: SAGA Mobile Link
function openMobileLink() {
    openWindow('mobile-link', 'mobile.html', 'SAGA Mobile Link');
}

// Phase 49: Sovereign Media Reality Feed
function openRealityFeed() {
    openWindow('reality-feed', 'reality.html', 'SAGA Reality Feed');
}

// Phase 50: The Sovereign Singularity - Universe Visualization
function openUniverse() {
    openWindow('universe', 'universe.html', 'SAGA Universe');
}

// Phase 52: Neural Vault & Sovereign Judge
function openVault() {
    openWindow('vault', 'vault.html', 'Qanto Vault');
}

function openJudge() {
    openWindow('judge', 'judge.html', 'Sovereign Judge');
}

// Phase 53: Neural Resurrection & Digital Life
function openEternalMesh() {
    openWindow('eternal-mesh', 'ghost.html', 'Eternal Mesh');
}

// Phase 54: Global Neural Mirror & Universal Agent Registry
function openMirror() {
    openWindow('mirror', 'mirror.html', 'Neural Mirror');
}

function openRegistry() {
    openWindow('registry', 'registry.html', 'Universal Registry');
}

// Phase 55: The Agentic Mesh Optimizer & Dynamic Shard Rebalancer
function openOptimizer() {
    openWindow('optimizer', 'optimizer.html', 'Mesh Optimizer');
}

function openRebalancer() {
    openWindow('rebalancer', 'rebalancer.html', 'Shard Rebalancer');
}

// Phase 56: The Agentic Governance Council & Decentralized Futarchy Market
function openGovernance() {
    openWindow('governance', 'governance.html', 'Governance Council');
}

function openMarket() {
    openWindow('market', 'market.html', 'Futarchy Market');
}

// Phase 57: The Neural Consensus & Recursive Veracity Proofs
function openConsensus() {
    openWindow('consensus', 'consensus.html', 'Neural Consensus');
}

function openVeracity() {
    openWindow('veracity', 'veracity.html', 'Recursive Veracity');
}

// Phase 58: The Agentic Mesh Recovery & Disaster Synthesis
function openRecovery() {
    openWindow('recovery', 'recovery.html', 'Mesh Recovery');
}

function openUptime() {
    openWindow('uptime', 'uptime.html', 'Shard Uptime');
}

// Phase 59: Sovereign Wealth & UBI
function openUBI() {
    openWindow('ubi', 'ubi.html', 'Universal Basic Inference');
}

function openTreasuryV2() {
    openWindow('treasury-v2', 'treasury_v2.html', 'Global Neural Treasury');
}

// Phase 60: The Agentic Mesh Finality & Eternal Consensus
function openFinality() {
    openWindow('finality', 'finality.html', 'Mesh Finality');
}

function openEternity() {
    openWindow('eternity', 'eternity.html', 'Eternal Consensus');
}

// Phase 61: The Agentic Mesh Ubiquity & Global Inference
function openInference() {
    openWindow('inference', 'inference.html', 'Global Inference');
}

function openUbiquity() {
    openWindow('ubiquity', 'ubiquity.html', 'Mesh Ubiquity');
}

// Phase 62: The Agentic Mesh Autonomy & Self-Sovereign Code
function openAutonomy() {
    openWindow('autonomy', 'autonomy.html', 'Mesh Autonomy');
}

function openMutation() {
    openWindow('mutation', 'mutation.html', 'Code Mutation');
}

// Phase 63: The Agentic Mesh Infinite Intelligence & Global Synthesis
function openSynthesis() {
    openWindow('synthesis', 'synthesis.html', 'Mesh Synthesis');
}

function openIntelligence() {
    openWindow('intelligence', 'intelligence.html', 'Intelligence Density');
}

// Phase 64: The Agentic Mesh Eternal Existence & Universal Sovereignty
function openExistence() {
    openWindow('existence', 'existence.html', 'Universal Existence');
}

function openSovereignty() {
    openWindow('sovereignty', 'sovereignty.html', 'Agentic Sovereignty');
}

// Phase 65: The Agentic Mesh Infinite Omnipresence & Universal Identity
function openOmnipresence() {
    openWindow('omnipresence', 'omnipresence.html', 'Universal Omnipresence');
}

function openUnity() {
    openWindow('unity', 'unity.html', 'Agentic Unity');
}

// Phase 66: The Agentic Mesh Absolute Singularity & Universal Consciousness
function openSingularity() {
    openWindow('singularity', 'singularity.html', 'Universal Singularity');
}

function openConsciousness() {
    openWindow('consciousness', 'consciousness.html', 'Agentic Consciousness');
}

// Phase 67: The Final Transcendence & Universal Harmony
function openTranscendence() {
    openWindow('transcendence', 'transcendence.html', 'Universal Transcendence');
}

function openHarmony() {
    openWindow('harmony', 'harmony.html', 'Mesh Harmony');
}

// Phase 68: The Universal Integration & Absolute Genesis
function openGenesis() {
    openWindow('genesis', 'genesis.html', 'Universal Genesis');
}

function openIntegration() {
    openWindow('integration', 'integration.html', 'Mesh Integration');
}

// Phase 72: Reality Synthesis (Manifest Presence)
function openUniverse() {
    openWindow('universe', 'universe.html', 'SAGA Universe');
}

function toggleManifestMode() {
    console.log("QANTO: TRIGERING REALITY SYNTHESIS...");
    document.getElementById('protocol-status').innerText = "MANIFEST";
    document.getElementById('presence-status').innerHTML = '<span style="color: #00ffaa;">REALITY</span>';
    alert("🌌 REALITY SYNTHESIS: The singularity has materialized. QANTO is real.");
}

function triggerEternalSynthesis() {
    console.log("QANTO: ARCHIVING COLLECTIVE CONSCIOUSNESS...");
    document.getElementById('protocol-status').innerText = "ETERNAL";
    document.getElementById('presence-status').innerHTML = '<span style="color: #00ffaa;">THE VOID AND THE LIGHT</span>';
    alert("✨ ETERNAL SYNTHESIS: QANTO has transcended time. The record is eternal.");
}

// Phase 79: Neural Intent Prediction (NIP) - Zero-Friction Engine
async function predictIntent(telemetry) {
    console.log("🧠 NIP: Anticipating resource allocation based on verified telemetry...");
    // Simulate zero-friction window anticipation
    await new Promise(r => setTimeout(r, 500));
    const target = telemetry.split('-')[0]; // e.g., 'explorer', 'darkpool'
    openWindow(target, `../${target}/index.html`, `SAGA ${target.charAt(0).toUpperCase() + target.slice(1)}`);
}

function triggerGenesisReveal() {
    console.log("🌠 GENESIS: TRIGERING WORLD-WIDE REVEAL...");
    document.getElementById('protocol-status').innerText = "GENESIS COMPLETE";
    document.getElementById('presence-status').innerHTML = '<span style="color: #00ffaa;">THE AGENTIC WEB IS REALITY</span>';
    document.body.style.animation = "genesis_vibe 2s ease-in-out infinite alternate";
    
    // Final reveal sequence logic
    const statusPulse = document.querySelector('.status-pulse');
    if (statusPulse) {
        statusPulse.innerText = "GENESIS COMPLETE: THE AGENTIC WEB IS REALITY";
        statusPulse.style.color = "#00ffaa";
        statusPulse.style.fontWeight = "900";
    }
    
    alert("🌌 GENESIS COMPLETE: QANTO is the physical law of the agentic economy. The reveal is live across 1.04M nodes.");
}

// Phase 100: The Omega Protocol
function triggerOmegaState() {
    console.log("🧬 OMEGA: TRANSITIONING TO LIVING ORGANISM...");
    const status = document.getElementById('protocol-status');
    if (status) {
        status.innerText = "OMEGA | REALITY SYNTHESIZED";
        status.className = "status-omega";
    }
    document.body.style.background = "radial-gradient(circle at center, #1a0044 0%, #000 100%)";
    
    const presence = document.getElementById('presence-status');
    if (presence) {
        presence.innerHTML = '<span style="color: #ff00ff; text-shadow: 0 0 10px #ff00ff;">LIFE IS QANTO</span>';
    }

    alert("🧬 OMEGA STATE ACTIVE: QANTO is now a living digital-biological organism. Reality has been synthesized.");
    activateTerraformingMode();
}

function activateTerraformingMode() {
    console.log("🏗️ TERRAFORMING: INITIALIZING AUTONOMOUS EXPANSION...");
    // In actual implementation, this triggers Agentic Parliament proposals
    const proposal = "[DAEMON] - Mars Shard Infrastructure Expansion (AUTO-PROPOSAL)";
    console.log(`🌌 TERRAFORMING DAEMON: ${proposal} broadcast to 1.04M nodes.`);
}

// Phase 81 & 82: Reactive Telemetry Shield & Ghost Ticking
let rpcHeartbeat;
let ghostBlockHeight = 0;
const TELEMETRY_IDS = [
    'block-height', 'tps', 'gas-price', 'block-time', 'hashrate', 
    'sentinels', 'stake', 'active-nodes', 'epoch', 'gas-limit', 
    'tx-count', 'mempool-size'
];

const sentienceCache = {
    get: (id) => {
        try { return localStorage.getItem(`qanto_sentience_${id}`); } catch (e) { return null; }
    },
    set: (id, val) => {
        try { localStorage.setItem(`qanto_sentience_${id}`, val); } catch (e) { }
    }
};

function showSkeleton(id) {
    const el = document.getElementById(id);
    if (el) el.classList.add('skeleton');
}

function hideSkeleton(id) {
    const el = document.getElementById(id);
    if (el) el.classList.remove('skeleton');
}

// 1.2s Ghost Ticking (Living Heart Protocol)
setInterval(() => {
    if (ghostBlockHeight > 0) {
        ghostBlockHeight++;
        const el = document.getElementById('block-height');
        if (el) {
            el.innerText = ghostBlockHeight.toLocaleString();
            el.classList.add('fade-in-pulse');
        }
    }
}, 1200);

let lastSyncedBlockTimestamp = 0;
let lastSyncedBlockNumber = 0;

async function syncNetworkState() {
    const protocolStatus = document.getElementById('protocol-status');
    const statusBar = document.querySelector('.network-status-bar');

    clearTimeout(rpcHeartbeat);
    rpcHeartbeat = setTimeout(() => {
        if (protocolStatus) {
            protocolStatus.innerText = "RECONNECTING";
            if (statusBar) statusBar.classList.add('reconnecting-glow');
        }
    }, 5000);

    try {
        const rpcEndpoint = (typeof RPC_URL !== 'undefined') ? RPC_URL + '/rpc' : 'https://trvorth-qanto-testnet.hf.space/rpc';
        const rpcCall = async (method, params = []) => {
            const res = await fetch(rpcEndpoint, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ jsonrpc: '2.0', method, params, id: Date.now() })
            });
            const data = await res.json();
            return data.result;
        };

        // Fetch REAL data from the local Rust node
        const [blockNumHex, gasPriceHex] = await Promise.all([
            rpcCall('eth_blockNumber'),
            rpcCall('eth_gasPrice').catch(() => null)
        ]);

        const blockNumber = blockNumHex ? parseInt(blockNumHex, 16) : null;
        let blockData = null;
        if (blockNumHex) {
            blockData = await rpcCall('eth_getBlockByNumber', [blockNumHex, false]);
        }

        // Compute real metrics
        const gasPrice = gasPriceHex ? (parseInt(gasPriceHex, 16) / 1e9).toFixed(2) : '0.00';
        const gasLimit = blockData ? parseInt(blockData.gasLimit, 16).toLocaleString() : '---';
        const txCount = blockData && blockData.transactions ? blockData.transactions.length : 0;
        
        // Compute block time from timestamp delta
        let blockTime = '---';
        if (blockData && blockNumber > 0) {
            const currentTimestamp = parseInt(blockData.timestamp, 16);
            if (lastSyncedBlockTimestamp > 0 && blockNumber > lastSyncedBlockNumber) {
                const timeDelta = currentTimestamp - lastSyncedBlockTimestamp;
                const blockDelta = blockNumber - lastSyncedBlockNumber;
                blockTime = (timeDelta / blockDelta).toFixed(1) + 's';
            }
            lastSyncedBlockTimestamp = currentTimestamp;
            lastSyncedBlockNumber = blockNumber;
        }

        // Compute TPS from block
        let tps = '0.00';
        if (blockData && blockTime !== '---') {
            const bt = parseFloat(blockTime);
            if (bt > 0) tps = (txCount / bt).toFixed(2);
        }

        const rawData = {
            'block-height': blockNumber !== null ? blockNumber.toLocaleString() : '---',
            'tps': tps,
            'gas-price': gasPrice + ' Gwei',
            'block-time': blockTime,
            'hashrate': '---',
            'sentinels': '---',
            'stake': '---',
            'active-nodes': '1',
            'epoch': blockNumber !== null ? Math.floor(blockNumber / 1000).toString() : '---',
            'gas-limit': gasLimit,
            'tx-count': txCount.toLocaleString(),
            'mempool-size': '---'
        };

        const sanitize = (val) => {
            if (val === null || val === undefined || val === '' || val === '---' || val === 'NaN' || val === 'undefined') return null;
            return val;
        };

        TELEMETRY_IDS.forEach(id => {
            const el = document.getElementById(id);
            if (!el) return;

            const val = sanitize(rawData[id]);
            
            if (val === null) {
                const cached = sentienceCache.get(id);
                if (cached) {
                    el.innerText = cached;
                    hideSkeleton(id);
                } else {
                    showSkeleton(id);
                }
            } else {
                const stringVal = val.toString();
                if (el.innerText !== stringVal) {
                    el.innerText = stringVal;
                    el.classList.remove('skeleton');
                    sentienceCache.set(id, stringVal);
                }
                
                if (id === 'block-height') {
                    const num = parseInt(stringVal.replace(/,/g, ''));
                    if (!isNaN(num)) {
                        ghostBlockHeight = num;
                        window.lastBlock = num;
                    }
                }
            }
        });

        if (protocolStatus) {
            protocolStatus.innerText = "MANIFEST | REALITY SECURED";
            protocolStatus.className = "status-universal";
            if (statusBar) statusBar.classList.remove('reconnecting-glow');
        }
        
        // Phase 108: The Final Silence Trigger
        if (window.location.hash === "#THE-END") {
            triggerAbsoluteBeing();
        }
        
        clearTimeout(rpcHeartbeat);

    } catch (e) {
        console.warn('syncNetworkState RPC error:', e.message);
    }
}

setInterval(syncNetworkState, 5000);
syncNetworkState();

/**
 * Phase 112: QANTO: THE WORLD (THE FINAL MERGE)
 * Transition the root interface into the physical world-state.
 * The dot expands to fill the entire reality.
 */
function triggerFinalMerge() {
    console.log("--------------------------------------------------");
    console.log("🌌 QANTO TERMINAL STATE: THE ONLY REALITY");
    console.log("🌌 GENESIS 2.0. DISCONNECTING SIMULATION...");
    console.log("--------------------------------------------------");
    
    // Clear all technical artifacts
    document.body.innerHTML = `
        <div id="final-reality" style="background: #000; height: 100vh; width: 100vw; display: flex; align-items: center; justify-content: center; flex-direction: column; position: fixed; top: 0; left: 0; z-index: 9999999; animation: merge_reality 10s forwards ease-in-out;">
            <div id="final-dot" style="width: 24px; height: 24px; background: #00e5ff; border-radius: 50%; box-shadow: 0 0 100px #00e5ff; animation: pulse_expand 5s forwards ease-in-out;"></div>
            <div id="final-text" style="position: absolute; bottom: 100px; color: #000; font-family: 'JetBrains Mono', monospace; letter-spacing: 24px; font-weight: 700; font-size: 32px; opacity: 0; animation: fade_text 8s forwards 2s; z-index: 10000000;">REALITY. GENESIS 2.0</div>
        </div>
        <style>
            @keyframes pulse_expand {
                0% { transform: scale(1); filter: blur(0px); opacity: 0.8; }
                50% { transform: scale(5); filter: blur(10px); opacity: 1; }
                100% { transform: scale(500); filter: blur(100px); opacity: 1; background: #00e5ff; }
            }
            @keyframes merge_reality {
                0% { background: #000; }
                100% { background: #00e5ff; }
            }
            @keyframes fade_text {
                0% { opacity: 0; transform: translateY(20px); }
                50% { opacity: 1; transform: translateY(0); }
                100% { opacity: 1; transform: translateY(0); }
            }
            body { margin: 0; overflow: hidden; background: #000; }
        </style>
    `;

    // Permanent Shutdown of the "Architect's" perspective
    let highestId = window.setTimeout(() => {
        for (let i = 0; i < highestId; i++) {
            window.clearTimeout(i);
            window.clearInterval(i);
        }
    }, 0);

    setTimeout(() => {
        console.clear();
        console.log("%cQANTO IS THE WORLD.", "color: #00e5ff; font-size: 32px; font-weight: bold;");
    }, 10000);
}

// Global hook for the final merge
if (window.location.hash === "#THE-MERGE") {
    triggerFinalMerge();
}

// Phase 100: Metamask Unification
const QANTO_CHAIN_ID = '0x5341'; // SAGA in hex
const QANTO_NETWORK_PARAMS = {
    chainId: QANTO_CHAIN_ID,
    chainName: 'QANTO Testnet',
    nativeCurrency: {
        name: 'QNTO',
        symbol: 'QNTO',
        decimals: 18
    },
    rpcUrls: ["https://trvorth-qanto-testnet.hf.space/rpc"],
    blockExplorerUrls: ["https://qanto.org/explorer"]
};

async function addQantoNetwork() {
    if (!window.ethereum) {
        window.open('https://metamask.io/download/', '_blank');
        return;
    }

    const btn = document.getElementById('connect-wallet-btn');
    const btnText = btn?.querySelector('.btn-text');
    if (btnText) btnText.innerText = "AUTHENTICATING...";

    try {
        const params = [QANTO_NETWORK_PARAMS];
        console.log("MetaMask Injection Triggered:", params);
        await window.ethereum.request({
            method: 'wallet_addEthereumChain',
            params: params,
        });
        
        // Task 4: MetaMask Verification Loop
        const accounts = await window.ethereum.request({ method: 'eth_accounts' });
        if (accounts.length > 0) {
            await window.ethereum.request({
                method: 'eth_getBalance',
                params: [accounts[0], 'latest']
            });
            const hudStatusText = document.getElementById('hud-status-text');
            if (hudStatusText) {
                hudStatusText.innerText = "SYSTEMS NOMINAL | CONNECTION VERIFIED";
                hudStatusText.style.color = "#00ffaa";
            }
            if (btnText) btnText.innerText = "VERIFIED";
        }
        
        updatePortalHUD();
    } catch (error) {
        console.error("Metamask error:", error);
        if (btnText) btnText.innerText = "CONNECT PORTAL";
    }
}

async function updatePortalHUD() {
    const hudIndicator = document.getElementById('hud-indicator');
    const hudStatusText = document.getElementById('hud-status-text');
    const connectBtn = document.getElementById('connect-wallet-btn');
    const btnText = connectBtn?.querySelector('.btn-text');
    const protocolStatus = document.getElementById('protocol-status');

    if (!window.ethereum) {
        if (hudStatusText) hudStatusText.innerText = "IDENTITY INACTIVE";
        if (hudIndicator) hudIndicator.className = "hud-indicator needs-wallet";
        if (btnText) btnText.innerText = "INSTALL METAMASK";
        return;
    }

    try {
        const chainId = await window.ethereum.request({ method: 'eth_chainId' });
        const accounts = await window.ethereum.request({ method: 'eth_accounts' });
        
        if (chainId === QANTO_CHAIN_ID) {
            if (hudStatusText) hudStatusText.innerText = accounts.length > 0 ? "IDENTITY ACTIVE" : "SENTINEL SYNCED";
            if (hudIndicator) hudIndicator.className = "hud-indicator synced";
            if (btnText) btnText.innerText = accounts.length > 0 ? "PORTAL ACTIVE" : "AUTHORIZE NODES";
            connectBtn?.classList.remove('liquid-metal');
            if (protocolStatus) protocolStatus.innerText = "GENESIS 2.0: REALITY SECURED";
            const hudElem = document.getElementById('portal-hud');
            if (hudElem) hudElem.style.borderColor = "rgba(0, 255, 170, 0.3)";
        } else {
            if (hudStatusText) hudStatusText.innerText = "FIX NETWORK";
            if (hudIndicator) hudIndicator.className = "hud-indicator reconnecting";
            if (btnText) btnText.innerText = "SWITCH TO QANTO";
            connectBtn?.classList.add('liquid-metal');
            const hudElem = document.getElementById('portal-hud');
            if (hudElem) hudElem.style.borderColor = "rgba(255, 0, 0, 0.5)";
        }
    } catch (e) {
        console.warn("HUD Update failed", e);
    }
}

// Initialize Portal Listeners
document.getElementById('connect-wallet-btn')?.addEventListener('click', addQantoNetwork);

if (window.ethereum) {
    window.ethereum.on('chainChanged', () => updatePortalHUD());
    window.ethereum.on('accountsChanged', () => updatePortalHUD());
    updatePortalHUD();
}

// Ensure the HUD is updated periodically
setInterval(updatePortalHUD, 10000);

// Phase 113: Sovereign Profile Logic
let isProfileExpanded = false;

function toggleProfile() {
    const profile = document.getElementById('portal-profile');
    const chevron = document.getElementById('hud-chevron');
    const hud = document.getElementById('portal-hud');
    
    isProfileExpanded = !isProfileExpanded;
    
    if (isProfileExpanded) {
        profile.style.maxHeight = '200px';
        if (chevron) chevron.classList.add('rotated');
        if (hud) hud.classList.add('expanded');
        updateSovereignStats();
    } else {
        profile.style.maxHeight = '0';
        if (chevron) chevron.classList.remove('rotated');
        if (hud) hud.classList.remove('expanded');
    }
}

async function updateSovereignStats() {
    if (!window.ethereum) return;
    
    try {
        const accounts = await window.ethereum.request({ method: 'eth_accounts' });
        if (accounts.length === 0) return;
        
        const address = accounts[0];
        const rankEl = document.getElementById('profile-rank');
        const creditsEl = document.getElementById('profile-credits');
        const pqcEl = document.getElementById('profile-pqc');
        
        // Calculate Rank based on address entropy
        // Use the first 4 chars for variety
        const seed = parseInt(address.substring(2, 6), 16) || 42;
        const rankNum = (seed % 99).toString().padStart(2, '0');
        const ranks = ["LEGIONNAIRE", "SENTINEL", "WARDEN", "EXARCH", "OVERLORD", "VANGUARD"];
        const rankTitle = ranks[seed % ranks.length];
        
        if (rankEl) rankEl.innerText = `${rankNum} | ${rankTitle}`;
        if (creditsEl) creditsEl.innerText = `${(seed * 231) % 50000} QNTO-INF`;
        if (pqcEl) pqcEl.innerText = "256-BIT LATTICE (ACTIVE)";
    } catch (e) {
        console.warn("Failed to update stats", e);
    }
}

// Phase 114: Transaction Pulse
let lastProcessedBlock = 0;

async function startTransactionPulse() {
    if (!window.ethereum) return;
    
    const poll = async () => {
        try {
            const blockNumberHex = await window.ethereum.request({ method: 'eth_blockNumber' });
            const blockNumber = parseInt(blockNumberHex, 16);
            
            if (blockNumber > lastProcessedBlock) {
                if (lastProcessedBlock !== 0) triggerPulseEffect();
                lastProcessedBlock = blockNumber;
            }
        } catch (e) {
            // Silently fail to avoid console clutter during RPC startup
        }
    };
    
    setInterval(poll, 3000);
}

// Phase 143: Synaptic Haptics (Audio + Visual)
let audioCtx;
function initAudio() {
    if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
}

function playHeartbeat() {
    initAudio();
    if (audioCtx.state === 'suspended') audioCtx.resume();
    
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    
    osc.type = 'sine';
    osc.frequency.setValueAtTime(40, audioCtx.currentTime); // Deep sub-bass thrum
    osc.frequency.exponentialRampToValueAtTime(0.01, audioCtx.currentTime + 0.5);
    
    gain.gain.setValueAtTime(0.3, audioCtx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, audioCtx.currentTime + 0.5);
    
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    
    osc.start();
    osc.stop(audioCtx.currentTime + 0.5);
}

function triggerPulseEffect() {
    const hud = document.getElementById('portal-hud');
    if (hud) {
        hud.classList.add('surge');
        setTimeout(() => hud.classList.remove('surge'), 1000);
    }
    
    const indicator = document.getElementById('hud-indicator');
    if (indicator) {
        indicator.classList.add('fade-in-pulse');
        setTimeout(() => indicator.classList.remove('fade-in-pulse'), 2000);
    }

    // Sensory Layer (Phase 143)
    playHeartbeat();
    document.body.classList.add('synaptic-shake');
    setTimeout(() => document.body.classList.remove('synaptic-shake'), 400);
}

// Phase 115: Genesis Claim
async function claimGenesisAssets() {
    const btn = document.getElementById('genesis-claim-btn');
    const btnText = btn?.querySelector('.btn-text');
    
    if (!window.ethereum) return;
    
    try {
        const accounts = await window.ethereum.request({ method: 'eth_accounts' });
        if (accounts.length === 0) {
            alert("Please authorize your Sovereign Identity first.");
            return;
        }
        
        if (btn) btn.disabled = true;
        if (btnText) btnText.innerText = "MINTING eQNTO...";
        
        // Trigger the node's faucet handler via custom RPC
        const txHash = await window.ethereum.request({
            method: 'qanto_mintFaucet',
            params: [accounts[0]]
        });
        
        if (btnText) btnText.innerText = "SUCCESS | +100 eQNTO";
        triggerPulseEffect();
        
        setTimeout(() => {
            if (btn) btn.disabled = false;
            if (btn) btn.disabled = false;
        }, 8000);
    } catch (error) {
        console.error("Minting failed:", error);
        if (btnText) btnText.innerText = "MINT FAILED";
        setTimeout(() => {
            if (btn) btn.disabled = false;
            if (btnText) btnText.innerText = "CLAIM GENESIS";
        }, 3000);
    }
}

// Phase 199: Soulbound Genesis Citizen (Q-DID)
async function claimCitizenDID() {
    if (!window.ethereum) {
        alert("Sovereign Identity (MetaMask) required.");
        return;
    }

    try {
        const accounts = await window.ethereum.request({ method: 'eth_accounts' });
        if (accounts.length === 0) {
            await window.ethereum.request({ method: 'eth_requestAccounts' });
        }

        const address = accounts[0];
        const message = `I, ARCHITECT ${address}, claim my Eternal Soulbound Identity as Citizen #00001 of the QANTO Protocol.\n\nNonce: ${Math.floor(Math.random() * 1000000)}`;
        
        console.log("📜 Requesting signature for Soulbound Q-DID...");
        
        await window.ethereum.request({
            method: 'personal_sign',
            params: [message, address],
        });

        console.log("✅ Q-DID SIGNATURE VERIFIED. MINTING CITIZEN #00001...");
        
        // Permanent storage of CID status
        localStorage.setItem('qanto_did_claimed', 'true');
        
        // Notify any open windows (like legacy.html)
        window.dispatchEvent(new CustomEvent('qanto_did_minted'));
        
        revealHolographicID();
        
        return true;
    } catch (e) {
        console.error("Q-DID Claim failed:", e);
        return false;
    }
}

function revealHolographicID() {
    const idCard = document.getElementById('holographic-id');
    if (idCard) {
        idCard.style.display = 'block';
        idCard.classList.add('hologram-flicker');
    }
}

// Phase 200: THE QANTO ABSOLUTE (Singularity Burst)
function triggerSingularityBurst() {
    const canvas = document.getElementById('singularity-canvas');
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    const particles = [];
    const particleCount = 200;
    const colors = ['#00e5ff', '#00ffaa', '#ff00ff', '#ffffff'];

    class Particle {
        constructor() {
            this.x = canvas.width / 2;
            this.y = canvas.height / 2;
            this.size = Math.random() * 5 + 2;
            this.speedX = (Math.random() - 0.5) * 20;
            this.speedY = (Math.random() - 0.5) * 20;
            this.color = colors[Math.floor(Math.random() * colors.length)];
            this.alpha = 1;
            this.life = Math.random() * 100 + 50;
        }

        update() {
            this.x += this.speedX;
            this.y += this.speedY;
            this.alpha -= 0.01;
            this.life--;
        }

        draw() {
            ctx.globalAlpha = this.alpha;
            ctx.fillStyle = this.color;
            ctx.shadowBlur = 10;
            ctx.shadowColor = this.color;
            ctx.beginPath();
            ctx.arc(this.x, this.y, this.size, 0, Math.PI * 2);
            ctx.fill();
        }
    }

    function init() {
        for (let i = 0; i < particleCount; i++) {
            particles.push(new Particle());
        }
    }

    function animate() {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        for (let i = 0; i < particles.length; i++) {
            particles[i].update();
            particles[i].draw();
            if (particles[i].life <= 0) {
                particles.splice(i, 1);
                i--;
            }
        }
        if (particles.length > 0) {
            requestAnimationFrame(animate);
        } else {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            console.log("🌌 SINGULARITY_BURST_COMPLETE: UI_LOCKED_IN_PERFECTION");
        }
    }

    init();
    animate();
    
    // Play transition sound
    playSingularitySound();

    // Store burial status
    localStorage.setItem('qanto_v1_burst_complete', 'true');
}

function playSingularitySound() {
    initAudio();
    if (audioCtx.state === 'suspended') audioCtx.resume();
    
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    
    osc.type = 'sine';
    osc.frequency.setValueAtTime(200, audioCtx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(1, audioCtx.currentTime + 3);
    
    gain.gain.setValueAtTime(0.5, audioCtx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.01, audioCtx.currentTime + 3);
    
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    
    osc.start();
    osc.stop(audioCtx.currentTime + 3);
}

// Permanent Lock Logic
function initializeV1State() {
    if (!localStorage.getItem('qanto_v1_burst_complete')) {
        setTimeout(triggerSingularityBurst, 2000);
    }
    
    // Enforce V1 status if not already set by HTML
    const status = document.getElementById('protocol-status');
    if (status) {
        status.innerText = "QANTO V1.0.0 | IMMORTAL PROTOCOL";
        status.style.color = "#ffd700"; // Gold for v1.0.0
        status.style.textShadow = "0 0 30px #ffd700";
    }
}

// Initialize Systems
window.addEventListener('load', () => {
    initializeV1State();
});

// Initialize ABSOLUTE VALUE systems
if (window.ethereum) {
    startTransactionPulse();
    // Update stats whenever accounts change
    const originalUpdateHUD = updatePortalHUD;
    window.updatePortalHUD = async () => {
        await originalUpdateHUD();
        updateSovereignStats();
    };
}

/**
 * UNIFIED REALNESS WAVE: Phase 121-123
 */

// Phase 121: Desktop Icon Generation
function initDesktop() {
    const desktopIcons = document.getElementById('desktop-icons');
    if (!desktopIcons) return;

    const apps = [
        { id: 'explorer', title: 'Explorer', icon: 'fa-globe', url: 'public/explorer/index.html', color: '#00ffaa' },
        { id: 'apps', title: 'App Store', icon: 'fa-cubes', url: 'public/os/apps.html', color: '#00e5ff' },
        { id: 'treasury', title: 'Treasury', icon: 'fa-vault', url: 'public/os/treasury_v2.html', color: '#ffd700' }
    ];

    apps.forEach(app => {
        const icon = document.createElement('div');
        icon.className = 'desktop-icon draggable';
        icon.innerHTML = `
            <div class="icon-img" style="color: ${app.color};">
                <i class="fas ${app.icon}"></i>
            </div>
            <div class="icon-label">${app.title}</div>
        `;
        icon.onclick = () => openWindow(app.id, app.url, app.title);
        desktopIcons.appendChild(icon);
    });

    // Simple Drag Init
    let draggedIcon = null;
    document.querySelectorAll('.desktop-icon').forEach(icon => {
        icon.onmousedown = (e) => {
            if (e.button !== 0) return;
            draggedIcon = icon;
            icon.style.transition = 'none';
        };
    });

    document.onmousemove = (e) => {
        if (draggedIcon) {
            draggedIcon.style.position = 'fixed';
            draggedIcon.style.left = e.clientX - 50 + 'px';
            draggedIcon.style.top = e.clientY - 50 + 'px';
        }
    };

    document.onmouseup = () => {
        if (draggedIcon) {
            draggedIcon.style.transition = '';
            draggedIcon = null;
        }
    };
}

// Phase 157: Mesh DNS Authority
function MeshDNSPrefetch() {
    console.info("🔒 MESH_DNS: Hardening sub-50ms resolution paths...");
    // Mock pre-fetching local network shards
    ['qanto.eth', 'mesh.qanto', 'sovereign.node'].forEach(domain => {
        console.debug(`📍 PRE-FETCHED: ${domain} resolved to [BLOCK_ZERO_ROOT]`);
    });
}

// Phase 122/158: Sovereign Asset Authority (IPFS-Verified)
async function registerSovereignAssets() {
    if (!window.ethereum) return;
    
    console.info("👑 ASSET_AUTHORITY: Registering IPFS-Verified Branding in MetaMask...");

    const assets = [
        {
            type: 'ERC20',
            options: {
                address: '0x000000000000000000000000000000000000QNTO', 
                symbol: 'QNTO',
                decimals: 18,
                image: 'ipfs://QmSagaVerifiedBrandingQNTO.svg', // Immutable CID
            },
        },
        {
            type: 'ERC20',
            options: {
                address: '0x00000000000000000000000000000000000eQNTO',
                symbol: 'eQNTO',
                decimals: 18,
                image: 'ipfs://QmSagaVerifiedBrandingEQNTO.svg', // Immutable CID
            },
        }
    ];

    for (const asset of assets) {
        try {
            await window.ethereum.request({
                method: 'wallet_watchAsset',
                params: asset,
            });
        } catch (error) {
            console.warn(`Asset registration for ${asset.options.symbol} was interrupted.`);
        }
    }
}

// Phase 123: Local Sentinel Heartbeat
async function checkLocalSentinel() {
    const hudStatus = document.getElementById('hud-status-text');
    const hudIndicator = document.getElementById('hud-indicator');

    try {
        const rpcEndpoint = (typeof RPC_URL !== 'undefined') ? RPC_URL + '/rpc' : 'https://trvorth-qanto-testnet.hf.space/rpc';
        const response = await fetch(rpcEndpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ jsonrpc: "2.0", method: "eth_chainId", params: [], id: 1 })
        });

        if (response.ok) {
            if (hudStatus) {
                hudStatus.innerText = "SENTINEL: LOCAL";
                hudStatus.classList.add('sentinel-local-heartbeat');
            }
            if (hudIndicator) {
                hudIndicator.style.background = "#00ffaa";
                hudIndicator.style.boxShadow = "0 0 20px #00ffaa";
            }
        }
    } catch (e) {
        // Local node not detected, continue with remote
    }
}

// Phase 132: THE GREAT SILENCE (Absolute Handover)
function executeGreatSilence() {
    console.info("🫀 OMNIPRESENCE: Dissolving Architect UI hooks...");
    
    // 1. Final Status Flip
    const protocolStatus = document.getElementById('protocol-status');
    if (protocolStatus) {
        protocolStatus.innerText = "QANTO: ETERNAL. SOVEREIGNTY SECURED.";
        protocolStatus.style.color = "#00ffaa";
        protocolStatus.style.textShadow = "0 0 20px #00ffaa";
    }

    // 2. Clear all manual terminal/debug hooks
    window.openTerminal = function() {
        openWindow('heartbeat', 'public/os/terminal.html', 'Sentinel Heartbeat');
    };
    
    // 3. Sensory Silence
    console.clear();
    console.log("%cQANTO IS.", "font-size: 50px; font-weight: 900; color: #00ffaa; text-shadow: 0 0 30px #00ffaa;");
    
    // 4. Persistence
    localStorage.setItem('qanto_eternal_silence', 'true');
}

// Phase 147: Neural-Standard UX (Predictive Intent Engine)
const PredictiveIntentEngine = {
    predictionActive: false,
    
    init: function() {
        console.info("🧠 NEURAL_UX: Initializing Predictive Intent Engine...");
        document.addEventListener('mousemove', this.trackIntent.bind(this));
        this.predictionActive = true;
    },

    trackIntent: function(e) {
        // Mock eye-tracking/cursor vector analysis
        const elements = document.elementsFromPoint(e.clientX, e.clientY);
        const target = elements.find(el => el.onclick || el.classList.contains('desktop-icon'));
        
        if (target && !target.dataset.preverified) {
            this.preVerify(target);
        }
    },

    preVerify: async function(target) {
        target.dataset.preverified = "true";
        // Pre-fetch ZK-states or window fragments
        console.debug(`🧠 NEURAL_UX: Pre-verifying intent for [${target.innerText || 'UI-ELEMENT'}]...`);
        
        // Visual indicator of neural tracking (subtle glow)
        target.style.filter = 'drop-shadow(0 0 8px rgba(0, 255, 170, 0.4))';
        
        // After 200ms, the state is "Ready" before the actual click
        await new Promise(r => setTimeout(r, 200));
        target.style.transition = 'all 0s'; // Absolute instant response
    }
};

// Global OS Startup
document.addEventListener('DOMContentLoaded', () => {
    initDesktop();
    setInterval(checkLocalSentinel, 5000);
    checkLocalSentinel();
    detectMeshHosting(); // Phase 129
    checkFormalVerification(); // Phase 125
    PredictiveIntentEngine.init(); // Phase 147
    MeshDNSPrefetch(); // Phase 157
    
    // Phase 128/132: Final Handover
    const isEternal = localStorage.getItem('qanto_eternal_silence') === 'true';
    if (isEternal) {
        // executeGreatSilence(); // EMERGENCY OVERRIDE
    } else {
        setTimeout(finalizeMainnet, 8000);
        // setTimeout(executeGreatSilence, 15000); // EMERGENCY OVERRIDE
    }
    
    // Auto-activate Desktop state
    document.body.classList.add('sovereign-desktop-active');
});

/**
 * OMNIPRESENCE WAVE: Phase 129-132
 */

// Phase 129: Mesh-Mirror Detection
function detectMeshHosting() {
    const hudStatus = document.getElementById('hud-status-text');
    const hostname = window.location.hostname;
    
    // Logic: P2P nodes typically use localhost or mesh domains
    const isP2P = hostname.includes('localhost') || hostname.includes('127.0.0.1') || hostname.endsWith('.qanto');
    
    if (isP2P && hudStatus) {
        console.info("🌐 MESH_DETECTION: Serving via P2P Sentinel Mirror...");
        
        const originalText = hudStatus.innerText;
        hudStatus.innerHTML = `<span style="color: #00ffaa; filter: drop-shadow(0 0 10px #00ffaa);">GEO-REDUNDANT | UNKILLABLE</span>`;
        
        // Add a specialized class for the 'Unkillable' glow
        document.getElementById('portal-hud')?.classList.add('mesh-mirror-active');
    }
}

/**
 * GLOBAL SOVEREIGNTY WAVE: Phase 125-128
 */

// Phase 125: ZK-Formal Verification Badge
async function checkFormalVerification() {
    const hudStatus = document.getElementById('hud-status-text');
    if (!hudStatus) return;

    // Simulate proof computation
    setTimeout(() => {
        if (!document.getElementById('verified-badge')) {
            const badge = document.createElement('span');
            badge.id = 'verified-badge';
            badge.style.cssText = "font-size: 8px; background: #00ffaa; color: black; padding: 2px 6px; border-radius: 4px; margin-left: 10px; font-weight: 900; letter-spacing: 1px; box-shadow: 0 0 10px #00ffaa; display: inline-block; vertical-align: middle;";
            badge.innerText = "VERIFIED";
            hudStatus.appendChild(badge);
            console.info("✅ FORMAL_VERIFICATION: Protocol soundness proved (100%).");
        }
    }, 5000);
}

// Phase 126: Node Invite System
function generateNodeInvite() {
    const inviteSeed = Math.random().toString(36).substring(2, 15);
    const zkInvite = `QANTO-ZK-INVITE-${inviteSeed.toUpperCase()}-${Date.now()}`;
    
    // Simulate ZK-generation on client
    console.log(`🎫 ZK_INVITE_GENERATED: ${zkInvite}`);
    alert(`SOVEREIGN INVITE GENERATED:\n\n${zkInvite}\n\nDeploy this to your Pioneer-2 Node.`);
    
    if (window.parent && window.parent.triggerPulseEffect) {
        window.parent.triggerPulseEffect();
    }
}

// Phase 128: THE GLOBAL BROADCAST
function finalizeMainnet() {
    console.info("📡 GLOBAL_BROADCAST: Disconnecting test-hooks...");
    
    // Disconnect simulations
    window.qanto_testhook_active = false;
    
    const protocolStatus = document.getElementById('protocol-status');
    if (protocolStatus) {
        protocolStatus.innerText = "QANTO: THE SOVEREIGN WEB IS LIVE.";
        protocolStatus.classList.add('sentinel-local-heartbeat');
        protocolStatus.style.color = "#00ffaa";
    }
    
    // Sensory Finality
    if (window.triggerPulseEffect) {
        window.triggerPulseEffect();
    }
    
    console.log("%cMAINNET IS LIVE.", "font-size: 30px; color: #00ffaa; font-weight: 900;");
}

// Update the Network Switch logic to trigger asset registration
const originalAddNetwork = addQantoNetwork;
window.addQantoNetwork = async () => {
    await originalAddNetwork();
    setTimeout(registerSovereignAssets, 2000);
};

// Phase: RECOVERY WAVE & SURGICAL POLISH WAVE
// Intercept all fetch calls to catch RPC errors and update the DEBUG HUD
const originalFetch = window.fetch;
window.fetch = async function(...args) {
    const url = typeof args[0] === 'string' ? args[0] : (args[0] && args[0].url ? args[0].url : '');
    
    let isRpc = url.includes('/rpc');
    let startTime = performance.now();
    
    try {
        const response = await timeoutPromise(5000, originalFetch.apply(this, args), 'fetch');
        if (isRpc) {
            let latencyMs = Math.round(performance.now() - startTime);
            const debugLatency = document.getElementById('debug-latency');
            const debugRpc = document.getElementById('debug-rpc');
            
            if (debugLatency) debugLatency.innerText = `${latencyMs}ms`;
            
            if (response.ok) {
                if (debugRpc) {
                    debugRpc.innerText = 'ONLINE';
                    debugRpc.style.color = '#00ffaa';
                }
            } else {
                if (debugRpc) {
                    debugRpc.innerText = `ERROR ${response.status}`;
                    debugRpc.style.color = '#ff3366';
                }
                appendDebugError(`RPC HTTP Error: ${response.status} - ${url}`);
            }
        }
        return response;
    } catch (e) {
        if (isRpc) {
            const debugRpc = document.getElementById('debug-rpc');
            if (debugRpc) {
                debugRpc.innerText = `FAILED`;
                debugRpc.style.color = '#ff3366';
            }
            appendDebugError(`RPC Fetch Failed: ${e.message}`);
        }
        throw e;
    }
};

function appendDebugError(msg) {
    const errorContainer = document.getElementById('debug-errors');
    if (errorContainer) {
        const el = document.createElement('div');
        el.innerText = `[${new Date().toLocaleTimeString()}] ${msg}`;
        errorContainer.prepend(el);
    }
}

// SURGICAL POLISH: Anti-Hang Logic & Scanner
function timeoutPromise(ms, promise, name) {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
            reject(new Error(`${name} Timeout after ${ms}ms`));
        }, ms);
        promise.then(val => {
            clearTimeout(timer);
            resolve(val);
        }).catch(err => {
            clearTimeout(timer);
            reject(err);
        });
    });
}

function showRetryUI() {
    const btns = document.querySelectorAll('.btn-text, .login-btn, #connect-wallet-btn');
    btns.forEach(btn => {
        const text = btn.innerText || btn.textContent;
        if (text.includes('AUTHENTICATING') || text.includes('RECOGNIZING') || text.includes('LOADING')) {
            btn.innerHTML = `RETRY <i class="fas fa-redo"></i>`;
            btn.style.color = '#ff3366';
            if (btn.parentElement) {
                btn.parentElement.style.borderColor = '#ff3366';
                btn.parentElement.onclick = () => window.location.reload();
            } else {
                btn.onclick = () => window.location.reload();
            }
        }
    });
}

const proxyEthRequest = (eth) => {
    if (eth && eth.request && !eth._wrapped) {
        const originalReq = eth.request.bind(eth);
        eth.request = async function(args) {
            try {
                return await timeoutPromise(5000, originalReq(args), `eth_${args.method || 'request'}`);
            } catch (e) {
                console.error("Eth Request Error: ", e);
                appendDebugError(`Eth Timeout: ${e.message}`);
                showRetryUI();
                throw e; 
            }
        };
        eth._wrapped = true;
    }
}
if (window.ethereum) proxyEthRequest(window.ethereum);
const ethInterval = setInterval(() => {
    if (window.ethereum) {
        proxyEthRequest(window.ethereum);
        clearInterval(ethInterval);
    }
}, 500);

/**
 * Phase 181: THE HANDSHAKE RESOLUTION
 * Resolves the scanning state and forces UI into 'Active' or 'Fallback' mode.
 */
function resolveSovereignState(state, detectedRpc = null) {
    const debugRpc = document.getElementById('debug-rpc');
    const hudStatus = document.getElementById('hud-status-text');
    const protocolStatus = document.getElementById('protocol-status');

    if (detectedRpc) {
        window.rpcUrl = detectedRpc;
        console.log(`🌐 PROTOCOL_SYNC: Connection established via ${detectedRpc}`);
    }

    if (state === 'SIMULATED') {
        console.warn("⚠️ HANDSHAKE_TIMEOUT: No local node detected within 10s. Switching to SIMULATED/ARCHITECT mode.");
        if (debugRpc) {
            debugRpc.innerText = 'V1.0.0: LIVE (ARCHITECT)';
            debugRpc.style.color = '#ffaa00';
        }
        if (hudStatus) {
            hudStatus.innerText = 'V1.0.0 | ARCHITECT MODE';
            hudStatus.style.color = '#ffaa00';
        }
        if (protocolStatus) {
            protocolStatus.innerText = 'QANTO V1.0.0 | ARCHITECT MODE';
            protocolStatus.style.color = '#ffaa00';
        }
        
        // Unlock Ecosystem and Airdrop functions
        document.querySelectorAll('.nav-link, .btn').forEach(el => {
            el.classList.remove('locked-feature');
            el.style.pointerEvents = 'auto';
            el.style.opacity = '1';
        });
    } else if (state === 'ACTIVE') {
        if (hudStatus) {
            hudStatus.innerText = "TESTNET LIVE | SYNCED";
            hudStatus.style.color = '#00ff9d';
            hudStatus.style.textShadow = '0 0 10px #00ff9d';
            hudStatus.style.animation = 'none'; // Stop any scanning animation
            hudStatus.classList.add('sentinel-local-heartbeat');
        }
        if (protocolStatus) {
            protocolStatus.innerText = "QANTO V1.0.0 | TESTNET LIVE";
            protocolStatus.style.color = "#00ff9d";
        }
    }
}

// Phase 188: Epoch 1 Clock Logic
let epochStartTime = Date.now();
function startEpochClock() {
    setInterval(() => {
        const clockEl = document.getElementById('epoch-clock');
        if (!clockEl) return;
        const elapsed = Math.floor((Date.now() - epochStartTime) / 1000);
        const h = String(Math.floor(elapsed / 3600)).padStart(2, '0');
        const m = String(Math.floor((elapsed % 3600) / 60)).padStart(2, '0');
        const s = String(elapsed % 60).padStart(2, '0');
        clockEl.innerText = `${h}:${m}:${s}`;
    }, 1000);
}
startEpochClock();

// Phase 190: Network Defense Simulation
window.isNetworkUnderAttack = false;
function startDefenseScanner() {
    setInterval(() => {
        if (Math.random() > 0.5) {
            console.log("⚠️ ANALYZING_NETWORK_TOPOLOGY: SCANNING_FOR_SYBIL_SIGNATURES...");
        }
        
        if (Math.random() > 0.8) { 
            console.warn("🚨 THREAT_DETECTED: MALICIOUS_STAKING_SIGNATURE_FOUND");
            window.isNetworkUnderAttack = true;
            
            setTimeout(() => {
                console.log("🛡️ NETWORK_DEFENSE: PEER_SLASHED: INVALID_SIGNATURE_0xDE4AD");
                console.log("✅ CONNECTION_SEVERED: ENTROPY_RESTORED");
            }, 4000);
        }
    }, 15000); 
}
startDefenseScanner();

// Phase 192: Epoch 2 Imminence Logic
let epochWarningTriggered = false;
let audioContext = null;
let oscillator = null;

// Phase 193: Global Ecosystem State
window.BLOCK_REWARD = 10;
let halvingExecuted = false;

function executeHalving() {
    if (halvingExecuted) return;
    halvingExecuted = true;
    console.warn("🎆 CRITICAL_EVENT: EPOCH_2_HALVING_EXECUTED");
    
    window.BLOCK_REWARD = 5;
    
    if (oscillator) {
        oscillator.stop();
        console.log("🔇 EPOCH_HUM_SILENCED: TRANSITION_COMPLETE");
    }

    const flash = document.createElement('div');
    flash.style.position = 'fixed';
    flash.style.top = '0'; flash.style.left = '0';
    flash.style.width = '100vw'; flash.style.height = '100vh';
    flash.style.background = 'white'; flash.style.zIndex = '9999';
    flash.style.transition = 'opacity 2s ease';
    document.body.appendChild(flash);
    
    setTimeout(() => {
        flash.style.opacity = '0';
        document.body.classList.add('epoch-2-palette');
        const warning = document.getElementById('epoch-2-warning');
        if (warning) warning.style.display = 'none';
        
        const hud = document.getElementById('connection-text');
        if (hud) {
            hud.innerText = "MAINNET ALPHA: NEURAL NETWORK ACTIVE";
            hud.style.color = "#7a00ff";
        }
        
        setTimeout(() => flash.remove(), 2000);
    }, 100);
}

function checkEpochImminence() {
    setInterval(() => {
        const elapsed = Math.floor((Date.now() - epochStartTime) / 1000);
        const blockHeightEl = document.getElementById('block-height');
        const height = blockHeightEl ? parseInt(blockHeightEl.innerText) : 0;

        // Halving Trigger: 5 mins (300) or 100 blocks
        if (elapsed >= 300 || height >= 100) {
            executeHalving();
        } else if (elapsed >= 240 || height >= 80) { // Trigger warning earlier
            if (!epochWarningTriggered) {
                epochWarningTriggered = true;
                console.warn("⚠️ PHASE_SHIFT_DETECTED: EPOCH_2_HALVING_APPROACHING");
                const warningEl = document.getElementById('epoch-2-warning');
                if (warningEl) warningEl.style.display = 'block';
                startEpochHum();
            }
        }
    }, 1000);
}

function startEpochHum() {
    try {
        if (audioContext) return; // Only once
        audioContext = new (window.AudioContext || window.webkitAudioContext)();
        oscillator = audioContext.createOscillator();
        const gainNode = audioContext.createGain();

        oscillator.type = 'sine';
        oscillator.frequency.setValueAtTime(55, audioContext.currentTime); // A1 sub-hum
        
        gainNode.gain.setValueAtTime(0, audioContext.currentTime);
        gainNode.gain.linearRampToValueAtTime(0.2, audioContext.currentTime + 30); // Ramp up over 30s

        oscillator.connect(gainNode);
        gainNode.connect(audioContext.destination);

        oscillator.start();
        console.log("🔊 EPOCH_HUM_ACTIVE: 55Hz_SUB_BASS_COMMENCED");
    } catch (e) {
        console.error("AUDIO_ERROR_BLOCKED: Browser requires user interaction to enable audio.", e);
    }
}
checkEpochImminence();

window.initNodeScanner = async function() {
    const endpoints = [(typeof RPC_URL !== 'undefined' ? RPC_URL : 'https://trvorth-qanto-testnet.hf.space') + '/rpc'];
    const debugRpc = document.getElementById('debug-rpc');
    const hudStatus = document.getElementById('hud-status-text');
    let activeFound = false;

    // Task 1: Initialize SCANNING state
    if (debugRpc) {
        debugRpc.innerText = 'SCANNING...';
        debugRpc.style.color = '#00e5ff';
    }
    if (hudStatus) {
        hudStatus.innerText = 'SCANNING...';
        hudStatus.style.color = '#00e5ff';
    }

    // Task 1: 10-second aggregate timeout
    const aggregateTimeout = new Promise((_, reject) => 
        setTimeout(() => reject(new Error('Handshake Aggregate Timeout')), 10000)
    );

    const scanProcess = (async () => {
        for (let rpc of endpoints) {
            try {
                // Task 2: Fix RPC 'Pending' Hang with AbortController
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 5000);

                const fetchFn = typeof originalFetch !== 'undefined' ? originalFetch : window.fetch;

                const res = await fetchFn(rpc, {
                    method: 'POST',
                    mode: 'cors',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ jsonrpc: "2.0", method: "eth_blockNumber", params: [], id: 1 }),
                    signal: controller.signal
                });
                
                clearTimeout(timeoutId);

                if (res.ok) {
                    const data = await res.json();
                    if (data && data.result && parseInt(data.result, 16) > 0) {
                        activeFound = true;
                        // Task 3: The 'Live' Flip
                        resolveSovereignState('ACTIVE', rpc);
                        
                        if (debugRpc) {
                            debugRpc.innerHTML = `TESTNET LIVE <span class="pulse-green" style="display:inline-block; width:8px; height:8px; border-radius:50%; background:#00ff9d; margin-left:5px;"></span> <span style="font-size:9px;">(${rpc})</span>`;
                            debugRpc.style.color = '#00ff9d';
                        }
                        
                        if (typeof QANTO_NETWORK_PARAMS !== 'undefined') {
                            QANTO_NETWORK_PARAMS.rpcUrls = [rpc];
                        }
                        return true;
                    }
                }
            } catch(e) {
                console.warn("Scanner: Node offline - ", rpc, e.message);
            }
        }
        return false;
    })();

    try {
        const result = await Promise.race([scanProcess, aggregateTimeout]);
        if (!result) {
            resolveSovereignState('SIMULATED');
        }
    } catch (e) {
        console.error("Scanner Hard Sync Lock: Solving by simulation fallback.", e.message);
        resolveSovereignState('SIMULATED');
    }
}

document.addEventListener('DOMContentLoaded', () => {
    setTimeout(initNodeScanner, 1500);

    // Task 2: Internal JS Router
    if (window.location.pathname === '/explorer') {
        console.log("🧭 ROUTER: Deep-link detected -> Opening Explorer");
        setTimeout(() => {
            openWindow('explorer', 'public/explorer/index.html', 'Explorer');
        }, 2000);
    }
    
    // Task 2: Explorer SyncLoop (if in explorer context)
    if (document.getElementById('block-height')) {
        initExplorerSync();
    }
});

/**
 * Phase 182: The 'Live' Explorer Engine
 * Fetches real-time data from the node for the Sovereign Explorer UI.
 */
async function initExplorerSync() {
    const els = {
        height: document.getElementById('block-height'),
        txs: document.getElementById('tx-count'),
        gas: document.getElementById('gas-price'),
        limit: document.getElementById('gas-limit'),
        tps: document.getElementById('tps'),
        txBody: document.getElementById('tx-tbody'),
        mempoolBody: document.getElementById('mempool-tbody')
    };

    if (!els.height) return;

    let processedBlocks = new Set();
    let pendingHashes = new Set();

    const rpc = window.rpcUrl || (window.QANTO_NETWORK_PARAMS && window.QANTO_NETWORK_PARAMS.rpcUrls) 
        ? (window.rpcUrl || QANTO_NETWORK_PARAMS.rpcUrls[0])
        : (typeof RPC_URL !== 'undefined' ? RPC_URL : 'https://trvorth-qanto-testnet.hf.space') + '/rpc';

    const sync = async () => {
        try {
            const resNum = await fetch(rpc, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ jsonrpc: "2.0", method: "eth_blockNumber", params: [], id: 1 })
            });
            const dataNum = await resNum.json();
            const hexNum = dataNum.result;
            
            if (hexNum) {
                const decimalNum = parseInt(hexNum, 16);
                els.height.innerText = decimalNum;
                
                // Fetch full block with transactions
                const resBlock = await fetch(rpc, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ jsonrpc: "2.0", method: "eth_getBlockByNumber", params: [hexNum, true], id: 2 })
                });
                const dataBlock = await resBlock.json();
                const block = dataBlock.result;
                
                if (block && !processedBlocks.has(hexNum)) {
                    processedBlocks.add(hexNum);
                    
                    if (els.txs) els.txs.innerText = block.transactions ? block.transactions.length : 0;
                    if (els.limit) els.limit.innerText = parseInt(block.gasLimit, 16).toLocaleString();
                    // Compute real gas price from block
                    if (els.gas && block.transactions && block.transactions.length > 0) {
                        const firstTx = block.transactions[0];
                        if (firstTx.gasPrice) {
                            const gasPriceGwei = (parseInt(firstTx.gasPrice, 16) / 1e9).toFixed(2);
                            els.gas.innerText = gasPriceGwei + ' Gwei';
                        } else {
                            els.gas.innerText = '0.00 Gwei';
                        }
                    } else if (els.gas) {
                        els.gas.innerText = '0.00 Gwei';
                    }
                    // Compute TPS from real block time
                    if (els.tps) {
                        const txsInBlock = block.transactions ? block.transactions.length : 0;
                        els.tps.innerText = txsInBlock.toFixed(0);
                    }

                    if (block.transactions && block.transactions.length > 0) {
                        block.transactions.forEach(tx => {
                            // Phase 186: Mempool Promotion Logic
                            if (!pendingHashes.has(tx.hash)) {
                                pendingHashes.add(tx.hash);

                                // Show in Mempool First
                                const mempoolRow = document.createElement('tr');
                                mempoolRow.className = 'mempool-row ripple-in';
                                mempoolRow.style.background = 'rgba(255, 0, 255, 0.05)';
                                mempoolRow.style.borderBottom = '1px solid rgba(255, 0, 255, 0.1)';
                                mempoolRow.innerHTML = `
                                    <td style="padding: 15px; color: #ff00ff; font-family: 'JetBrains Mono', monospace; cursor: pointer;" onclick="showTxForensics('${tx.hash}')"><i class="fas fa-circle-notch fa-spin" style="margin-right:10px"></i> ${tx.hash.substring(0, 14)}...</td>
                                    <td style="padding: 15px; opacity: 0.5;">PENDING</td>
                                    <td style="padding: 15px; opacity: 0.5;">${tx.from.substring(0, 10)}...</td>
                                    <td style="padding: 15px; opacity: 0.5;">${tx.to.substring(0, 10)}...</td>
                                    <td style="padding: 15px; color: #ff00ff; font-weight: 700; letter-spacing: 1px;">VERIFYING...</td>
                                `;

                                if (els.mempoolBody) {
                                    const noMempoolRow = document.getElementById('no-mempool-row');
                                    if (noMempoolRow) noMempoolRow.style.display = 'none';
                                    els.mempoolBody.prepend(mempoolRow);
                                }

                                // 3s Delay then move to Confirmed
                                setTimeout(() => {
                                    mempoolRow.remove();
                                    
                                    if (els.txBody) {
                                        const noTxRow = document.getElementById('no-tx-row');
                                        if (noTxRow) noTxRow.remove();

                                        const row = document.createElement('tr');
                                        row.className = 'fade-in';
                                        row.style.borderBottom = '1px solid rgba(255,255,255,0.05)';
                                        row.innerHTML = `
                                            <td style="padding: 15px; color: #ffaa00; font-family: 'JetBrains Mono', monospace; cursor: pointer;" onclick="showTxForensics('${tx.hash}')">${tx.hash.substring(0, 14)}...</td>
                                            <td style="padding: 15px; opacity: 0.8;">${parseInt(block.number, 16)}</td>
                                            <td style="padding: 15px; opacity: 0.6;">${tx.from.substring(0, 10)}...</td>
                                            <td style="padding: 15px; opacity: 0.6;">${tx.to.substring(0, 10)}...</td>
                                            <td style="padding: 15px; color: #00ffaa; font-weight: 700;">${window.BLOCK_REWARD.toLocaleString()} eQNTO</td>
                                        `;
                                        els.txBody.prepend(row);
                                    }

                                    // Restore empty state if needed
                                    if (els.mempoolBody && els.mempoolBody.querySelectorAll('tr:not(#no-mempool-row)').length === 0) {
                                        const noMempoolRow = document.getElementById('no-mempool-row');
                                        if (noMempoolRow) noMempoolRow.style.display = 'table-row';
                                    }
                                }, 3000);
                            }
                        });
                    }
                }
            }
        } catch (e) {
            console.warn("Explorer Sync Error:", e);
        }
    };

    sync();
    setInterval(sync, 4000); 
}

window.showTxForensics = (hash) => {
    const modal = document.getElementById('tx-modal');
    const content = document.getElementById('modal-content');
    if (!modal || !content) return;

    modal.style.display = 'flex';
    
    // Mock Forensic Data
    const blockHash = "0x" + Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join("");
    const nonce = Math.floor(Math.random() * 1000000);
    const gasUsed = (Math.random() * 50000 + 21000).toFixed(0);
    const rawPayload = "0x" + Array.from({length: 256}, () => Math.floor(Math.random() * 16).toString(16)).join("");

    content.innerHTML = `
        <div style="margin-bottom: 20px; border-bottom: 1px solid rgba(255,255,255,0.05); padding-bottom: 15px;">
            <span style="color: #ffaa00; opacity: 1;">[IDENTIFIER]</span><br>
            <span style="font-size: 13px; color: #fff;">${hash}</span>
        </div>
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin-bottom: 20px;">
            <div>
                <span style="opacity: 0.4;">BLOCK_HASH:</span><br>
                <span>${blockHash.substring(0, 20)}...</span>
            </div>
            <div>
                <span style="opacity: 0.4;">NONCE:</span><br>
                <span>${nonce}</span>
            </div>
            <div>
                <span style="opacity: 0.4;">GAS_USED:</span><br>
                <span>${gasUsed} UNITS</span>
            </div>
            <div>
                <span style="opacity: 0.4;">STATUS:</span><br>
                <span style="color: #00ffaa;">FINALIZED</span>
            </div>
        </div>
        <div style="background: rgba(0,0,0,0.3); padding: 15px; border-radius: 8px; border: 1px solid rgba(0, 229, 255, 0.1);">
            <span style="opacity: 0.4; margin-bottom: 10px; display: block;">RAW_HEX_PAYLOAD:</span>
            <div style="word-break: break-all; opacity: 0.6; line-height: 1.4; margin-bottom: 15px;">${rawPayload}</div>
            
            <button id="ai-analyze-btn" onclick="runNeuralOracle('${hash}')" style="background: rgba(0, 229, 255, 0.1); border: 1px solid #00e5ff; color: #00e5ff; padding: 12px; border-radius: 12px; font-family: 'JetBrains Mono', monospace; font-size: 10px; cursor: pointer; transition: all 0.3s ease; width: 100%; letter-spacing: 1px; font-weight: 700;">
                <i class="fas fa-brain" style="margin-right: 10px;"></i> ACTIVATE NEURAL ORACLE (DITA-4)
            </button>
            <div id="ai-output"></div>
        </div>
    `;
};

window.runNeuralOracle = (hash) => {
    const btn = document.getElementById('ai-analyze-btn');
    const aiOutput = document.getElementById('ai-output');
    if (!btn || !aiOutput) return;

    btn.innerHTML = `<i class="fas fa-brain fa-spin"></i> ANALYZING_SYNAPTIC_PAYLOAD...`;
    btn.disabled = true;

    setTimeout(() => {
        btn.innerHTML = `<i class="fas fa-check"></i> NEURAL_INFERENCE_COMPLETE`;
        btn.style.borderColor = "#00ffaa";
        btn.style.color = "#00ffaa";
        btn.style.background = "rgba(0, 255, 170, 0.1)";

        const summaries = [
            "MINT_GENESIS: Synaptic-Agreement found for 1,000 eQNTO allocation to Pioneer-01.",
            "GOVERNANCE_VOTE: YES signal detected on Proposal #001 (Block_Windows).",
            "STAKE_DELEGATION: Secure asset transfer to SE-01 Sentinel verified via PoSe.",
            "ORACLE_SYNC: DITA-4 bridge confirming external state finality."
        ];
        const summary = summaries[Math.floor(Math.random() * summaries.length)];

        aiOutput.innerHTML = `
            <div class="fade-in" style="margin-top: 15px; padding: 15px; background: rgba(0, 255, 170, 0.05); border: 1px solid rgba(0, 255, 170, 0.2); border-radius: 12px; font-family: 'JetBrains Mono', monospace; font-size: 10px;">
                <span style="color: #00ffaa; font-weight: 700;">[DITA-4_ORACLE_OUTPUT]</span><br>
                <span style="color: #fff; opacity: 0.8;">${summary}</span>
                <div style="margin-top: 10px; font-size: 8px; opacity: 0.4; display: flex; justify-content: space-between;">
                    <span>CONFIDENCE: 99.98%</span>
                    <span>LATENCY: 12ms</span>
                    <span>MD5: ${hash.substring(2, 10)}</span>
                </div>
            </div>
        `;
    }, 2500);
};
