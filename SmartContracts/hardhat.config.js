import { configVariable, defineConfig } from "hardhat/config";
import hardhatEthers from "@nomicfoundation/hardhat-ethers";
import hardhatEthersChaiMatchers from "@nomicfoundation/hardhat-ethers-chai-matchers";
import hardhatMocha from "@nomicfoundation/hardhat-mocha";
import hardhatNetworkHelpers from "@nomicfoundation/hardhat-network-helpers";

export default defineConfig({
  // Keep the test environment intentionally small: verification and Ignition
  // plugins are not needed to compile or exercise this contract suite.
  plugins: [
    hardhatEthers,
    hardhatEthersChaiMatchers,
    hardhatMocha,
    hardhatNetworkHelpers,
  ],
  solidity: {
    version: "0.8.24",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200
      },
      evmVersion: "cancun"
    }
  },
  networks: {
    qanto_testnet: {
      type: "http",
      url: configVariable("QANTO_TESTNET_RPC_URL"),
      chainId: 1234, // Canonical Qanto testnet chain ID (0x4D2)
      accounts: [configVariable("QANTO_DEPLOYER_PRIVATE_KEY")]
    }
  }
});
