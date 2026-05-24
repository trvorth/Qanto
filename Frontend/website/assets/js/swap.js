import { connectWallet } from "./web3-provider.js";

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
            // Interactive signature request
            const msgParams = JSON.stringify({
                domain: {
                    chainId: 1234,
                    name: 'QantoSwap',
                    version: '1',
                },
                message: {
                    from: this.userAddress,
                    to: "0x5FbDB2315678afecb367f032d93F642f64180aa3",
                    value: amount,
                },
                primaryType: 'Swap',
                types: {
                    EIP712Domain: [
                        { name: 'name', type: 'string' },
                        { name: 'version', type: 'string' },
                        { name: 'chainId', type: 'uint256' },
                    ],
                    Swap: [
                        { name: 'from', type: 'address' },
                        { name: 'to', type: 'address' },
                        { name: 'value', type: 'string' },
                    ],
                },
            });

            const signature = await window.ethereum.request({
                method: 'eth_signTypedData_v4',
                params: [this.userAddress, msgParams],
            });

            console.log("QantoSwap signature confirmed:", signature);
            alert(`Swap transaction signed successfully!\nSignature: ${signature.substring(0, 16)}...`);
        } catch (err) {
            console.error("User rejected swap or MetaMask error:", err);
            alert("Swap rejected or failed.");
        } finally {
            this.swapButton.disabled = false;
            this.swapButton.innerText = "Swap QNTO";
            this.inputAmount.value = '';
            this.outputAmount.value = '';
        }
    }
}

// Initialize on load
window.addEventListener('DOMContentLoaded', () => {
    window.qantoSwap = new QantoSwap();
});
export default QantoSwap;
