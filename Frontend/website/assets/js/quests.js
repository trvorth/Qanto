/**
 * Qanto Pioneer Quests - On-Chain Logic
 * Phase 32: Viral Growth & Quest Integration
 */

const PIONEER_NFT_ADDRESS = "0x0165878A594ca255338adfa4d48449f69242Eb8F";
const MIN_BALANCE_ETH = 0.05;

// Pioneer NFT ABI (Selected methods)
const PIONEER_ABI = [
    "function hasMinted(address user) view returns (bool)",
    "function claimBadge() external",
    "function points(address user) view returns (uint256)"
];

class QuestManager {
    constructor() {
        this.provider = null;
        this.signer = null;
        this.userAddress = null;
        this.currentStep = 1;
        
        this.init();
    }

    async init() {
        // UI Elements
        this.btnConnect = document.getElementById('btn-connect');
        this.btnVerifyBalance = document.getElementById('btn-verify-balance');
        this.btnMint = document.getElementById('btn-mint');
        this.progressBar = document.getElementById('quest-progress-bar');
        this.completionText = document.getElementById('completion-text');
        this.rankText = document.getElementById('rank-text');

        // Event Listeners
        this.btnConnect.addEventListener('click', () => this.connectWallet());
        this.btnVerifyBalance.addEventListener('click', () => this.verifyBalance());
        this.btnMint.addEventListener('click', () => this.mintNFT());

        // Check if already connected
        if (window.ethereum) {
            this.provider = new ethers.BrowserProvider(window.ethereum);
            const accounts = await this.provider.listAccounts();
            if (accounts.length > 0) {
                this.handleConnection(accounts[0].address);
            }
        }
    }

    async connectWallet() {
        if (!window.ethereum) {
            alert('Please install MetaMask to join the Pioneer Program.');
            return;
        }

        try {
            const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
            this.handleConnection(accounts[0]);
        } catch (err) {
            console.error('Connection failed:', err);
        }
    }

    async handleConnection(address) {
        this.userAddress = address;
        this.signer = await this.provider.getSigner();
        
        // Update UI for Quest 1
        this.markStepCompleted(1);
        this.btnConnect.innerText = `${address.slice(0, 6)}...${address.slice(-4)}`;
        this.btnConnect.classList.remove('btn-primary');
        this.btnConnect.classList.add('btn-success');
        
        // Unlock Quest 2
        this.btnVerifyBalance.disabled = false;
        this.updateProgress(33);
        this.rankText.innerText = "RANK: RECRUIT";
        
        // Auto-check Quest 3 (Check if already minted)
        this.checkAlreadyMinted();
    }

    async verifyBalance() {
        try {
            this.btnVerifyBalance.innerText = "Checking...";
            const balance = await this.provider.getBalance(this.userAddress);
            const balanceEth = parseFloat(ethers.formatEther(balance));

            if (balanceEth >= MIN_BALANCE_ETH) {
                this.markStepCompleted(2);
                this.btnVerifyBalance.innerText = "Balance Verified";
                this.btnVerifyBalance.classList.remove('btn-primary');
                this.btnVerifyBalance.classList.add('btn-success');
                this.btnVerifyBalance.disabled = true;
                
                // Unlock Quest 3
                this.btnMint.disabled = false;
                this.updateProgress(66);
                this.rankText.innerText = "RANK: ADVOCATE";
            } else {
                alert(`Insufficient Balance: Found ${balanceEth.toFixed(4)} ETH. Required: ${MIN_BALANCE_ETH} ETH for Gas Integrity Audit.`);
                this.btnVerifyBalance.innerText = "Verify Balance";
            }
        } catch (err) {
            console.error('Balance check failed:', err);
            this.btnVerifyBalance.innerText = "Verify Balance";
        }
    }

    async checkAlreadyMinted() {
        if (!this.userAddress) return;
        
        try {
            const contract = new ethers.Contract(PIONEER_NFT_ADDRESS, PIONEER_ABI, this.provider);
            const hasMinted = await contract.hasMinted(this.userAddress);
            
            if (hasMinted) {
                this.markStepCompleted(2); // Assume balance verified if minted
                this.markStepCompleted(3);
                this.btnMint.innerText = "Certificate Claimed";
                this.btnMint.classList.remove('btn-primary');
                this.btnMint.classList.add('btn-success');
                this.btnMint.disabled = true;
                this.updateProgress(100);
                this.rankText.innerText = "RANK: PIONEER";
            }
        } catch (err) {
            console.warn('Mint check failed (Contract might not be deployed yet):', err);
        }
    }

    async mintNFT() {
        try {
            this.btnMint.innerText = "Initializing Neural Mint...";
            const contract = new ethers.Contract(PIONEER_NFT_ADDRESS, PIONEER_ABI, this.signer);
            
            const tx = await contract.claimBadge();
            this.btnMint.innerText = "Propagating TX...";
            
            await tx.wait();
            
            this.markStepCompleted(3);
            this.btnMint.innerText = "Certificate Claimed";
            this.btnMint.classList.remove('btn-primary');
            this.btnMint.classList.add('btn-success');
            this.btnMint.disabled = true;
            this.updateProgress(100);
            this.rankText.innerText = "RANK: PIONEER";
            
            showConfetti(); // Optional flair
        } catch (err) {
            console.error('Minting failed:', err);
            this.btnMint.innerText = "Mint NFT";
            alert('Minting failed. Ensure you have 3000+ Quest Points or proper gas.');
        }
    }

    markStepCompleted(stepNum) {
        const card = document.getElementById(`quest-${stepNum}`);
        card.classList.add('completed');
        
        const lockIcon = card.querySelector('.lock-icon');
        const checkIcon = card.querySelector('.check-icon');
        
        if (lockIcon) lockIcon.style.display = 'none';
        if (checkIcon) checkIcon.style.display = 'block';
    }

    updateProgress(percent) {
        this.progressBar.style.width = `${percent}%`;
        this.completionText.innerText = `${percent}% COMPLETE`;
    }
}

// Initial flair
function showConfetti() {
    console.log("Pioneer Confetti Releasing...");
    // Future flair injection
}

// Initialize on load
document.addEventListener('DOMContentLoaded', () => {
    window.questManager = new QuestManager();
});
