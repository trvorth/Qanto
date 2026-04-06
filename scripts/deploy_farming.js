const hre = require("hardhat");

async function main() {
  const [deployer] = await hre.ethers.getSigners();
  console.log("Deploying Sovereignty Layer with account:", deployer.address);

  // 1. QantoToken (Assuming it's already deployed, otherwise redeploy for Votes)
  const QantoToken = await hre.ethers.getContractFactory("QantoToken");
  const qanto = await QantoToken.attach("0x5FbDB2315678afecb367f032d93F642f64180aa3"); // Example Phase 31 address

  // 2. Deploy SagaFarming
  const SagaFarming = await hre.ethers.getContractFactory("SagaFarming");
  const qantoPerBlock = hre.ethers.parseEther("10"); // 10 QNTO per block
  const startBlock = await hre.ethers.provider.getBlockNumber();
  
  const farming = await SagaFarming.deploy(qanto.target, qantoPerBlock, startBlock);
  await farming.waitForDeployment();
  console.log("SagaFarming deployed to:", farming.target);

  // 3. Deploy QantoGovernor
  const QantoGovernor = await hre.ethers.getContractFactory("QantoGovernor");
  const governor = await QantoGovernor.deploy(qanto.target);
  await governor.waitForDeployment();
  console.log("QantoGovernor deployed to:", governor.target);

  // 4. Initial Pool Setup (QNTO-USDT LP)
  // LP Address from Phase 33 initialization
  const lpAddress = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"; 
  await farming.add(100, lpAddress, true);
  console.log("Added QNTO-USDT LP pool to SagaFarming");

  console.log("Sovereignty Wave: [INITIALIZED]");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
