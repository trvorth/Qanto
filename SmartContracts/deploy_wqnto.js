/**
 * WQNTO DEX Deployment Script
 * Deploys the 6-decimal Wrapped QANTO (WQNTO) contract to EVM-compatible networks.
 *
 * Prerequisites:
 *   npm install ethers dotenv
 *
 * Usage:
 *   RPC_URL=https://your-rpc PRIVATE_KEY=0x... node deploy_wqnto.js
 *
 * Environment Variables:
 *   RPC_URL      - Target EVM RPC endpoint (e.g. Ethereum mainnet, Polygon, BSC)
 *   PRIVATE_KEY  - Deployer wallet private key (without 0x prefix is also accepted)
 *   GAS_PRICE    - (Optional) Gas price in gwei
 */

require('dotenv').config();
const { ethers } = require('ethers');
const fs = require('fs');
const path = require('path');

// ============================================================
// WQNTO Contract ABI & Bytecode (compiled from WQNTO.sol)
// ============================================================
// To generate bytecode, run: solc --optimize --bin WQNTO.sol
// Or use Hardhat/Foundry: npx hardhat compile

const WQNTO_ABI = [
    "constructor()",
    "function name() view returns (string)",
    "function symbol() view returns (string)",
    "function decimals() view returns (uint8)",
    "function totalSupply() view returns (uint256)",
    "function balanceOf(address) view returns (uint256)",
    "function allowance(address,address) view returns (uint256)",
    "function approve(address,uint256) returns (bool)",
    "function transfer(address,uint256) returns (bool)",
    "function transferFrom(address,address,uint256) returns (bool)",
    "function deposit() payable",
    "function withdraw(uint256)",
    "event Approval(address indexed, address indexed, uint256)",
    "event Transfer(address indexed, address indexed, uint256)",
    "event Deposit(address indexed, uint256)",
    "event Withdrawal(address indexed, uint256)"
];

// ============================================================
// DEPLOYMENT CONFIGURATION
// ============================================================

const CONFIG = {
    rpcUrl: process.env.RPC_URL || 'https://trvorth-qanto-testnet.hf.space/rpc',
    privateKey: process.env.PRIVATE_KEY,
    gasPrice: process.env.GAS_PRICE ? ethers.parseUnits(process.env.GAS_PRICE, 'gwei') : undefined,
    confirmations: 2,
};

// ============================================================
// MAIN DEPLOYMENT
// ============================================================

async function main() {
    console.log('='.repeat(60));
    console.log('🚀 WQNTO DEPLOYMENT SCRIPT');
    console.log('='.repeat(60));
    console.log(`  Token:      Wrapped QANTO (WQNTO)`);
    console.log(`  Decimals:   6`);
    console.log(`  Standard:   ERC-20 Compatible`);
    console.log(`  Solidity:   ^0.8.20`);
    console.log(`  RPC:        ${CONFIG.rpcUrl}`);
    console.log('='.repeat(60));

    if (!CONFIG.privateKey) {
        console.error('\n❌ ERROR: PRIVATE_KEY environment variable is required.');
        console.error('   Usage: PRIVATE_KEY=0x... RPC_URL=https://... node deploy_wqnto.js');
        process.exit(1);
    }

    // Connect to network
    const provider = new ethers.JsonRpcProvider(CONFIG.rpcUrl);
    const network = await provider.getNetwork();
    console.log(`\n📡 Connected to network: ${network.name} (chainId: ${network.chainId})`);

    const wallet = new ethers.Wallet(CONFIG.privateKey, provider);
    const balance = await provider.getBalance(wallet.address);
    console.log(`💰 Deployer: ${wallet.address}`);
    console.log(`💎 Balance:  ${ethers.formatUnits(balance, 6)} QNTO`);

    if (balance === 0n) {
        console.error('\n❌ Deployer wallet has zero balance. Fund it before deploying.');
        process.exit(1);
    }

    // Check for compiled bytecode
    const bytecodePath = path.join(__dirname, 'WQNTO_bytecode.bin');
    if (!fs.existsSync(bytecodePath)) {
        console.error('\n❌ WQNTO_bytecode.bin not found.');
        console.error('   Compile WQNTO.sol first:');
        console.error('     solc --optimize --bin --abi WQNTO.sol -o ./build/');
        console.error('     cp ./build/WQNTO.bin ./WQNTO_bytecode.bin');
        console.error('   Or use Hardhat:');
        console.error('     npx hardhat compile');
        process.exit(1);
    }

    const bytecode = '0x' + fs.readFileSync(bytecodePath, 'utf-8').trim();

    // Deploy
    console.log('\n🔨 Deploying WQNTO contract...');
    const factory = new ethers.ContractFactory(WQNTO_ABI, bytecode, wallet);

    const deployTx = {
        gasPrice: CONFIG.gasPrice,
    };

    const contract = await factory.deploy(deployTx);
    console.log(`📤 TX Hash: ${contract.deploymentTransaction().hash}`);
    console.log(`⏳ Waiting for ${CONFIG.confirmations} confirmations...`);

    await contract.waitForDeployment();
    const contractAddress = await contract.getAddress();

    console.log('\n' + '='.repeat(60));
    console.log('✅ WQNTO DEPLOYED SUCCESSFULLY');
    console.log('='.repeat(60));
    console.log(`  Address:    ${contractAddress}`);
    console.log(`  Name:       ${await contract.name()}`);
    console.log(`  Symbol:     ${await contract.symbol()}`);
    console.log(`  Decimals:   ${await contract.decimals()}`);
    console.log(`  Network:    ${network.name} (${network.chainId})`);
    console.log('='.repeat(60));

    // Save deployment receipt
    const receipt = {
        contractAddress,
        deployer: wallet.address,
        network: network.name,
        chainId: Number(network.chainId),
        txHash: contract.deploymentTransaction().hash,
        blockNumber: contract.deploymentTransaction().blockNumber,
        timestamp: new Date().toISOString(),
        token: {
            name: 'Wrapped QANTO',
            symbol: 'WQNTO',
            decimals: 9,
        },
    };

    const receiptPath = path.join(__dirname, 'deployment_receipt.json');
    fs.writeFileSync(receiptPath, JSON.stringify(receipt, null, 2));
    console.log(`\n📄 Deployment receipt saved to: ${receiptPath}`);
}

main().catch((err) => {
    console.error('\n❌ Deployment failed:', err.message);
    process.exit(1);
});
