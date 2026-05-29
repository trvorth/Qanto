import hre from "hardhat";

async function main() {
  const provider = hre.ethers.provider;
  const hp = provider._hardhatProvider;
  
  console.log("LazyInitializationProviderAdapter prototype methods:", Object.getOwnPropertyNames(Object.getPrototypeOf(hp)));
  
  // Let's trigger initialization
  console.log("Initializing underlying provider...");
  const underlying = await hp._getProvider();
  console.log("underlying class:", underlying.constructor.name);
  console.log("underlying prototype chain:");
  let proto = underlying;
  while (proto) {
    console.log(" -", proto.constructor.name, Object.keys(proto));
    // Let's print if it has _wrapped or _provider
    if (proto._wrapped) {
      console.log("   wraps:", proto._wrapped.constructor.name);
    }
    proto = Object.getPrototypeOf(proto);
  }
}

main().catch(console.error);
