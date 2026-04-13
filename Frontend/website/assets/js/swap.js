/**
 * QantoSwap Logic - Phase 33
 * Handles real-time swap calculations and economic telemetry.
 */

class QantoSwap {
    constructor() {
        this.inputAmount = document.getElementById('input-amount');
        this.outputAmount = document.getElementById('output-amount');
        this.swapButton = document.getElementById('swap-button');
        this.priceImpact = document.getElementById('price-impact');
        this.minReceived = document.getElementById('min-received');
        
        this.currentPrice = 1.0; // 1 QNTO = 1 USDT (Testnet Initial)
        this.slippage = 0.005; // 0.5%
        
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
        // For demonstration, we use a simple linear price with minor impact
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
        if (this.swapButton.innerText === "Connect Wallet") {
            // Trigger global connection from main.js if available
            if (window.ethereum) {
                try {
                    await window.ethereum.request({ method: 'eth_requestAccounts' });
                    this.swapButton.innerText = "Swap QNTO";
                    this.swapButton.classList.remove('btn-primary');
                    this.swapButton.classList.add('btn-success');
                } catch (err) {
                    console.error("User rejected connection");
                }
            } else {
                alert("Please install a Web3 wallet to swap.");
            }
        } else {
            this.executeSwap();
        }
    }

    executeSwap() {
        const amount = this.inputAmount.value;
        this.swapButton.disabled = true;
        this.swapButton.innerText = "Processing Transaction...";
        
        // Simulate transaction delay
        setTimeout(() => {
            alert(`Successfully swapped ${amount} QNTO! \nTransaction: 0x${Math.random().toString(16).slice(2)}...`);
            this.swapButton.disabled = false;
            this.swapButton.innerText = "Swap QNTO";
        }, 2000);
    }
}

// Initialize on load
window.addEventListener('DOMContentLoaded', () => {
    window.qantoSwap = new QantoSwap();
});
