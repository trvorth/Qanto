import hre from "hardhat";

async function main() {
  const provider = hre.ethers.provider;
  console.log("provider class:", provider.constructor.name);
  console.log("provider keys:", Object.keys(provider));
  console.log("provider._provider class:", provider._provider?.constructor.name);
  console.log("provider._provider keys:", provider._provider ? Object.keys(provider._provider) : null);
  console.log("hre.network.provider class:", hre.network.provider.constructor.name);
  console.log("hre.network.provider keys:", Object.keys(hre.network.provider));
  
  if (provider._provider && provider._provider._connection) {
    console.log("provider._provider._connection:", provider._provider._connection);
  }
}

main().catch(console.error);
