import { ethers } from "ethers";
import { connectWallet, getSigner, getProvider } from "./web3-provider.js";

const WQNTO_ADDRESS = "0x9F00000000000000000000000000000000000001"; // Deterministic Public Testnet Alpha Address

document.addEventListener("DOMContentLoaded", () => {
    const payInput = document.getElementById('pay-input');
    const receiveInput = document.getElementById('receive-input');
    const swapBtn = document.getElementById('btn-swap');
    const flipBtn = document.getElementById('btn-flip');
    const toast = document.getElementById('tx-toast');
    const balanceA = document.querySelector('.token-box:first-of-type .token-box-header span:last-of-type');
    const balanceB = document.querySelector('.token-box:last-of-type .token-box-header span:last-of-type');

    let userAddress = null;
    let userBalance = "0.00";
    let wqntoBalance = "0.00";

    // Update the button state based on connection and input
    function updateButtonState() {
        if (!userAddress) {
            swapBtn.innerText = 'Connect Wallet';
            swapBtn.disabled = false;
            return;
        }

        const val = parseFloat(payInput.value);
        if (isNaN(val) || val <= 0) {
            swapBtn.innerText = 'Enter an amount';
            swapBtn.disabled = true;
        } else if (val > parseFloat(userBalance)) {
            swapBtn.innerText = 'Insufficient QNTO balance';
            swapBtn.disabled = true;
        } else {
            swapBtn.innerText = 'Swap';
            swapBtn.disabled = false;
        }
    }

    // Refresh balances from the chain
    async function refreshBalances() {
        if (!userAddress) return;
        try {
            const provider = getProvider();
            const bal = await provider.getBalance(userAddress);
            userBalance = ethers.formatUnits(bal, 9);
            balanceA.innerText = `Balance: ${parseFloat(userBalance).toFixed(4)} QNTO`;

            // Try to get WQNTO balance
            try {
                const wqntoContract = new ethers.Contract(
                    WQNTO_ADDRESS,
                    ["function balanceOf(address) view returns (uint256)"],
                    provider
                );
                const wqntoBal = await wqntoContract.balanceOf(userAddress);
                wqntoBalance = ethers.formatUnits(wqntoBal, 9);
            } catch (e) {
                // Fallback if not deployed
                wqntoBalance = "0.00";
            }
            balanceB.innerText = `Balance: ${parseFloat(wqntoBalance).toFixed(4)} WQNTO`;
        } catch (e) {
            console.error("Failed to refresh balances:", e);
        }
    }

    payInput.addEventListener('input', () => {
        const val = parseFloat(payInput.value);
        if (val > 0) {
            receiveInput.value = val.toFixed(4);
        } else {
            receiveInput.value = '';
        }
        updateButtonState();
    });

    swapBtn.addEventListener('click', async () => {
        if (!userAddress) {
            // Trigger connection
            const connection = await connectWallet();
            if (connection) {
                userAddress = connection.address;
                userBalance = connection.balance;
                await refreshBalances();
                updateButtonState();
            }
            return;
        }

        const amount = payInput.value;
        swapBtn.disabled = true;
        swapBtn.innerText = 'Confirming Transaction...';
        payInput.disabled = true;

        try {
            const signer = await getSigner();
            const provider = getProvider();
            
            console.log(`Attempting real transaction to WQNTO contract at ${WQNTO_ADDRESS}`);
            
            // Check if contract exists (bytecode at address is not 0x)
            const code = await provider.getCode(WQNTO_ADDRESS);
            
            if (code === '0x' || code === '0x00') {
                throw new Error("ContractNotDeployed");
            }

            // Send real transaction sending native QNTO to WQNTO contract
            const tx = await signer.sendTransaction({
                to: WQNTO_ADDRESS,
                value: ethers.parseUnits(amount, 9)
            });

            const receipt = await tx.wait();
            console.log("Transaction confirmed:", receipt);
            showToast(`✅ Swap Successful! TxHash: ${receipt.hash.substring(0, 8)}...${receipt.hash.substring(58)}`);
        } catch (error) {
            console.error("Real swap transaction failed:", error);
            showToast(`❌ Swap failed: ${error.message || 'contract unavailable or transaction rejected'}`);
        } finally {
            payInput.disabled = false;
            payInput.value = '';
            receiveInput.value = '';
            await refreshBalances();
            updateButtonState();
        }
    });

    function showToast(message) {
        toast.innerHTML = message;
        toast.style.display = 'block';
        setTimeout(() => { toast.style.display = 'none'; }, 5000);
    }

    // Rotate arrow on click
    let rotated = false;
    flipBtn.addEventListener('click', () => {
        rotated = !rotated;
        flipBtn.style.transform = rotated ? 'rotate(180deg)' : 'rotate(0deg)';
    });

    updateButtonState();
});
