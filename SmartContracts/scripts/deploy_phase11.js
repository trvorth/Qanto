import hre from "hardhat";

async function main() {
    console.log("🔥 IGNITING PHASE XI: MASS ADOPTION CONTRACTS 🔥");
    const [deployer] = await hre.ethers.getSigners();
    
    // Synthetic Merkle Root for testing (calculated for 0x...0001 receiving 1000 QNTO)
    const mockMerkleRoot = "0x59a11702eebba0d94bf82deffcd32b988fce76b5d033621430035070ff227090";
    
    // 1. Deploy Sentinel NFT
    const Sentinel = await hre.ethers.getContractFactory("SentinelNFT");
    const sentinel = await Sentinel.deploy();
    await sentinel.waitForDeployment();
    const sentinelAddr = await sentinel.getAddress();
    console.log(`[+] SentinelNFT Deployed: ${sentinelAddr}`);

    // 2. We use the previously deployed WQNTO as the Airdrop token (0x9F00...0001)
    const wqntoSyntheticAddress = "0x9F00000000000000000000000000000000000001";

    // 3. Deploy Airdrop Contract
    const Airdrop = await hre.ethers.getContractFactory("QantoDrop");
    const airdrop = await Airdrop.deploy(wqntoSyntheticAddress, mockMerkleRoot);
    await airdrop.waitForDeployment();
    const airdropAddr = await airdrop.getAddress();
    console.log(`🚀 QANTO AIRDROP MATRIX LIVE AT: ${airdropAddr}`);
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
