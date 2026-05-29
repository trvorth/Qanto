import hre from "hardhat";

async function main() {
  const provider = hre.ethers.provider;
  const hp = provider._hardhatProvider;
  
  console.log("Initializing underlying provider via _getOrInitProvider...");
  const underlying = await hp._getOrInitProvider();
  console.log("underlying class:", underlying.constructor.name);
  console.log("underlying prototype chain:");
  
  let current = underlying;
  while (current) {
    const keys = Object.keys(current);
    const wrappedName = current._wrapped ? current._wrapped.constructor.name : "None";
    console.log(` - ${current.constructor.name} (keys: ${keys.join(', ')}, wraps: ${wrappedName})`);
    current = current._wrapped;
  }
}

main().catch(console.error);
