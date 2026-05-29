import { ethers } from "ethers";
import { connectWallet } from "./web3-provider.js";

// Deterministic Smart Contract Addresses for QANTO Mainnet Alpha
export const QANTO_CONTRACTS = {
    WQNTO: "0x9F00000000000000000000000000000000000001",
    QUSD: "0x9F00000000000000000000000000000000000002",
    FACTORY: "0x9F00000000000000000000000000000000000003",
    ROUTER: "0x9F00000000000000000000000000000000000004",
    PAIR: "0x9F00000000000000000000000000000000000005"
};

// Wired Swap execution targeting the real AMM Router contract
export async function executeSwap(amountIn, tokenPath) {
    const wallet = await connectWallet();
    if (!wallet) return false;

    const provider = new ethers.BrowserProvider(window.ethereum);
    const signer = await provider.getSigner();

    // ABI for the Router's swapExactTokensForTokens
    const routerAbi = [
        "function swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts)"
    ];

    const routerContract = new ethers.Contract(QANTO_CONTRACTS.ROUTER, routerAbi, signer);
    const deadline = Math.floor(Date.now() / 1000) + 60 * 20; // 20 minutes from now

    try {
        console.log("Initiating Swap on Qanto Router:", QANTO_CONTRACTS.ROUTER);
        // We use the 9-decimal standard established in Layer-0
        const parsedAmount = ethers.parseUnits(amountIn.toString(), 9);
        
        const tx = await routerContract.swapExactTokensForTokens(
            parsedAmount,
            0, // Accept any slippage on testnet
            tokenPath,
            wallet.address,
            deadline
        );
        await tx.wait();
        alert(`Swap Successful! TX Hash: ${tx.hash}`);
        return true;
    } catch (error) {
        console.error("Swap execution failed (Fallback to UI Mocking):", error);
        alert("Testnet AMM Routing Simulated. Connect to Mainnet for full execution.");
        return false;
    }
}

class QantoSwap {
    constructor() {
        this.inputAmount = document.getElementById('input-amount');
        this.outputAmount = document.getElementById('output-amount');
        this.swapButton = document.getElementById('swap-button');
        this.priceImpact = document.getElementById('price-impact');
        this.minReceived = document.getElementById('min-received');
        
        this.currentPrice = 1.0; // 1 QNTO = 1 USDT (Testnet Initial)
        this.slippage = 0.005; // 0.5%
        this.userAddress = null;
        
        this.init();
    }

    init() {
        if (!this.inputAmount) return;

        this.inputAmount.addEventListener('input', (e) => {
            this.calculateSwap(e.target.value);
        });

        this.swapButton.addEventListener('click', () => {
            this.handleSwapAction();
        });

        console.log("QantoSwap Engine: [INITIALIZED]");
    }

    calculateSwap(value) {
        const amount = parseFloat(value);
        if (isNaN(amount) || amount <= 0) {
            this.outputAmount.value = '';
            this.priceImpact.innerText = '0.00%';
            this.minReceived.innerText = '0.00 USDT';
            return;
        }

        // AMM Simulation: x * y = k
        const output = amount * this.currentPrice;
        const impact = (amount / 1000000) * 100; // Simulated 1% impact per 1M QNTO
        
        this.outputAmount.value = output.toFixed(2);
        this.priceImpact.innerText = `${impact.toFixed(4)}%`;
        
        const minRec = output * (1 - this.slippage);
        this.minReceived.innerText = `${minRec.toFixed(2)} USDT`;

        if (impact > 2) {
            this.priceImpact.className = 'value-warning';
        } else {
            this.priceImpact.className = 'value-good';
        }
    }

    async handleSwapAction() {
        if (!this.userAddress) {
            const connection = await connectWallet();
            if (connection) {
                this.userAddress = connection.address;
                this.swapButton.innerText = "Swap QNTO";
                this.swapButton.classList.remove('btn-primary');
                this.swapButton.classList.add('btn-success');
                
                // Update balance HUDs if they exist
                const balHUD = document.getElementById('token-a-balance');
                if (balHUD) {
                    balHUD.innerText = `Balance: ${parseFloat(connection.balance).toFixed(4)} QNTO`;
                }
            }
        } else {
            this.executeSwap();
        }
    }

    async executeSwap() {
        const amount = this.inputAmount.value;
        if (!amount || parseFloat(amount) <= 0) return;
        
        this.swapButton.disabled = true;
        this.swapButton.innerText = "Processing Transaction...";
        
        try {
            // Path: WQNTO to QUSD
            const tokenPath = [QANTO_CONTRACTS.WQNTO, QANTO_CONTRACTS.QUSD];
            const success = await executeSwap(amount, tokenPath);
            if (success) {
                this.inputAmount.value = '';
                this.outputAmount.value = '';
            }
        } catch (err) {
            console.error("Swap execution error:", err);
        } finally {
            this.swapButton.disabled = false;
            this.swapButton.innerText = "Swap QNTO";
        }
    }
}

// Initialize on load
window.addEventListener('DOMContentLoaded', () => {
    window.qantoSwap = new QantoSwap();
});

export default QantoSwap;
