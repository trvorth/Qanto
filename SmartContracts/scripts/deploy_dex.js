import hre from "hardhat";

async function main() {
  console.log("Igniting QantoSwap Smart Contracts on QANTO Layer-0...");
  const WQNTO = await hre.ethers.getContractFactory("WQNTO");
  const wqnto = await WQNTO.deploy();
  await wqnto.waitForDeployment();
  console.log(`WQNTO deployed to: ${await wqnto.getAddress()}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
