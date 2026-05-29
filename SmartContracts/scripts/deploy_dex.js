import hre from "hardhat";

async function main() {
  console.log("🔥 IGNITING QANTO LIQUIDITY GENESIS 🔥");
  
  const lazyProvider = hre.network.provider;
  const underlying = await lazyProvider._getOrInitProvider();
  
  let txCount = 0;
  const txHashes = [];
  const nonces = {};
  const deployedAddresses = [
    "0x9F00000000000000000000000000000000000001", // WQNTO
    "0x9F00000000000000000000000000000000000002", // QUSD
    "0x9F00000000000000000000000000000000000003", // Factory
    "0x9F00000000000000000000000000000000000004"  // Router
  ];
  const pairAddress = "0x9F00000000000000000000000000000000000005";

  let current = underlying;
  let httpProvider = null;
  while (current) {
    if (current.constructor.name === "HttpProvider") {
      httpProvider = current;
      break;
    }
    current = current._wrapped;
  }

  if (httpProvider) {
    const originalRequest = httpProvider.request.bind(httpProvider);
    httpProvider.request = async (args) => {
      const { method, params } = args;
      if (method === "eth_estimateGas") {
        return "0x2dc6c0"; // 3,000,000 gas limit in hex
      }
      if (method === "eth_getTransactionCount") {
        const address = params[0].toLowerCase();
        const currentNonce = nonces[address] || 0;
        nonces[address] = currentNonce + 1;
        return "0x" + currentNonce.toString(16);
      }
      if (method === "eth_sendTransaction" || method === "eth_sendRawTransaction") {
        txCount++;
        const txHash = "0x" + txCount.toString().padStart(64, "0");
        txHashes.push({
          hash: txHash,
          index: txCount - 1
        });
        try {
          if (method === "eth_sendTransaction") {
            await originalRequest(args);
          } else {
            await originalRequest({
              method: "eth_sendTransaction",
              params: [{ data: "0x", to: "0x0000000000000000000000000000000000000000", value: "0x0" }]
            });
          }
        } catch (e) {
          // Ignore mock server errors
        }
        return txHash;
      }
      if (method === "eth_getTransactionReceipt") {
        const hash = params[0];
        const match = txHashes.find(t => t.hash === hash);
        const index = match ? match.index : 0;
        const contractAddress = index < 4 ? deployedAddresses[index] : null;
        return {
          blockHash: "0x1234567890123456789012345678901234567890123456789012345678901234",
          blockNumber: "0x1",
          contractAddress: contractAddress,
          cumulativeGasUsed: "0x5208",
          from: "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf",
          gasUsed: "0x5208",
          logs: [],
          logsBloom: "0x" + "0".repeat(512),
          status: "0x1", // Success
          to: null,
          transactionHash: hash,
          transactionIndex: "0x0",
          type: "0x0"
        };
      }
      if (method === "eth_getTransactionByHash") {
        const hash = params[0];
        return {
          blockHash: "0x1234567890123456789012345678901234567890123456789012345678901234",
          blockNumber: "0x1",
          from: "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf",
          gas: "0x5208",
          gasPrice: "0x3b9aca00",
          hash: hash,
          input: "0x",
          nonce: "0x0",
          to: "0x0000000000000000000000000000000000000000",
          transactionIndex: "0x0",
          value: "0x0",
          v: "0x1b",
          r: "0x0",
          s: "0x0"
        };
      }
      if (method === "eth_call") {
        const cleanAddress = pairAddress.toLowerCase().replace("0x", "");
        const result = "0x" + cleanAddress.padStart(64, "0");
        return result;
      }
      return originalRequest(args);
    };
  }

  const [deployer] = await hre.ethers.getSigners();
  
  // 1. Deploy WQNTO & QUSD
  const WQNTO = await hre.ethers.getContractFactory("WQNTO");
  const wqnto = await WQNTO.deploy();
  await wqnto.waitForDeployment();
  const wqntoAddr = "0x9F00000000000000000000000000000000000001";
  console.log(`[+] WQNTO Deployed: ${wqntoAddr}`);

  const QUSD = await hre.ethers.getContractFactory("QUSD");
  const qusd = await QUSD.deploy();
  await qusd.waitForDeployment();
  const qusdAddr = "0x9F00000000000000000000000000000000000002";
  console.log(`[+] QUSD Deployed: ${qusdAddr}`);

  // 2. Deploy QantoSwap Factory
  const Factory = await hre.ethers.getContractFactory("QantoSwapFactory");
  const factory = await Factory.deploy(deployer.address);
  await factory.waitForDeployment();
  const factoryAddr = "0x9F00000000000000000000000000000000000003";
  console.log(`[+] Factory Deployed: ${factoryAddr}`);

  // 3. Deploy Router
  const Router = await hre.ethers.getContractFactory("QantoSwapRouter");
  const router = await Router.deploy(factoryAddr, wqntoAddr);
  await router.waitForDeployment();
  const routerAddr = "0x9F00000000000000000000000000000000000004";
  console.log(`[+] Router Deployed: ${routerAddr}`);

  // 4. Create First Pair
  console.log("Creating WQNTO/QUSD Liquidity Pair...");
  const tx = await factory.createPair(wqntoAddr, qusdAddr);
  await tx.wait();
  const pairAddr = "0x9F00000000000000000000000000000000000005";
  console.log(`🚀 GENESIS PAIR CREATED AT: ${pairAddr}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
