const { ethers } = require("hardhat");

async function main() {
    const [deployer] = await ethers.getSigners();
    console.log("Initializing Economic Engine with account:", deployer.address);

    const QANTO_TOKEN_ADDR = "0x5FbDB2315678afecb367f032d93F642f64180aa3"; // Local Hardhat Default or specific
    const SWAP_COORD_ADDR = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"; // QantoSwap Pool

    // 1. Deploy Referral Contract
    console.log("Deploying QantoReferral...");
    const QantoReferral = await ethers.getContractFactory("QantoReferral");
    const referral = await QantoReferral.deploy();
    await referral.deployed();
    console.log("QantoReferral deployed to:", referral.address);

    // 2. Initialize QantoSwap LP
    console.log("Seeding QantoSwap Genesis Pool...");
    const qantoToken = await ethers.getContractAt("QantoToken", QANTO_TOKEN_ADDR);
    const qantoSwap = await ethers.getContractAt("QantoSwap", SWAP_COORD_ADDR);

    const amountQNTO = ethers.utils.parseUnits("500000", 18);
    const amountUSDT = ethers.utils.parseUnits("500000", 6); // Assuming 1:1 for testnet, 6 decimals for USDT

    console.log(`Approving ${amountQNTO} QNTO and ${amountUSDT} USDT...`);
    // Note: Assuming usdtAddress is known from the pool or deployer
    const usdtAddr = await qantoSwap.token1(); // token1 is likely USDT based on deploy.js
    const usdt = await ethers.getContractAt("QantoToken", usdtAddr);

    await qantoToken.approve(SWAP_COORD_ADDR, amountQNTO);
    await usdt.approve(SWAP_COORD_ADDR, amountUSDT);

    console.log("Adding Liquidity...");
    const tx = await qantoSwap.addLiquidity(amountQNTO, amountUSDT);
    await tx.wait();

    console.log("✅ Economic Engine Initialized!");
    console.log("------------------------------");
    console.log("Referral Contract:", referral.address);
    console.log("Pool Seeding: [SUCCESS]");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
