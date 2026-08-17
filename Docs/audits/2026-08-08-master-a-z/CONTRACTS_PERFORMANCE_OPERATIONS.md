# Qanto Smart Contracts, Performance, and Operations Audit

Audit date: 2026-08-08  
Audit target: effective working tree at <code>/Users/cps/qanto</code>  
Scope: mandate workstreams 8, 9, and 10  
Execution mode: source audit plus targeted build, test, coverage, and configuration checks; protocol-neutral documentation only in this track  

## Executive verdict

**NOT READY.** The current working tree has materially safer fail-closed behavior, a repaired Solidity test stack, safer Docker build-context defaults, and a non-blocking Prometheus exporter. Those are useful containment changes. They do not make the advertised contract suite, performance envelope, or deployment stack production-ready.

Independent blockers in this scope are:

1. QUSD is a controlled mint/burn token, not a collateralized or redeemable stablecoin.
2. QantoSwap has no reserve, liquidity, LP-token, invariant, fee, or settlement implementation. Factory and router entry points now correctly revert, so the AMM is safe only because it is unavailable.
3. <code>SmartContracts/scripts/deploy_dex.js</code> fabricates transaction hashes, receipts, and deployment addresses while suppressing provider errors. It can print successful deployment output when no contract was deployed.
4. The only named Solidity suites cover QantoFaucet and QUSD. Instrumented coverage confirms the gap: total line coverage is 42.45% and statement coverage is 43.36%; TGE, Drop, SentinelNFT, and Governor are 0%/0%, while Timelock has no supplied per-file coverage row.
5. The minimized Hardhat 3 stack now compiles all ten current Solidity files with solc 0.8.24 and passes all 38 tests, and <code>npm audit --json</code> reports zero advisories. Slither, Mythril, Foundry, Echidna, and a standalone Solidity compiler were unavailable, so the requested independent Solidity SAST/fuzzing gate is still absent.
6. The 10M+ TPS, 32 BPS, and sub-31 ms claims are not supported by a reproducible result artifact. Several “validation” paths count simulated work or accept every transaction without cryptographic validation.
7. The advertised 262,144-entry lock-free queue is an unbounded <code>SegQueue</code>; the SIMD cleanup path is explicitly a placeholder; and the standalone sharding implementation overwrites its own split.
8. The documented one-click Ubuntu launch path is broken before deployment: its script-path command substitution is malformed, it tries to clone under <code>/srv</code> before privileged directory preparation, and it defaults to a mutable branch.
9. The raw Kubernetes Deployment, alternate StatefulSets, and Helm chart do not form one reproducible deployment. Missing/incorrect config paths, wallet-password secrets, services, identity persistence, and a shared RWO claim prevent a clean multi-node launch.
10. The production Compose stack exposes RPC/gRPC directly without the Caddy TLS boundary, ships no production Prometheus/Grafana services, and the checked-in dashboards query mostly nonexistent metrics.
11. Dedicated VPS/Docker assets now exist, and accidental public integration is contained: README labels the HF endpoints legacy/unfit, Hardhat requires lazy environment-backed RPC/deployer values, and exchange metadata is an explicit non-submittable draft with null endpoints. Migration is still incomplete because no verified dedicated public endpoint/topology exists and separate DEX/Phase XI deployment scripts remain synthetic.

The effective worktree was already dirty and shared among audit tracks. This report records current behavior and does not infer authorship from Git status. Modified and untracked remediation is labeled <code>FIXED_IN_WORKTREE</code> or <code>CONTAINED_IN_WORKTREE</code>, not “committed.”

## Evidence labels

- <code>CONFIRMED</code>: directly established from current source.
- <code>DERIVED</code>: follows mechanically from confirmed constants or control flow.
- <code>MEASURED</code>: produced by an executed local check in this pass.
- <code>FIXED_IN_WORKTREE</code>: defect is repaired in the effective working tree.
- <code>CONTAINED_IN_WORKTREE</code>: unsafe feature now fails closed or is disabled, but the feature itself is still absent.
- <code>BLOCKED_TOOLING</code>: required tool or uncontended build resource was unavailable.

---

# Workstream 8 — Smart Contract Security

## 8.1 Contract and test inventory

The current <code>SmartContracts/contracts/</code> inventory contains the ten mandated contracts. QantoFaucet is currently untracked in Git but present in the effective audit tree.

| Contract | Current effective behavior | Access-control surface | Audit classification |
|---|---|---|---|
| WQNTO | ERC-20 wrapper with 9 decimals; deposit mints and withdrawal burns before a guarded native call | Permissionless deposit/withdrawal; no owner | Implemented primitive; focused tests absent |
| QUSD | Zero-supply, capped, pausable ERC-20 with minter, pauser, burner, and admin roles | Admin can grant roles; burner can destroy any holder balance without allowance | Controlled token primitive, not a stablecoin |
| QantoGovernor | OpenZeppelin Governor/Votes/Timelock composition | Depends on external IVotes token and Timelock roles | Compiles conceptually; deployment/test path absent and timing is wrong |
| QantoTGE | Fixed-rate native-asset sale, default inactive, owner toggle and withdrawal | Single owner controls sale availability and proceeds | Safer sale primitive; vesting/policy unimplemented |
| QantoTimelock | Generic OpenZeppelin TimelockController wrapper | Constructor-supplied proposer/executor/admin roles | Governance delay primitive, not TGE vesting |
| QantoFaucet | Test-token accounting with cap, daily limit, cooldown, owner mint, and pause | Ownable2Step owner can mint to cap and pause | Test-only primitive; production-network guard is incomplete |
| QantoDrop | Root-scoped Merkle claims with SafeERC20 | Owner can replace root and withdraw all remaining tokens | Functional centralized airdrop primitive; tests absent |
| QantoSwapFactory | <code>createPair</code> always reverts | Dormant fee setter state has no mutator | Correct fail-closed containment; AMM absent |
| QantoSwapRouter | Swap entry point validates path/deadline then always reverts | None | Correct fail-closed containment; swaps absent |
| SentinelNFT | Owner-only capped ERC-721 URI mint using <code>_safeMint</code> | Single owner mints all NFTs | Basic mint primitive; tests absent |

Only <code>SmartContracts/test/contracts.test.js</code> exists. Lines 9-145 contain 18 QantoFaucet cases and lines 149-288 contain 20 QUSD cases. No named test suite covers WQNTO, QantoGovernor, QantoTGE, QantoTimelock, QantoDrop, QantoSwapFactory, QantoSwapRouter, or SentinelNFT. The instrumented coverage run nevertheless records incidental executed lines in Factory, Router, and WQNTO; the exact percentages are reported in section 8.C.

## 8.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| SC-001 | SEV-1 | QUSD peg/collateral model | <code>SmartContracts/contracts/QUSD.sol:9-18</code>, <code>:24-38</code> | QUSD explicitly has no collateral custody, oracle validation, liquidation, redemption, or peg mechanism. It is a role-gated capped token. Deploying or marketing it as a stablecoin would be materially false and would expose users to unbounded peg risk. | CONFIRMED; deployment as stablecoin blocked |
| SC-002 | SEV-1 | QantoSwap AMM | <code>SmartContracts/contracts/QantoSwapFactory.sol:18-25</code>, <code>:28-44</code>; <code>SmartContracts/contracts/QantoSwapRouter.sol:23-37</code> | Pair creation and swaps now revert. The pair has only addresses/reserve fields and initialization; it has no deposits, reserves update, LP shares, constant-product invariant, fees, burns, swaps, or reentrancy protection. | CONTAINED_IN_WORKTREE; AMM unavailable |
| SC-003 | SEV-1 | DEX deployment integrity | <code>SmartContracts/scripts/deploy_dex.js:6-110</code>, <code>:113-147</code> | The script intercepts JSON-RPC, manufactures nonces, transaction hashes, successful receipts, contract addresses, and <code>eth_call</code> output, ignores provider errors, then prints hard-coded deployment addresses. QUSD is also deployed with no constructor arguments at lines 122-124, which no longer matches its ABI. | CONFIRMED; remove from any release/deployment path |
| SC-004 | SEV-1 | Phase XI deployment integrity | <code>SmartContracts/scripts/deploy_phase11.js:7-22</code> | The script labels a synthetic root and hard-coded synthetic WQNTO address as a live deployment dependency. It can deploy an airdrop pointing at an address where no verified token exists. | CONFIRMED |
| SC-005 | INFO | WQNTO deployment script | <code>SmartContracts/deploy_wqnto.js:1-33</code>, <code>:35-70</code>, <code>:73-87</code>, <code>:89-123</code>, <code>:124-152</code>; <code>SmartContracts/package.json:12-23</code> | The script is now native ESM, loads ABI and bytecode from the Hardhat compiler artifact, verifies the zero-argument constructor, requires RPC/private key, defaults the expected chain to 1234, refuses a mismatch, waits a validated confirmation count, verifies nine deployed decimals, and writes a mode-0600 receipt. | FIXED_IN_WORKTREE; syntax and fail-closed checks passed |
| SC-006 | SEV-2 | TGE/vesting | <code>SmartContracts/contracts/QantoTGE.sol:12-19</code>, <code>:28-48</code>; <code>SmartContracts/contracts/QantoTimelock.sol:6-12</code> | Decimal scaling and transfer safety are fixed and the sale defaults off. There is still no start/end time, per-wallet cap, allowlist, vesting allocation, cliff, release schedule, refund path, unsold-token recovery, or binding between TGE and Timelock. The owner can enable the sale at any time. | PARTIALLY_FIXED; vesting unimplemented |
| SC-007 | SEV-2 | Governance | <code>SmartContracts/contracts/QantoGovernor.sol:19-25</code>; <code>SmartContracts/contracts/WQNTO.sol:7-15</code>; <code>SmartContracts/contracts/QUSD.sol:13-18</code> | The stated “1 week at 32 BPS” period is 45,818 blocks, approximately 23.9 minutes at 32 BPS; one week would be 19,353,600 blocks. Voting delay is one block and proposal threshold is zero. No in-repo token implements ERC20Votes/IVotes, and no deployment or lifecycle test supplies a compatible token and timelock. | CONFIRMED |
| SC-008 | SEV-2 | QUSD privileged authority | <code>SmartContracts/contracts/QUSD.sol:24-30</code>, <code>:32-45</code>, <code>:52-57</code> | One admin initially controls mint, pause, role administration, and privileged burning. <code>BURNER_ROLE</code> can burn any account without allowance. This may be intentional compliance behavior, but no timelock, multisig, role-separation policy, or emergency runbook is enforced. | CONFIRMED; governance decision required |
| SC-009 | SEV-2 | Faucet deployment policy | <code>SmartContracts/contracts/QantoFaucet.sol:8-28</code>, <code>:63-70</code>, <code>:79-90</code> | The comment says production deployment is constructor-enforced, but the guard rejects only chain IDs 1 and 137. It would allow any other production chain, including a future Qanto mainnet. Public <code>claimFor</code> also lets a third party consume a recipient's cooldown. The contract exposes token-like metadata/events but has no ERC-20 transfer/allowance API. | CONFIRMED |
| SC-010 | SEV-3 | QantoDrop administration | <code>SmartContracts/contracts/QantoDrop.sol:27-35</code>, <code>:38-53</code> | Claims are now scoped by Merkle root and use SafeERC20. A single owner can nevertheless replace the active root or withdraw the entire funded balance at any time; there is no immutable campaign identifier, deadline, delayed root update, or post-campaign withdrawal rule. | PARTIALLY_FIXED |
| SC-011 | INFO | WQNTO withdrawal safety | <code>SmartContracts/contracts/WQNTO.sol:7-15</code>, <code>:26-31</code> | WQNTO now declares nine decimals, guards withdrawal against reentrancy, burns before interaction, uses <code>call</code>, and reverts on transfer failure. | FIXED_IN_WORKTREE |
| SC-012 | INFO | SentinelNFT receiver safety | <code>SmartContracts/contracts/SentinelNFT.sol:14-24</code> | Minting now uses <code>_safeMint</code>, preventing NFTs from being silently stranded in contracts that do not implement ERC721Receiver. | FIXED_IN_WORKTREE |
| SC-013 | SEV-2 | Contract test coverage | <code>SmartContracts/test/contracts.test.js:9-145</code>, <code>:149-288</code>; <code>SmartContracts/package.json:6-23</code>; <code>SmartContracts/hardhat.config.js:9-26</code> | Instrumented <code>npx hardhat --coverage test</code> compiled ten Solidity files with solc 0.8.24 and passed 38/38, but total line/statement coverage is only 42.45%/43.36%. TGE, Drop, SentinelNFT, and Governor are 0%/0%; no coverage threshold is enforced. | MEASURED; major gaps remain |
| SC-014 | SEV-2 | Solidity static analysis | Tool inventory result; <code>SmartContracts/package.json:13-28</code>; <code>SmartContracts/hardhat.config.js:9-17</code> | <code>npm audit --json</code> reports zero dependency advisories. Slither, Mythril, solc, Foundry, and Echidna were unavailable, and none is declared in the minimized workspace. No Solidity SAST/fuzzing artifact exists in this audit tree. Dependency audit is not a substitute for contract analysis. | PARTIALLY_VERIFIED: npm audit clean; Solidity tooling blocked |
| SC-015 | INFO | Network/release metadata | <code>SmartContracts/cex-listing-payload.json:2-10</code>, <code>:12-30</code>, <code>:37-60</code>; <code>SmartContracts/hardhat.config.js:1</code>, <code>:26-32</code>; <code>README.md:340-360</code> | Chain IDs now agree at <code>0x4D2</code>/1234. Issuance is explicitly unresolved, public endpoints are null, API endpoints and audits are empty, and the payload is <code>DRAFT_DO_NOT_SUBMIT</code>. Hardhat no longer loads root <code>../.env</code> or hardcodes HF; it lazily requires <code>QANTO_TESTNET_RPC_URL</code> and <code>QANTO_DEPLOYER_PRIVATE_KEY</code>. README labels displayed HF values as legacy metadata and the topology as unsuitable. | FIXED_IN_WORKTREE; intentionally fail closed pending deployment |

## 8.B Implemented fixes in the effective worktree

The security-relevant Solidity changes are real and directionally correct:

| Fix | Current evidence | Verification |
|---|---|---|
| WQNTO withdrawal changed from a fixed-gas transfer to checks-effects-interactions with <code>nonReentrant</code> and a checked native <code>call</code>; decimals remain nine. | <code>SmartContracts/contracts/WQNTO.sol:7-15</code>, <code>:26-31</code> | Source verified; no WQNTO test exists |
| QUSD changed from a one-billion-token deployer premint to a zero-supply, capped, role-gated, pausable, burnable primitive and explicitly disclaims peg functionality. | <code>SmartContracts/contracts/QUSD.sol:9-18</code>, <code>:24-65</code>; <code>SmartContracts/test/contracts.test.js:149-288</code> | 20 tests passed in the full compile-and-test run |
| TGE now uses SafeERC20, correct nine-decimal scaling, zero-address checks, a checked native withdrawal, and defaults inactive. | <code>SmartContracts/contracts/QantoTGE.sol:9-19</code>, <code>:23-48</code> | Source verified; no TGE test exists |
| QantoDrop now validates constructor/root inputs, scopes claim state by root, and uses SafeERC20 for claims and owner withdrawal. | <code>SmartContracts/contracts/QantoDrop.sol:10-24</code>, <code>:27-53</code> | Source verified; no airdrop test exists |
| Unsafe placeholder AMM entry points now revert with <code>AmmNotImplemented</code>. | <code>SmartContracts/contracts/QantoSwapFactory.sol:4-25</code>; <code>SmartContracts/contracts/QantoSwapRouter.sol:6-37</code> | Source verified; this is containment, not AMM implementation |
| SentinelNFT now uses safe receiver-aware minting. | <code>SmartContracts/contracts/SentinelNFT.sol:14-24</code> | Source verified |
| Hardhat was migrated to a minimized ESM-compatible Hardhat 3 plugin stack and a 38-case Faucet/QUSD suite was added. Network credentials/endpoints now use lazy Hardhat config variables instead of loading root <code>../.env</code> or embedding HF. | <code>SmartContracts/hardhat.config.js:1-24</code>, <code>:26-32</code>; <code>SmartContracts/package.json:6-28</code>; <code>SmartContracts/test/contracts.test.js:1-288</code> | Fresh full <code>npm test</code>: ten files compiled with solc 0.8.24 and 38/38 passed with no dotenv injection; <code>npm audit --json</code>: 0 advisories |
| WQNTO deployment was rewritten as a fail-closed ESM artifact-based deployment with chain-ID, constructor, confirmation, receipt-status, and deployed-decimal checks. | <code>SmartContracts/deploy_wqnto.js:19-33</code>, <code>:35-70</code>, <code>:73-87</code>, <code>:89-146</code> | <code>node --check deploy_wqnto.js</code> passed; missing-RPC execution failed closed before provider/deployment work |

Exact security-relevant diff shape, with unchanged context omitted:

~~~diff
diff --git a/SmartContracts/contracts/WQNTO.sol b/SmartContracts/contracts/WQNTO.sol
+import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
-contract WQNTO is ERC20 {
+contract WQNTO is ERC20, ReentrancyGuard {
-    function withdraw(uint wad) public {
+    function withdraw(uint wad) public nonReentrant {
         _burn(msg.sender, wad);
-        payable(msg.sender).transfer(wad);
+        (bool success, ) = payable(msg.sender).call{value: wad}("");
+        require(success, "WQNTO: Native transfer failed");
     }

diff --git a/SmartContracts/contracts/QUSD.sol b/SmartContracts/contracts/QUSD.sol
-contract QUSD is ERC20 {
-    constructor() ERC20("Qanto USD", "QUSD") {
-        _mint(msg.sender, 1000000000 * 10 ** decimals());
-    }
-}
+contract QUSD is ERC20, ERC20Burnable, ERC20Pausable, AccessControl {
+    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
+    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
+    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");
+    uint256 public constant MAX_SUPPLY = 1_000_000_000 ether;
+    constructor(address admin, address initialMinter) ERC20("Qanto USD", "QUSD") { ... }
+    function mint(address to, uint256 amount)
+        external onlyRole(MINTER_ROLE) whenNotPaused { ... }
+    function pause() external onlyRole(PAUSER_ROLE) { _pause(); }
+    function unpause() external onlyRole(PAUSER_ROLE) { _unpause(); }
+    function burnFrom(address account, uint256 amount) public override { ... }
+}

diff --git a/SmartContracts/contracts/QantoTGE.sol b/SmartContracts/contracts/QantoTGE.sol
+    using SafeERC20 for IERC20;
+    uint256 public constant QNTO_SCALE = 1e9;
+    uint256 private constant WEI_PER_ETH = 1e18;
-    bool public isPresaleActive = true;
+    bool public isPresaleActive;
-    uint256 tokensToTransfer = msg.value * RATE;
+    uint256 tokensToTransfer = (msg.value * RATE * QNTO_SCALE) / WEI_PER_ETH;
-    require(qntoToken.transfer(msg.sender, tokensToTransfer), "Token transfer failed.");
+    qntoToken.safeTransfer(msg.sender, tokensToTransfer);

diff --git a/SmartContracts/contracts/QantoSwapFactory.sol b/SmartContracts/contracts/QantoSwapFactory.sol
+    error AmmNotImplemented();
     function createPair(address tokenA, address tokenB)
-        external returns (address pair) {
-        // deployed an empty pair
+        external pure returns (address pair) {
+        tokenA; tokenB; pair;
+        revert AmmNotImplemented();
     }

diff --git a/SmartContracts/contracts/QantoSwapRouter.sol b/SmartContracts/contracts/QantoSwapRouter.sol
+    error AmmNotImplemented();
     function swapExactTokensForTokens(...) external view returns (uint[] memory) {
         require(path.length >= 2, "QantoSwap: INVALID_PATH");
         require(deadline >= block.timestamp, "QantoSwap: EXPIRED");
-        return amounts;
+        revert AmmNotImplemented();
     }
~~~

The ellipses above replace unchanged/full helper bodies only; the byte-for-byte effective files and exact line ranges are cited in the table. A release review should archive the complete Git diff, including untracked Faucet/test files, before commit.

## 8.C Static-tool and test results

| Command/check | Result | Interpretation |
|---|---|---|
| Fresh <code>npm test</code> in <code>SmartContracts/</code> | PASS: ten Solidity files compiled with solc 0.8.24; 38/38 | Current sources compile and the Faucet/QUSD suites pass without root dotenv injection or public-network credentials. |
| <code>npx hardhat --coverage test</code> | PASS: ten Solidity files compiled with solc 0.8.24; 38/38 tests passed | Instrumented total coverage is 42.45% lines and 43.36% statements. |
| <code>npm audit --json</code> in <code>SmartContracts/</code> | PASS: 0 advisories | The current npm dependency graph reports zero known advisories at audit time. This does not analyze Solidity logic. |
| <code>node --check deploy_wqnto.js</code> | PASS | The current artifact-based deployment script is syntactically valid ESM. |
| Isolated <code>deploy_wqnto.js</code> run with RPC_URL absent | PASS, failed closed with the required-environment error | Missing deployment authority/configuration is rejected before provider, signing, or deployment work. |
| Slither/Mythril/solc/Forge/Echidna availability check | All unavailable | Required SAST/fuzzing gate not run. |
| Package/config inspection | <code>SmartContracts/package.json:13-28</code> declares Hardhat 3.12.0, ethers 6.17.0, OpenZeppelin 5.6.1, four focused Hardhat plugins, and security overrides; <code>SmartContracts/hardhat.config.js:1-24</code> loads only those plugins and pins Solidity 0.8.24 with optimizer/Cancun settings; lines 26-32 lazily require RPC/key variables and chain 1234 | Minimized test stack is coherent, local tests do not load root <code>../.env</code>, and network deployment fails closed without explicit variables. |

### Instrumented coverage

The following line/statement percentages are the measured output of the instrumented Hardhat run:

| Contract | Current source evidence | Line coverage | Statement coverage |
|---|---|---:|---:|
| QUSD | <code>SmartContracts/contracts/QUSD.sol:9-65</code>; tests at <code>SmartContracts/test/contracts.test.js:149-288</code> | 94.12% | 95.65% |
| QantoFaucet | <code>SmartContracts/contracts/QantoFaucet.sol:8-172</code>; tests at <code>SmartContracts/test/contracts.test.js:9-145</code> | 89.19% | 88.89% |
| QantoSwapFactory | <code>SmartContracts/contracts/QantoSwapFactory.sol:4-44</code> | 47.06% | 0% |
| QantoSwapRouter | <code>SmartContracts/contracts/QantoSwapRouter.sol:6-37</code> | 8.33% | 0% |
| WQNTO | <code>SmartContracts/contracts/WQNTO.sol:7-31</code> | 10% | 0% |
| QantoTGE | <code>SmartContracts/contracts/QantoTGE.sol:9-48</code> | 0% | 0% |
| QantoDrop | <code>SmartContracts/contracts/QantoDrop.sol:10-53</code> | 0% | 0% |
| SentinelNFT | <code>SmartContracts/contracts/SentinelNFT.sol:7-24</code> | 0% | 0% |
| QantoGovernor | <code>SmartContracts/contracts/QantoGovernor.sol:11-36</code> | 0% | 0% |
| QantoTimelock | <code>SmartContracts/contracts/QantoTimelock.sol:6-12</code> | Not supplied as a per-file row | Not supplied as a per-file row |
| **Total** | Instrumented Hardhat result | **42.45%** | **43.36%** |

Coverage now has a reproducible measurement, but it is not a quality pass. Critical value-bearing and authority paths remain at 0%, no package script or CI threshold enforces the result, and line/statement coverage does not replace branch, invariant, fuzz, or mutation testing.

## 8.D Deferred items

| Item | Why deferred | Estimated effort | Priority |
|---|---|---:|---|
| Design and implement an actual QUSD system | Requires collateral type, oracle trust model, health factor, liquidations, redemption, bad-debt handling, emergency controls, and economic simulation—not a patch to ERC-20 minting | 6-12 engineering weeks plus independent economic/security audit | P0 before any stablecoin claim |
| Implement or remove QantoSwap | Constant-product math, LP token, fee accounting, protocol fee, slippage, fee-on-transfer/rebasing policy, oracle/TWAP, reentrancy and invariant testing are absent | 6-10 engineering weeks plus external audit | P0 before enabling |
| Remove/replace fabricated DEX and Phase XI deployment scripts | <code>deploy_wqnto.js</code> is repaired, but <code>scripts/deploy_dex.js</code> and <code>scripts/deploy_phase11.js</code> still use synthetic success data/addresses. They need canonical deployment modules, real receipt/address verification, constructor arguments, post-deploy role setup, and rollback manifests. | 2-4 days | P0 |
| Specify TGE and vesting | Allocation tables and legal/economic schedule are not defined in code or configuration | 2-4 weeks after policy approval | P0 before sale |
| Make governance deployable | Select ERC20Votes token/checkpoint strategy, correct periods, set nonzero proposal threshold, role the timelock, and test full proposal lifecycle | 1-2 weeks | P1 |
| Harden Faucet/Drop authority | Allowlist testnet chain IDs, remove or authenticate <code>claimFor</code>, document non-ERC20 behavior, add delayed/frozen campaign controls | 2-5 days | P1 |
| Complete test/SAST gate | Raise measured 42.45%/43.36% line/statement coverage with unit/invariant/fuzz tests for all ten contracts, add deployment smoke tests and an enforced threshold, then run Slither SARIF and a second analyzer | 1-2 weeks | P0 |

---

# Workstream 9 — Performance and Scalability

## 9.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| PERF-001 | SEV-2 | 10M+ TPS claim | <code>Protocol_Core/qanto-node/src/qantodag.rs:68-74</code>; <code>README.md:163-164</code>; <code>Docs/ROADMAP.md:180-181</code>, <code>:296</code> | The configured 312,500 transactions × 32 BPS equals 10M TPS only if the 32 MiB block averages at most 107.37 bytes per transaction. The benchmark default is 250 bytes, allowing at most 134,217 transactions or approximately 4.295M theoretical TPS before headers, signatures, validation, persistence, gossip, and consensus. No result artifact supports README's 25,447,000 TPS claim. | DERIVED; claim unverified |
| PERF-002 | SEV-2 | Synthetic performance validation | <code>Protocol_Core/qanto-node/src/performance_validation.rs:65-151</code>, <code>:184-259</code>, <code>:533-540</code> | BPS validation sleeps 10 ms and increments a counter without building/validating a block. TPS validation adds one million transactions per loop and sleeps 10 μs. Performance-test and short-duration paths count requested transactions without generating or accepting them; CPU/RAM are hard-coded to 75%/512 MB. | CONFIRMED; not benchmark evidence |
| PERF-003 | SEV-2 | Optimized transaction validation | <code>Protocol_Core/qanto-node/src/performance_optimizations.rs:102-167</code>, <code>:200-232</code>, <code>:296-322</code> | The synchronous production validator always returns true. The alternate signature path treats any non-empty signature as valid and caches by transaction ID. <code>process_batch</code> uses the always-true synchronous path. The builder is instantiated in <code>node.rs:494-496</code> but no production call to either build method was found, so this is latent rather than the canonical block path. | CONFIRMED latent safety defect |
| PERF-004 | SEV-2 | Queue/SIMD claims | <code>Protocol_Core/qanto-node/src/qantodag.rs:940-960</code>, <code>:998-999</code>, <code>:1042-1062</code>, <code>:4497-4545</code> | The actual block queue is bounded at 512. The advertised 262,144 lock-free transaction queue is an unbounded <code>SegQueue</code>; maintenance only logs when it crosses a nominal threshold. The “SIMD” cleanup iterates at most 32 map entries and increments a counter; it performs no SIMD signature verification. | CONFIRMED |
| PERF-005 | SEV-2 | Optimized mempool/block builder | <code>Protocol_Core/qanto-node/src/performance_optimizations.rs:428-467</code>, <code>:489-503</code>, <code>:536-631</code> | Lookup, priority, and byte counters are mutated before the wrapped mempool accepts a transaction, leaving ghost state on failure. Removal deletes lookup entries before constructing the removal objects, producing an empty list. Candidate transactions are popped, ignored, and then fetched again by count. | CONFIRMED latent corruption |
| PERF-006 | SEV-2 | Memory optimizer | <code>Protocol_Core/qanto-node/src/memory_optimization.rs:46-69</code>, <code>:188-207</code>, <code>:259-300</code>, <code>:426-515</code> | “Persistent” mmap storage truncates its file on every open. Unsafe Send/Sync comments say raw pointers are not exposed, but a public allocator returns <code>NonNull&lt;u8&gt;</code>. The “zero-copy” mempool moves then clones transactions, undercounts heap memory with <code>size_of_val</code>, and sorts ascending fee while taking from the front. | CONFIRMED; mmap path not found in node wiring |
| PERF-007 | SEV-2 | Hyper-sharding | <code>Protocol_Core/qanto-node/src/sharding.rs:21-90</code>; <code>Protocol_Core/qanto-node/src/qantodag.rs:4414-4419</code> | The standalone manager removes shard zero, computes <code>next_id</code> from the now-empty map as zero, then inserts both halves under ID zero, overwriting the first. It has no routing, cross-shard execution, receipts, state roots, validator assignment, or consensus integration. Automatic dynamic sharding is correctly disabled. | CONTAINED_IN_WORKTREE; specified feature absent |
| PERF-008 | SEV-2 | Reproducible benchmark | <code>Protocol_Core/qanto-node/src/performance_benchmark.rs:35-65</code>, <code>:341-407</code>, <code>:1610-1622</code> | A more realistic benchmark harness exists and defaults to 1,000 TPS with 250-byte transactions, but no committed <code>benchmark_results_*.json</code> artifact was found. The available debug binary exits before measurement with <code>QuantumSignature(SigningError)</code>. | MEASURED evidence gate failure |
| PERF-009 | SEV-2 | Release panic/durability | <code>Cargo.toml:44-51</code>; <code>Protocol_Core/qanto-node/src/node.rs:261-263</code>; <code>Protocol_Core/qanto-node/src/persistence.rs:186-200</code>, <code>:251-260</code> | Fat LTO, one codegen unit, stripping, and opt-level 3 are reasonable throughput choices. <code>panic = "abort"</code> skips destructors and the persistence writer's graceful drain/flush path. The storage layer has WAL syncs, but no crash-injection/recovery evidence proves all accepted critical state survives abort/power loss. | CONFIRMED configuration; durability unproven |
| PERF-010 | SEV-3 | Worker topology | <code>Protocol_Core/qanto-node/src/qantodag.rs:97-111</code>, <code>:1042-1069</code> | 256 block permits and 128 validation permits are wired, and a 128-thread Rayon pool is created regardless of host CPU. These are configuration counts, not proof of parallel speedup, and will oversubscribe the documented 8-16 vCPU testnet host. | CONFIRMED; tune from measurements |
| PERF-011 | SEV-3 | Performance monitor | <code>Protocol_Core/qanto-node/src/qantodag.rs:959-960</code>; <code>Protocol_Core/qanto-node/src/performance_monitoring.rs:341-412</code>, <code>:567-586</code> | The monitor is explicitly allowed dead code in the DAG. Adaptive tuning mutates its own parameter object; no evidence shows those values reconfigure live workers/queues. Rate calculation averages previous rate snapshots rather than calculating counter deltas over time. | CONFIRMED |
| PERF-012 | SEV-3 | Optimized Q-DAG | <code>Protocol_Core/qanto-node/src/optimized_qdag.rs:57-67</code>, <code>:348-381</code>, <code>:495-503</code> | SIMD is enabled by architecture, but the function imports x86 intrinsics and then performs ordinary sequential hash calls. Predictive generation defaults true but is explicitly disabled in its implementation. | CONFIRMED |

## 9.B Claim-versus-evidence matrix

| Claim | Configuration/source fact | Executed or persisted evidence | Assessment |
|---|---|---|---|
| 10M+ TPS | 312,500 tx/block × 32 BPS constant | No successful current benchmark; no result JSON; prebuilt diagnostic exits with SigningError | **NOT VERIFIED** |
| 25,447,000 “hyperscale” TPS | README table only | No raw data, command, host manifest, flamegraph, latency distribution, or acceptance count | **UNSUPPORTED** |
| 32 BPS | Constants/comments and a validator that sleeps 10 ms | Validator does not construct, sign, gossip, validate, persist, or finalize blocks | **SIMULATED, NOT MEASURED** |
| Less than 31 ms block processing | 25 ms validation timeout constant | No end-to-end p50/p95/p99 artifact under load | **NOT VERIFIED** |
| 256 block + 128 transaction workers | Semaphores and a 128-thread Rayon pool are constructed | No scaling curve; likely oversubscribed on documented host | **CONFIGURED ONLY** |
| Lock-free queue capacity 262,144 | Nominal constant exists | Data structure is unbounded and only monitored | **FALSE AS IMPLEMENTED** |
| SIMD batch size 32 for signature checks | Constant exists | Active cleanup is a scalar placeholder; alternate validator checks only non-empty bytes | **FALSE AS IMPLEMENTED** |
| Hyper-sharding | 90-line standalone manager | Split overwrites itself; no protocol integration; automatic use disabled | **STUB/CONTAINED** |
| Realistic benchmark support | Harness generates transactions, sets UTXOs, warms up, runs workers, and saves JSON | Default is 1,000 TPS; current diagnostic exits before measurement; artifacts absent | **HARNESS PRESENT, EVIDENCE ABSENT** |

The block-size arithmetic is deterministic:

~~~text
33,554,432 bytes / 312,500 transactions = 107.374 bytes/transaction
33,554,432 bytes / 250 benchmark bytes   = 134,217 whole transactions
134,217 transactions * 32 blocks/second = 4,294,944 theoretical TPS
~~~

This upper bound still excludes block/header encoding, signatures, Merkle data, UTXO reads/writes, network propagation, consensus, and storage. It is not an attainable throughput estimate.

## 9.C Implemented safety changes

No measured bottleneck was optimized in this pass because there is no trustworthy baseline. One critical protocol-neutral containment change is present:

~~~diff
diff --git a/Protocol_Core/qanto-node/src/qantodag.rs b/Protocol_Core/qanto-node/src/qantodag.rs
-        if let Err(e) = self.finalize_blocks().await { ... }
-        if let Err(e) = self.dynamic_sharding().await { ... }
+        // Automatic finality and dynamic sharding are intentionally disabled.
+        // Both currently derive consensus-affecting state from local observations
+        // rather than a committed, deterministic protocol transition. Re-enable
+        // only after their consensus specifications and multi-node determinism
+        // tests are implemented.
~~~

Current evidence is <code>Protocol_Core/qanto-node/src/qantodag.rs:4414-4419</code>. This prevents a broken sharding/finality path from changing live consensus state. It does not implement either feature.

## 9.D Targeted result

<code>./target/debug/performance-benchmark --duration-secs 1 --num-workers 1</code> exited 1 in approximately 0.3 seconds:

~~~text
WARN qanto::persistence: Persistence writer thread exiting: receiver disconnected
Error: QuantumSignature(SigningError)
~~~

The binary is useful only as a diagnostic. Without a clean rebuild, signed fixture provenance, accepted-transaction counters, and a result artifact, it cannot support or refute the headline throughput numerically.

## 9.E Required benchmark design

A launch-grade benchmark should:

1. build from a recorded commit with <code>--locked</code>, a pinned toolchain, and a machine/container manifest;
2. use deterministic valid signed UTXO chains, invalid-signature controls, conflicts, large-input transactions, and realistic 250-500 byte payload distributions;
3. count only transactions that pass canonical validation and are durably committed to the selected chain;
4. exercise at least three nodes over real TCP/WSS, including gossip, serialization, persistence, and one network impairment profile;
5. report sustained accepted TPS/BPS, p50/p95/p99/p99.9 latency, rejection reasons, CPU, RSS, allocator behavior, disk IOPS/fsync latency, bandwidth, queue depth, and recovery after overload;
6. publish raw JSON/CSV, exact command, host topology, fixture seed, and flamegraphs;
7. run scaling curves at 1, 2, 4, 8, 16, 32, and configured worker counts, rather than assuming more workers are faster;
8. include 30-minute steady-state, spike, soak, and adversarial-invalid-transaction phases.

## 9.F Deferred items

| Item | Why deferred | Estimated effort | Priority |
|---|---|---:|---|
| Replace synthetic validators with canonical acceptance accounting | Requires reusable valid signed fixtures and consensus-safe test harness | 1-2 weeks | P0 |
| Fix or delete latent optimized builder/mempool paths | Current path accepts all transactions and corrupts its queues; optimizing it before correctness would be reckless | 1 week | P0 before wiring |
| Publish reproducible baseline and scaling curves | Requires stable build, dedicated hosts, multi-node topology, and raw artifact storage | 1-2 weeks | P0 before performance claims |
| Implement real SIMD/bounded queue semantics | Must preserve byte-identical verification and deterministic overload behavior | 2-4 weeks after profiling | P1 |
| Specify sharding protocol | Needs state partitioning, cross-shard atomicity, receipts, validator assignment, data availability, reorg/finality semantics, and formal invariants | 3-6 engineer-months minimum | P0 before advertising/enabling |
| Prove abort crash consistency | Requires crash injection at persistence boundaries and automated restore/state-root comparison | 1-2 weeks | P1 before mainnet |

---

# Workstream 10 — DevOps, Infrastructure, and Deployment

## 10.A Findings table

| ID | Severity | Component | File:Line | Description | Status |
|---|---|---|---|---|---|
| OPS-001 | SEV-1 | Ubuntu bootstrapper | <code>DevOps/docker/ubuntu-testnet-ignite.sh:4-8</code>, <code>:134-153</code>, <code>:370-403</code>; <code>PUBLIC_TESTNET_LAUNCH.md:5-19</code> | The preferred launch script calls Python without the required path argument and places <code>BASH_SOURCE[0]</code> as a separate command inside command substitution. Even after that fix, it clones under <code>/srv/qanto</code> before privileged directory preparation, defaults to mutable <code>main</code>, and suppresses pull failure. The guide's claim that this path launches and verifies the first block is not reproducible. | CONFIRMED launch blocker |
| OPS-002 | SEV-1 | Raw Kubernetes Deployment | <code>DevOps/k8s/manifests/qanto-node-deployment.yaml:10-16</code>, <code>:50-87</code>, <code>:112-118</code>; <code>DevOps/k8s/manifests/qanto-node-pvc.yaml:1-15</code>; <code>DevOps/docker/Dockerfile:59-83</code> | Two replicas mount one ReadWriteOnce claim. Config is mounted at <code>/config</code>, while the image expects <code>/opt/qanto/config/config.toml</code>; no CONFIG_PATH or wallet-password secret is provided. The maintained entrypoint therefore exits before starting the node. | CONFIRMED launch blocker |
| OPS-003 | SEV-2 | Alternate raw StatefulSets | <code>DevOps/k8s/statefulset.yaml:7-56</code>, <code>:87-104</code>; <code>DevOps/k8s/manifests/qanto-node-pvc.yaml:49-107</code> | The first StatefulSet references <code>qanto-headless</code>, which has no matching Service in the inventory, and a <code>qanto-wallet-password</code> Secret not declared in repo. The validator StatefulSet references another undeclared secret and omits the maintained image's required config/password paths. Competing manifest sets use different names and UIDs. | CONFIRMED |
| OPS-004 | SEV-1 | Helm chart | <code>DevOps/helm/qanto-node/values.yaml:4-7</code>, <code>:69-98</code>; <code>DevOps/helm/qanto-node/templates/deployment.yaml:46-60</code>, <code>:80-108</code> | Deployment always mounts a generated wallet-password Secret, but no Secret template or existing-secret value exists. Default image repository is <code>qanto</code>, not the maintained <code>qanto/qanto-node</code>. Values expose ingress, autoscaling, monitoring, and ServiceMonitor knobs with no corresponding templates. Default chart is not self-contained. | CONFIRMED launch blocker |
| OPS-005 | SEV-2 | Public RPC edge | <code>DevOps/docker/docker-compose.prod.yml:67-92</code>; <code>DevOps/docker/Caddyfile:5-15</code> | RPC HTTP and gRPC bind directly to host ports. Caddy proxies only QantoScan and provides no RPC/gRPC TLS, authentication, request-size limit, rate limit, or abuse isolation. | CONFIRMED |
| OPS-006 | SEV-2 | Development Compose config | <code>DevOps/docker/docker-compose.yml:38-49</code>; <code>DevOps/docker/README.md:61-65</code>; <code>Protocol_Core/qanto-node/src/node.rs:305-314</code> | Compose mounts the entire config directory read-only, while the deployment README says it must be writable and Node::new persists <code>local_full_p2p_address</code>. A first start with a blank/stale value can fail saving configuration. | CONFIRMED |
| OPS-007 | SEV-2 | Grafana metric compatibility | <code>grafana/dashboards/qanto-testnet.json:15-106</code>; <code>DevOps/docker/monitoring/grafana/dashboards/qanto-node-overview.json:105-259</code>; <code>Protocol_Core/qanto-node/src/metrics.rs:813-901</code> | The testnet dashboard's eight Qanto panels match only <code>qanto_mempool_size</code>. The Docker overview queries nonexistent height and peer metrics; only its transactions panel matches. Most dashboards will be blank or misleading. | CONFIRMED |
| OPS-008 | SEV-2 | Production observability | <code>DevOps/docker/docker-compose.prod.yml:9-191</code>; <code>DevOps/docker/monitoring/prometheus/prometheus.yml:15-64</code> | Production Compose contains bootnode, rpcnode, QantoScan, and Caddy but no Prometheus, Grafana, Alertmanager, cAdvisor, or node-exporter. The development Prometheus config references rules and exporters that are absent from its Compose stack. There is no production alert route/runbook wiring. | CONFIRMED |
| OPS-009 | SEV-2 | Monitoring exposure/defaults | <code>DevOps/docker/docker-compose.yml:105-123</code>, <code>:145-164</code> | Prometheus admin API is enabled and published to the host. Grafana is published with fallback admin password <code>admin</code>. These defaults are acceptable only for isolated local development, not a public host. | CONFIRMED |
| OPS-010 | SEV-2 | HF-to-VPS migration | <code>PUBLIC_TESTNET_LAUNCH.md:1-19</code>; <code>README.md:340-360</code>; <code>SmartContracts/hardhat.config.js:1</code>, <code>:26-32</code>; <code>SmartContracts/cex-listing-payload.json:2-3</code>, <code>:28-44</code>, <code>:57-60</code> | Dedicated Ubuntu/Docker intent exists and legacy root/HF Docker files are deleted in the effective dirty tree. README now marks HF values as legacy/unfit; Hardhat requires explicit lazy environment values; exchange metadata is draft-only with null endpoints and no audit claim. These changes prevent accidental integration, but no verified dedicated RPC/WSS/explorer endpoints exist and the preferred Ubuntu deployment remains broken. | CONTAINED_IN_WORKTREE; migration safely unadvertised but incomplete |
| OPS-011 | SEV-2 | Backup/restore | <code>PUBLIC_TESTNET_LAUNCH.md:279-299</code> | The guide tars live Docker volume directories, assumes unprefixed volume names, gives no database quiesce/snapshot procedure, encryption, checksum, off-host copy, retention, restore steps, or restore test. A backup that has never been restored is just optimism in a tarball. | CONFIRMED |
| OPS-012 | SEV-3 | Lightweight AWS image | <code>DevOps/docker/Dockerfile.lightweight:11-23</code>; <code>Protocol_Core/qanto-node/Cargo.toml:2-4</code>, <code>:203-211</code> | The untracked image builds a nonexistent <code>release-aws-small</code> profile and package <code>qanto-node</code>, copies nonexistent binary <code>qanto-node</code>, and references a config outside the build context path shown. The actual package/binary is <code>qanto</code>. | CONFIRMED |
| OPS-013 | SEV-3 | Rust build pin | <code>DevOps/docker/Dockerfile:1-5</code>, <code>:22-24</code>; <code>rust-toolchain.toml:1-4</code> | Docker defaults to Rust 1.75 while the repository pins nightly-2026-06-28. This can trigger an implicit toolchain download or version mismatch and makes the base image argument misleading. | CONFIRMED |
| OPS-014 | INFO | Docker build context | <code>.dockerignore:1-65</code>; <code>DevOps/docker/Dockerfile:18-24</code> | Root Docker context now excludes Git metadata, environment files, Rust/Node build output, databases, node state, wallets, keys, certificates, and secret directories while preserving explicitly named examples. | FIXED_IN_WORKTREE |
| OPS-015 | SEV-2 | Prometheus async runtime safety | <code>Protocol_Core/qanto-node/src/metrics.rs:802-809</code>, <code>:881-892</code>, <code>:1685-1697</code>; <code>Protocol_Core/qanto-node/src/metrics_server.rs:25-27</code>; <code>Protocol_Core/qanto-node/src/node.rs:4377-4379</code> | The exporter previously called Tokio <code>blocking_read</code> from async handlers, which can panic on a runtime thread. It is now async, all handlers await it, and a focused runtime regression test passes. | FIXED_IN_WORKTREE, MEASURED |
| OPS-016 | INFO | Nextest configuration | <code>.config/nextest.toml:1-47</code> | Invalid/ignored root Nextest configuration was replaced by the canonical path with valid profiles, filters, and test groups. | FIXED_IN_WORKTREE, parser verified |

## 10.B Deployment reproducibility assessment

| Target | Static validity | Runtime/deploy verification | Verdict |
|---|---|---|---|
| Maintained Dockerfile | Multi-stage build targets the actual <code>qanto</code> binaries and runs non-root UID 10001 | Docker unavailable; image not built in this pass; toolchain pin mismatch remains | Not proven reproducible |
| Development Compose | Source is internally substantial and includes node/Prometheus/Grafana/Loki/Promtail | Docker unavailable; config is incorrectly read-only; insecure public defaults | Local-only and currently fragile |
| Production Compose | Dedicated bootnode/RPC/QantoScan/Caddy topology exists | Docker unavailable; bootstrap broken; RPC bypasses Caddy; no monitoring stack | Not public-testnet ready |
| Raw Kubernetes YAML | Eight non-template YAML files parsed successfully with Ruby YAML | kubectl unavailable; semantic config/secret/PVC defects confirmed by inspection | Not deployable as a coherent set |
| Helm chart | Template inventory exists for deployment, service, service account, ConfigMap, PVC | Helm unavailable; Secret template absent and several values are inert | Default render/deploy cannot be certified |
| Ubuntu 22.04 guide | Detailed manual procedure exists | Preferred script fails static semantic review; no fresh-host evidence artifact | Not reproducible |
| Grafana dashboards | Three JSON dashboards passed <code>jq empty</code> | Query names do not match exporter; no live scrape screenshot/query result | Structurally valid, operationally mostly blank |

Tool availability in this pass: Docker, Helm, kubectl, ShellCheck, and Terraform were unavailable. <code>bash -n</code> accepts the bootstrap script because the defect is semantically valid shell syntax; it does not validate the broken command-substitution argument/order.

## 10.C Prometheus/Grafana query audit

| Dashboard query | Exported by QantoMetrics? | Required action |
|---|---:|---|
| <code>qanto_block_height</code> | No | Instrument canonical/finalized height; do not substitute a cumulative block counter |
| <code>qanto_connected_peers</code> | No | Export authenticated connected-peer gauge |
| <code>rate(qanto_transactions_total[1m])</code> | No | Rename to <code>rate(qanto_transactions_processed_total[1m])</code> only if semantics match accepted transactions |
| <code>rate(qanto_blocks_mined_total[5m])</code> | No | Export mined/accepted blocks separately; processed and mining-attempt counters are not equivalent |
| <code>qanto_mempool_size</code> | Yes, at <code>metrics.rs:877-880</code> | Keep; add capacity/rejection labels |
| <code>qanto_hash_rate</code> | No | Use <code>qanto_hash_rate_hps</code> |
| <code>qanto_peer_rtt_seconds</code> | No | Add histogram by peer class without high-cardinality peer IDs |
| <code>qanto_sync_status</code> | No | Add bounded state gauge and sync-lag metric |
| <code>qanto_blockchain_height</code> | No | Same canonical-height instrumentation gap |
| <code>qanto_p2p_peers_connected</code> | No | Same connected-peer instrumentation gap |
| <code>rate(qanto_transactions_processed_total[5m])</code> | Yes | Query name matches; acceptance semantics still require validation |
| <code>container_*</code> panels | Not from node | Add cAdvisor to Compose/production monitoring or remove panels |

The scrape target/path itself is correct for development: <code>DevOps/docker/monitoring/prometheus/prometheus.yml:28-49</code> points at <code>qanto-node:8081/metrics/prometheus</code>, and the node handlers now await the exporter.

## 10.D Implemented operational fixes

### Root Docker context protection

<code>.dockerignore:1-65</code> was added. Its effective security categories are:

~~~diff
+# Local environment files; retain explicit example/template files
+**/.env
+**/.env.*
+!**/.env.example
+!**/.env.*.example
+
+# Build/dependency output
+target
+**/target
+node_modules
+**/node_modules
+
+# Local databases, node state, wallets, keys, and secrets
+**/*.db
+**/*.sqlite*
+/data/**
+/storage/**
+/Keystore/**
+**/keys/**
+**/wallets/**
+**/secrets/**
+**/*.key
+**/*.pem
~~~

This is important because <code>DevOps/docker/Dockerfile:22</code> copies the entire repository context.

### Async Prometheus export

~~~diff
diff --git a/Protocol_Core/qanto-node/src/metrics.rs b/Protocol_Core/qanto-node/src/metrics.rs
-    pub fn export_prometheus(&self) -> String {
+    pub async fn export_prometheus(&self) -> String {
+        let total_value_locked = *self.total_value_locked.read().await;
+        let validator_rewards_24h = *self.validator_rewards_24h.read().await;
+        let transaction_fees_24h = *self.transaction_fees_24h.read().await;
         ...
-            *self.total_value_locked.blocking_read()
+            total_value_locked
     }

diff --git a/Protocol_Core/qanto-node/src/metrics_server.rs b/Protocol_Core/qanto-node/src/metrics_server.rs
-    state.metrics.export_prometheus()
+    state.metrics.export_prometheus().await

diff --git a/Protocol_Core/qanto-node/src/node.rs b/Protocol_Core/qanto-node/src/node.rs
-    Ok(metrics.export_prometheus())
+    Ok(metrics.export_prometheus().await)
~~~

Verification:

- <code>cargo nextest run -p qanto metrics::tests::test_export_prometheus_reads_lock_backed_gauges_in_async_runtime</code>: 1 passed, 168 skipped.
- The regression test at <code>metrics.rs:1685-1697</code> executes inside Tokio and verifies all three lock-backed gauges.

### Canonical Nextest configuration

<code>.config/nextest.toml:4-47</code> now defines default, CI, and slow-integration profiles; mining/integration groups; and valid <code>test(...)</code> filter expressions. <code>cargo nextest show-config test-groups</code> exited 0, and the focused metrics test loaded the configuration successfully.

### Fail-closed public-integration configuration and metadata

- Hardhat imports <code>configVariable</code> at <code>SmartContracts/hardhat.config.js:1</code> and lazily requires <code>QANTO_TESTNET_RPC_URL</code> plus <code>QANTO_DEPLOYER_PRIVATE_KEY</code> at lines 26-32. Chain ID remains canonical at 1234. The configuration no longer imports dotenv, reads root <code>../.env</code>, or embeds an HF RPC URL.
- A fresh <code>npm test</code> compiled ten files with solc 0.8.24 and passed 38/38 without dotenv injection, demonstrating that local tests do not require or expose deployment credentials.
- <code>SmartContracts/cex-listing-payload.json:2-3</code> marks the payload <code>DRAFT_DO_NOT_SUBMIT</code>; lines 12-26 mark issuance unresolved; lines 28-44 use decimal chain ID 1234 and null public endpoints; lines 44-60 leave API endpoints/audits empty and disclaim an active integration.
- <code>README.md:340-360</code> keeps the old HF values only as visibly labeled legacy metadata and explicitly says the audited zero-budget topology is unsuitable for a public P2P blockchain.

These are correct containment/documentation fixes. They prevent local tests or draft metadata from silently selecting a public endpoint, but they deliberately do not claim that the dedicated public-testnet infrastructure exists.

## 10.E Deferred items

| Item | Why deferred | Estimated effort | Priority |
|---|---|---:|---|
| Repair and test Ubuntu bootstrap | Requires correcting script path/order, creating/chowning directories before clone, pinning a tag/commit, removing ignored pull failures, and testing twice on pristine Ubuntu 22.04 | 2-4 days | P0 |
| Build/publish signed node images | Requires CI build from pinned toolchain, SBOM, vulnerability scan, provenance/signature, immutable digest, and registry policy | 3-5 days | P0 |
| Consolidate raw K8s and Helm | Choose one canonical topology; add Secrets/external-secret support, per-node PVC/identity, Services, NetworkPolicies, PDB, anti-affinity, startup probes, ingress, and upgrade/rollback tests | 1-2 weeks | P0 |
| Put RPC behind a production edge | TLS for HTTP/gRPC/WSS, authentication where applicable, method/request-size limits, rate limiting, connection quotas, DDoS controls, and audit logging | 3-5 days | P0 |
| Reconcile dashboards/alerts | Add missing node metrics, correct PromQL, deploy exporters, define symptom-based alerts and runbooks, and verify live queries | 2-4 days | P0 |
| Finish HF migration | Preserve the current legacy warnings, lazy Hardhat variables, and draft/null exchange metadata until dedicated RPC/WSS/explorer endpoints are deployed; then verify DNS/TLS from two networks and populate the metadata only from release evidence | 2-3 days after infrastructure exists | P0 |
| Implement backup/restore drills | Quiesced or snapshot-consistent database backup, secrets/identity handling, encryption, checksums, off-host retention, restore automation, and measured RPO/RTO | 3-5 days plus recurring drill | P0 |
| Multi-node testnet soak | Requires fixed protocol blockers from other workstreams plus dedicated nodes, fault injection, disk-fill/restart/partition tests, and incident runbooks | 1-2 weeks minimum | P0 |

---

# Final verdict for workstreams 8-10

## **NOT READY**

Minimum conditions before a public testnet launch can be reconsidered:

1. Keep the repaired, receipt-verified, chain-ID-checked WQNTO deployment path, and remove or replace the fabricated <code>deploy_dex.js</code> and synthetic <code>deploy_phase11.js</code> release paths.
2. Keep QUSD, QantoSwap, bridge, and any sale/vesting claims disabled until each feature has a complete specification, implementation, invariant suite, and independent audit.
3. Preserve the clean 38/38 compile-and-test and zero-advisory npm gates, raise and enforce the measured 42.45%/43.36% line/statement coverage with full unit/integration/invariant tests, then add Slither and a second Solidity analyzer.
4. Retract or qualify the 10M+/32 BPS/sub-31 ms claims until a reproducible multi-node accepted-throughput artifact exists.
5. Keep dynamic sharding disabled; repair or delete fail-open performance helpers before any production wiring.
6. Produce one canonical deployable stack. The Ubuntu bootstrap, Compose edge, K8s/Helm secrets/storage/identity, image supply chain, and immutable release pin must pass on clean infrastructure.
7. Route public RPC/gRPC/WSS through a hardened edge and ship verified Prometheus/Grafana/alerting with query names matching real exporter metrics.
8. Complete the HF-to-dedicated migration while preserving draft/null fail-closed metadata until endpoints are independently verified, then perform a documented backup restore plus multi-node soak/failure drill.

These conditions cover only workstreams 8-10. Independent consensus, cryptographic, bridge, transaction-state, finality, and economic blockers documented by the other audit tracks remain cumulative.
