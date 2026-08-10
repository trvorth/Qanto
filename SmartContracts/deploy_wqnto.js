/**
 * WQNTO deployment script.
 *
 * Prerequisites:
 *   npm install
 *   npx hardhat compile
 *
 * Usage:
 *   RPC_URL=https://your-rpc PRIVATE_KEY=0x... node deploy_wqnto.js
 *
 * Environment variables:
 *   RPC_URL           - Target EVM RPC endpoint (required)
 *   PRIVATE_KEY       - Deployer private key (required)
 *   EXPECTED_CHAIN_ID - Refuse a different chain (default: 1234)
 *   GAS_PRICE         - Optional legacy gas price in gwei
 *   CONFIRMATIONS     - Required deployment confirmations (default: 2)
 */

import "dotenv/config";
import { ethers } from "ethers";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const artifactPath = path.join(
  __dirname,
  "artifacts",
  "contracts",
  "WQNTO.sol",
  "WQNTO.json",
);

function requiredEnvironmentVariable(name) {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`${name} environment variable is required`);
  }
  return value;
}

function positiveIntegerEnvironmentVariable(name, fallback) {
  const raw = process.env[name]?.trim() || fallback;
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function loadWqntoArtifact() {
  if (!fs.existsSync(artifactPath)) {
    throw new Error(
      `Compiled WQNTO artifact not found at ${artifactPath}; run \`npx hardhat compile\` first`,
    );
  }

  const artifact = JSON.parse(fs.readFileSync(artifactPath, "utf8"));
  if (!Array.isArray(artifact.abi)) {
    throw new Error("Compiled WQNTO artifact has no ABI");
  }
  if (typeof artifact.bytecode !== "string" || !/^0x[0-9a-fA-F]+$/.test(artifact.bytecode)) {
    throw new Error("Compiled WQNTO artifact has no deployable bytecode");
  }
  const constructor = artifact.abi.find((entry) => entry.type === "constructor");
  if (!constructor || constructor.inputs.length !== 0) {
    throw new Error("WQNTO constructor ABI must have zero arguments");
  }
  return artifact;
}

async function main() {
  const rpcUrl = requiredEnvironmentVariable("RPC_URL");
  const privateKey = requiredEnvironmentVariable("PRIVATE_KEY");
  const expectedChainId = BigInt(process.env.EXPECTED_CHAIN_ID?.trim() || "1234");
  const confirmations = positiveIntegerEnvironmentVariable("CONFIRMATIONS", "2");
  const artifact = loadWqntoArtifact();

  const provider = new ethers.JsonRpcProvider(rpcUrl);
  const network = await provider.getNetwork();
  if (network.chainId !== expectedChainId) {
    throw new Error(
      `Refusing deployment to chain ${network.chainId}; expected ${expectedChainId}. ` +
        "Set EXPECTED_CHAIN_ID explicitly for an intentional alternate network.",
    );
  }

  const wallet = new ethers.Wallet(privateKey, provider);
  const balance = await provider.getBalance(wallet.address);
  if (balance === 0n) {
    throw new Error(`Deployer ${wallet.address} has zero native balance`);
  }

  const overrides = {};
  if (process.env.GAS_PRICE?.trim()) {
    overrides.gasPrice = ethers.parseUnits(process.env.GAS_PRICE.trim(), "gwei");
  }

  console.log(`Deploying WQNTO (9 decimals) to chain ${network.chainId}`);
  console.log(`Deployer: ${wallet.address}`);
  console.log(`Native balance: ${ethers.formatUnits(balance, 9)} QNTO`);

  const factory = new ethers.ContractFactory(artifact.abi, artifact.bytecode, wallet);
  const contract = await factory.deploy(overrides);
  const deploymentTransaction = contract.deploymentTransaction();
  if (!deploymentTransaction) {
    throw new Error("Deployment transaction was not created");
  }

  console.log(`Deployment transaction: ${deploymentTransaction.hash}`);
  const transactionReceipt = await deploymentTransaction.wait(confirmations);
  if (!transactionReceipt || transactionReceipt.status !== 1) {
    throw new Error("WQNTO deployment transaction failed");
  }
  await contract.waitForDeployment();

  const contractAddress = await contract.getAddress();
  const deployedDecimals = await contract.decimals();
  if (deployedDecimals !== 9n) {
    throw new Error(`Deployed WQNTO reports ${deployedDecimals} decimals, expected 9`);
  }

  const receipt = {
    contractAddress,
    deployer: wallet.address,
    network: network.name,
    chainId: Number(network.chainId),
    txHash: deploymentTransaction.hash,
    blockNumber: transactionReceipt.blockNumber,
    confirmations,
    timestamp: new Date().toISOString(),
    token: {
      name: await contract.name(),
      symbol: await contract.symbol(),
      decimals: Number(deployedDecimals),
    },
  };

  const receiptPath = path.join(__dirname, "deployment_receipt.json");
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    mode: 0o600,
  });

  console.log(`WQNTO deployed at ${contractAddress}`);
  console.log(`Deployment receipt: ${receiptPath}`);
}

main().catch((error) => {
  console.error(`Deployment failed: ${error.message}`);
  process.exitCode = 1;
});
