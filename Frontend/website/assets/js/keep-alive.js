/**
 * QANTO Auto-Awakener
 * Pings the Hugging Face container to wake it from sleep state before RPC calls execute.
 */
const HF_SPACE_URL = "https://huggingface.co/spaces/trvorth/qanto-testnet";
const DIRECT_URL = "https://trvorth-qanto-testnet.hf.space";

async function wakeUpNode() {
    try {
        console.log("[Auto-Awakener] Pinging Hugging Face Bootnode...");
        // A simple GET request to the root to trigger container startup
        await fetch(DIRECT_URL, { mode: 'no-cors', cache: 'no-cache' });
        console.log("[Auto-Awakener] Bootnode is awake.");
    } catch (error) {
        console.warn("[Auto-Awakener] Ping executed, waiting for container spin-up.", error);
    }
}

// Run immediately on script load
wakeUpNode();
// Re-ping every 5 minutes to prevent sleep during active user sessions
setInterval(wakeUpNode, 300000);
