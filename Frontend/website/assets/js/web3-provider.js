import { ethers } from "ethers";

export const QANTO_NETWORK = {
    chainId: '0x4D2', // Hex for 1234
    chainName: 'QANTO Testnet',
    nativeCurrency: { name: 'QANTO', symbol: 'QNTO', decimals: 9 },
    rpcUrls: ['https://trvorth-qanto-testnet.hf.space/rpc'],
    blockExplorerUrls: ['https://qanto.org/explorer']
};

let provider = null;
let signer = null;

export function getProvider() {
    if (!provider && window.ethereum) {
        provider = new ethers.BrowserProvider(window.ethereum);
    }
    return provider;
}

export async function getSigner() {
    if (!signer && window.ethereum) {
        const prov = getProvider();
        signer = await prov.getSigner();
    }
    return signer;
}

export async function connectWallet() {
    if (!window.ethereum) {
        alert("MetaMask is required to access the QANTO Agentic Mesh.");
        return null;
    }
    try {
        provider = new ethers.BrowserProvider(window.ethereum);
        await provider.send("eth_requestAccounts", []);
        signer = await provider.getSigner();
        
        // Force Network Switch
        let network = await provider.getNetwork();
        if (network.chainId !== 1234n) {
            try {
                await window.ethereum.request({
                    method: 'wallet_switchEthereumChain',
                    params: [{ chainId: QANTO_NETWORK.chainId }],
                });
            } catch (switchError) {
                // This error code indicates that the chain has not been added to MetaMask.
                if (switchError.code === 4902) {
                    await window.ethereum.request({
                        method: 'wallet_addEthereumChain',
                        params: [QANTO_NETWORK],
                    });
                } else {
                    throw switchError;
                }
            }
            // Reinitialize provider and signer after chain change
            provider = new ethers.BrowserProvider(window.ethereum);
            signer = await provider.getSigner();
        }
        
        const address = await signer.getAddress();
        const balance = await provider.getBalance(address);
        return { 
            address, 
            balance: ethers.formatUnits(balance, 9) 
        };
    } catch (error) {
        console.error("Connection Failed:", error);
        return null;
    }
}
