import hre from "hardhat";

async function main() {
  const provider = hre.ethers.provider;
  const hp = provider._hardhatProvider;
  
  console.log("hp class:", hp.constructor.name);
  console.log("hp keys:", Object.keys(hp));
  
  if (hp._provider) {
    console.log("hp._provider class:", hp._provider.constructor.name);
    console.log("hp._provider keys:", Object.keys(hp._provider));
    if (hp._provider._wrapped) {
      console.log("hp._provider._wrapped class:", hp._provider._wrapped.constructor.name);
      console.log("hp._provider._wrapped keys:", Object.keys(hp._provider._wrapped));
    }
  }

  // Let's print the entire prototype chain of hp
  let proto = Object.getPrototypeOf(hp);
  console.log("hp prototype chain:");
  while (proto) {
    console.log(" -", proto.constructor.name);
    proto = Object.getPrototypeOf(proto);
  }
}

main().catch(console.error);
