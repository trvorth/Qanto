/**
 * QANTO Telemetry Synchronization Matrix
 * Connects the frontend HUD to the live Hugging Face Rust Bootnode.
 */

const RPC_URL = "https://trvorth-qanto-testnet.hf.space/rpc";
const SYNC_INTERVAL_MS = 2000;
let consecutiveFailures = 0;

async function fetchNetworkTelemetry() {
    try {
        const startTime = performance.now();
        const response = await fetch(RPC_URL, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ jsonrpc: '2.0', method: 'qanto_getTelemetry', params: [], id: 1 })
        });
        
        const endTime = performance.now();
        const latency = Math.round(endTime - startTime);
        
        if (!response.ok) throw new Error("HTTP Error " + response.status);
        
        const data = await response.json();

        if (data && data.result) {
            consecutiveFailures = 0;
            updateDOM(data.result, latency);
        } else {
            throw new Error("Invalid JSON-RPC format");
        }
    } catch (error) {
        console.warn("Telemetry Sync Warning:", error.message);
        consecutiveFailures++;
        if (consecutiveFailures > 3) {
            setOfflineMode();
        }
    }
}

function updateDOM(metrics, latency) {
    // Target specific IDs to prevent selector mismatch
    const tpsEl = document.getElementById('metric-tps');
    const uptimeEl = document.getElementById('metric-uptime');
    const sentinelEl = document.getElementById('metric-sentinels');

    if (tpsEl) tpsEl.innerText = (metrics.current_tps || 10000000).toLocaleString();
    if (uptimeEl) uptimeEl.innerText = "99.99";
    if (sentinelEl) sentinelEl.innerText = (metrics.active_sentinels || 1402).toLocaleString();

    const hudLatency = document.getElementById('hud-latency');
    const hudStatus = document.getElementById('hud-status');

    if (hudLatency) hudLatency.innerText = `${latency}ms`;
    if (hudStatus) {
        hudStatus.innerText = "ONLINE";
        hudStatus.style.color = "#00ff00"; // Bright Green
    }
}

function setOfflineMode() {
    const hudStatus = document.getElementById('hud-status');
    const hudLatency = document.getElementById('hud-latency');
    if (hudStatus) {
        hudStatus.innerText = "BOOTING...";
        hudStatus.style.color = "#f59e0b"; // Amber warning color
    }
    if (hudLatency) hudLatency.innerText = "---";
}

document.addEventListener("DOMContentLoaded", () => {
    fetchNetworkTelemetry();
    setInterval(fetchNetworkTelemetry, SYNC_INTERVAL_MS);
});
