const hre = require("hardhat");
const { MerkleTree } = require("merkletreejs");
const keccak256 = require("keccak256");
const fs = require("fs");

async function main() {
  console.log("QANTO Snapshot Engine: [STARTING]");

  // 1. Attach to QantoReferral from Phase 33
  const referralAddress = "0x5FbDB2315678afecb367f032d93F642f64180aa3"; // Example address
  const QantoReferral = await hre.ethers.getContractFactory("QantoReferral");
  const referral = QantoReferral.attach(referralAddress);

  // 2. Fetch all Top Addresses (Simulated addresses for Genesis 100)
  // In production, you would loop through the contract events or a subgraph
  const pioneers = [
    { address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", amount: "5000" },
    { address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8", amount: "2500" },
    // ... list would be populated from on-chain event data
  ];

  // 3. Generate Merkle Tree
  const elements = pioneers.map((p, index) => 
    hre.ethers.solidityPackedKeccak256(
      ["uint256", "address", "uint256"],
      [index, p.address, hre.ethers.parseEther(p.amount)]
    )
  );

  const tree = new MerkleTree(elements, keccak256, { sort: true });
  const root = tree.getHexRoot();

  console.log("Merkle Root Found: ", root);
  
  // 4. Save to filesystem for MerkleDistributor deployment
  const snapshotData = {
    root: root,
    pioneers: pioneers,
    tree: tree.toString()
  };

  fs.writeFileSync("./scripts/genesis_snapshot.json", JSON.stringify(snapshotData, null, 2));
  console.log("Snapshot Saved to ./scripts/genesis_snapshot.json");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
