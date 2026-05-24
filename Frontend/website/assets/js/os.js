// MINIMAL, PARASITE-FREE QANTO OS
import { connectWallet } from "./web3-provider.js";

document.addEventListener("DOMContentLoaded", () => {
    
    // 1. Safe RPC Heartbeat (15s timeout)
    async function checkRPC() {
        try {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), 15000);
            // Ping the HF Testnet
            await fetch('https://trvorth-qanto-testnet.hf.space/rpc', { signal: controller.signal });
            clearTimeout(timeoutId);
        } catch(e) {
            console.warn("QANTO RPC Offline or Syncing...");
        }
        setTimeout(checkRPC, 15000);
    }
    checkRPC();

    // 2. MetaMask Network Addition / Connection
    const addNetBtn = document.getElementById('btn-add-network');
    if (addNetBtn) {
        addNetBtn.addEventListener('click', async (e) => {
            e.preventDefault();
            const connection = await connectWallet();
            if (connection) {
                const shortAddr = `${connection.address.substring(0, 6)}...${connection.address.substring(38)}`;
                addNetBtn.innerText = `${shortAddr} (${parseFloat(connection.balance).toFixed(2)} QNTO)`;
                addNetBtn.classList.remove('btn-primary');
                addNetBtn.classList.add('btn-success');
            }
        });
    }
});
