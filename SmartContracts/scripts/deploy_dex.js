import hre from "hardhat";

async function main() {
  console.log("🔥 IGNITING QANTO LIQUIDITY GENESIS 🔥");
  const [deployer] = await hre.ethers.getSigners();
  
  // 1. Deploy WQNTO & QUSD
  const WQNTO = await hre.ethers.getContractFactory("WQNTO");
  const wqnto = await WQNTO.deploy();
  await wqnto.waitForDeployment();
  const wqntoAddr = await wqnto.getAddress();
  console.log(`[+] WQNTO Deployed: ${wqntoAddr}`);

  const QUSD = await hre.ethers.getContractFactory("QUSD");
  const qusd = await QUSD.deploy();
  await qusd.waitForDeployment();
  const qusdAddr = await qusd.getAddress();
  console.log(`[+] QUSD Deployed: ${qusdAddr}`);

  // 2. Deploy QantoSwap Factory
  const Factory = await hre.ethers.getContractFactory("QantoSwapFactory");
  const factory = await Factory.deploy(deployer.address);
  await factory.waitForDeployment();
  const factoryAddr = await factory.getAddress();
  console.log(`[+] Factory Deployed: ${factoryAddr}`);

  // 3. Deploy Router
  const Router = await hre.ethers.getContractFactory("QantoSwapRouter");
  const router = await Router.deploy(factoryAddr, wqntoAddr);
  await router.waitForDeployment();
  console.log(`[+] Router Deployed: ${await router.getAddress()}`);

  // 4. Create First Pair
  console.log("Creating WQNTO/QUSD Liquidity Pair...");
  const tx = await factory.createPair(wqntoAddr, qusdAddr);
  await tx.wait();
  const pairAddr = await factory.getPair(wqntoAddr, qusdAddr);
  console.log(`🚀 GENESIS PAIR CREATED AT: ${pairAddr}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
