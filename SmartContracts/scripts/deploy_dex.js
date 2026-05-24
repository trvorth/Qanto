import hre from "hardhat";

async function main() {
  console.log("Igniting QantoSwap Smart Contracts on QANTO Layer-0...");
  
  // Deploy WQNTO
  const WQNTO = await hre.ethers.getContractFactory("WQNTO");
  const wqnto = await WQNTO.deploy();
  await wqnto.waitForDeployment();
  const wqntoAddress = await wqnto.getAddress();
  console.log(`WQNTO deployed to: ${wqntoAddress}`);

  // Deploy Factory
  const [deployer] = await hre.ethers.getSigners();
  console.log(`Deploying factory with owner: ${deployer.address}`);
  const Factory = await hre.ethers.getContractFactory("QantoSwapFactory");
  const factory = await Factory.deploy(deployer.address);
  await factory.waitForDeployment();
  const factoryAddress = await factory.getAddress();
  console.log(`QantoSwapFactory deployed to: ${factoryAddress}`);

  // Deploy Router
  const Router = await hre.ethers.getContractFactory("QantoSwapRouter");
  const router = await Router.deploy(factoryAddress, wqntoAddress);
  await router.waitForDeployment();
  const routerAddress = await router.getAddress();
  console.log(`QantoSwapRouter deployed to: ${routerAddress}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
