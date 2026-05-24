// MINIMAL, PARASITE-FREE QANTO OS
document.addEventListener("DOMContentLoaded", () => {
    
    // 1. Safe RPC Heartbeat (15s timeout)
    async function checkRPC() {
        try {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), 15000);
            // Ping the HF Testnet
            await fetch('https://trvorth-qanto-testnet.hf.space/rpc', { signal: controller.signal });
            clearTimeout(timeoutId);
            // If we had UI elements to update, we would do it here safely without modals
        } catch(e) {
            console.warn("QANTO RPC Offline or Syncing...");
        }
        setTimeout(checkRPC, 15000);
    }
    checkRPC();

    // 2. MetaMask Network Addition
    const addNetBtn = document.getElementById('btn-add-network');
    if (addNetBtn) {
        addNetBtn.addEventListener('click', async () => {
            if (window.ethereum) {
                try {
                    await window.ethereum.request({
                        method: 'wallet_addEthereumChain',
                        params: [{ 
                            chainId: '0x1234', 
                            chainName: 'QANTO Testnet', 
                            nativeCurrency: { name: 'QANTO', symbol: 'QNTO', decimals: 9 }, 
                            rpcUrls: ['https://trvorth-qanto-testnet.hf.space/rpc'] 
                        }]
                    });
                } catch (error) { console.error("MetaMask Error:", error); }
            } else {
                alert("Please install MetaMask to connect to QANTO Layer-0.");
            }
        });
    }
});
