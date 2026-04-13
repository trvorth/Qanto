/**
 * QANTO ZK-SDK v1.0.0
 * Lightweight SDK for Verifiable AI Mesh Interaction
 * (c) 2026 QANTO Sovereign Protocol
 */

const { ethers } = require("ethers");

class QantoZKSDK {
    constructor(rpcUrl, oracleAddress) {
        this.provider = new ethers.JsonRpcProvider(rpcUrl);
        this.oracleAddress = oracleAddress;
        this.abi = [
            "function submitInference(bytes32 modelHash, bytes proof, bytes data) external",
            "function getInference(bytes32 modelHash) external view returns (tuple(bytes proof, bytes data, uint256 timestamp, bool verified))",
            "event InferenceSubmitted(bytes32 indexed modelHash, address indexed submitter)"
        ];
    }

    /**
     * Submits a ZK-Inference proof to the QANTO Sentinel mesh.
     * @param {string} modelHash - keccak256 hash of the model parameters
     * @param {string} proof - The ZK-Proof (hex string)
     * @param {string} data - Input/Output data (hex string)
     * @param {ethers.Signer} signer - The Ethers signer to authorize the tx
     */
    async submitInference(modelHash, proof, data, signer) {
        const oracle = new ethers.Contract(this.oracleAddress, this.abi, signer);
        console.log(`📡 Submitting proof for model: ${modelHash}...`);
        
        try {
            const tx = await oracle.submitInference(modelHash, proof, data);
            const receipt = await tx.wait();
            console.log(`✅ Proof submitted. Block: ${receipt.blockNumber}`);
            return receipt;
        } catch (error) {
            console.error("❌ Submission failed:", error);
            throw error;
        }
    }

    /**
     * Checks the verification status of a model's inference result.
     * @param {string} modelHash 
     */
    async verifyOnChain(modelHash) {
        const oracle = new ethers.Contract(this.oracleAddress, this.abi, this.provider);
        const result = await oracle.getInference(modelHash);
        return {
            isVerified: result.verified,
            timestamp: result.timestamp,
            data: result.data
        };
    }
}

module.exports = QantoZKSDK;
