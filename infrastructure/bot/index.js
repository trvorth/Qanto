const { ethers } = require("ethers");
require("dotenv").config();

/**
 * QANTO Whale Alert & Social Pulse Bot
 * Monitors on-chain events for the community.
 */

const RPC_URL = process.env.RPC_URL || "http://127.0.0.1:8545";
const provider = new ethers.JsonRpcProvider(RPC_URL);

// Contract Addresses
const QantoSwapAddr = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"; // Factory/Pool
const PioneerNFTAddr = "0x0165878A594ca255338adfa4d48449f69242Eb8F";

// ABIs (Minimal for event detection)
const SwapABI = ["event Swap(address indexed sender, uint amount0In, uint amount1In, uint amount0Out, uint amount1Out, address indexed to)"];
const NFTABI = ["event Transfer(address indexed from, address indexed to, uint256 indexed tokenId)"];

async function main() {
  console.log("QANTO WHALE BOT: [INITIALIZED]");
  
  const swapContract = new ethers.Contract(QantoSwapAddr, SwapABI, provider);
  const nftContract = new ethers.Contract(PioneerNFTAddr, NFTABI, provider);

  // 1. Listen for Swap Events
  swapContract.on("Swap", (sender, amount0In, amount1In, amount0Out, amount1Out, to) => {
    const totalOut = amount1Out > 0 ? amount1Out : amount0Out;
    const formattedAmt = ethers.formatEther(totalOut);
    
    if (parseFloat(formattedAmt) > 1000) { // Example threshold
        console.log(`🚨 WHALE ALERT: Large Swap Detected! 🚨`);
        console.log(`Account: ${to} | Received: ${formattedAmt} QNTO`);
        // In production, trigger Webhook (X, Discord, Telegram)
    }
  });

  // 2. Listen for New Pioneer Joined
  nftContract.on("Transfer", (from, to, tokenId) => {
    if (from === ethers.ZeroAddress) {
        console.log(`🌊 NEW PIONEER JOINED THE SAGA! 🌊`);
        console.log(`Pioneer Address: ${to} | TokenID: #${tokenId}`);
        // Post to social feed
    }
  });

  console.log("Monitoring Network Pulse...");
}

main().catch((error) => {
  console.error("Bot Error:", error);
});
