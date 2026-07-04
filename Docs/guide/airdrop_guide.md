# QANTO Layer-0 Public Testnet Alpha: ZK-Airdrop Onboarding Guide 🌐

Welcome to the **QANTO Singularity**. QANTO is the world's first post-quantum, AI-governed blockchain infrastructure, capable of delivering over 10M TPS through its lock-free Rust architecture and Synaptic DAG ledger. 

This guide will walk you through setting up your wallet, connecting to the **Omni-Chain Quantum Aura**, viewing the Token Generation Event (TGE) dashboard, and executing your Zero-Knowledge (ZK) claim to claim your `$QNTO` allocation.

---

## 🛠️ Step 1: Install and Configure MetaMask

To interact with the QANTO Layer-0 network, you need an EVM-compatible Web3 wallet. We recommend **MetaMask** (available for Chrome, Firefox, iOS, and Android).

1. Download and install MetaMask from [metamask.io](https://metamask.io).
2. Create a new wallet or import your existing seed phrase.
3. Open MetaMask and click on the network selection dropdown in the top-left corner.
4. Click **Add Network** → **Add a network manually**.
5. Enter the following network parameters to connect to the QANTO Public Testnet Alpha mesh:

| Parameter | Value |
| :--- | :--- |
| **Network Name** | QANTO Public Testnet Alpha |
| **New RPC URL** | `https://rpc.qanto.org` (or fallback: `https://trvorth-qanto-testnet.hf.space/rpc`) |
| **Chain ID** | `21313` (Hex: `0x5341` - the SAGA neural signature) |
| **Currency Symbol** | `QNTO` |
| **Block Explorer URL** | `https://qanto.org/explorer` |

6. Click **Save** and switch your network to **QANTO Public Testnet Alpha**.

---

## 📊 Step 2: Access the TGE Dashboard

The Token Generation Event (TGE) marks the official launch of the `$QNTO` token. 

1. Navigate to the official QANTO Portal: [qanto.org](https://qanto.org) or [qanto.org/airdrop](https://qanto.org/airdrop).
2. Click **Connect Wallet** in the top right corner.
3. Approve the connection request in MetaMask. 
4. Once connected, your wallet address will be visible on the HUD, and your **Quantum Aura** connection status will turn green, indicating active telemetry sync with the SAGA AI Engine.
5. In the **TGE Console**, you will see:
   - **Total Circulating Supply:** Real-time counter approaching the 21B QNTO hard cap.
   - **Epoch Progress:** Current consensus period and block height.
   - **Your Genesis Allocation:** The amount of `$QNTO` pre-allocated to your address based on your testnet participation or developer mesh contributions.

---

## 🔒 Step 3: Execute the Zero-Knowledge (ZK) Claim

To protect user privacy and secure our distribution against quantum attack vectors, all airdrop distributions are executed via a local **Zero-Knowledge Merkle Proof**. Your MetaMask will sign a cryptographic hash, allowing the SAGA AI to verify your allocation without exposing your address metadata.

1. Locate the **Genesis Airdrop** card on the airdrop page.
2. Verify that your **Genesis Allocation** shows your eligible `$QNTO` balance (e.g., `1,000 QNTO`).
3. Click the **Execute Zero-Knowledge Claim** button.
4. Your browser will generate a local cryptographic Merkle Proof. You will see the status update to:
   *`Generating Merkle Proof...`*
5. MetaMask will open a popup window requesting a signature. Review the transaction and click **Confirm**.
   *Note: Since QANTO features sub-second finality, this transaction will confirm almost instantly.*
6. Once processed, you will see a success message:
   *`Claim Successful ✅ 1,000 $QNTO deposited to your wallet.`*
7. Verify your balance directly in MetaMask or inspect the transaction hash on the [QANTO Block Explorer](https://qanto.org/explorer).

---

## 💎 Step 4: Wrap your QNTO (Optional)

If you wish to provide liquidity on Uniswap or engage in QantoSwap yield-farming, you can wrap your native `$QNTO` into `$WQNTO` (Wrapped QANTO), which conforms to the ERC-20 standard with 6 decimals.

1. Go to [qanto.org/ecosystem](https://qanto.org/ecosystem) and open **QantoSwap**.
2. Select **QNTO** → **WQNTO** swap path.
3. Enter the amount you want to wrap, click **Swap/Wrap**, and sign the MetaMask transaction.
4. To import the `$WQNTO` token into MetaMask so it appears in your assets list:
   - Contract Address: `0x9F00000000000000000000000000000000000001`
   - Token Symbol: `WQNTO`
   - Token Decimals: `6`

---

## 🚨 Security Warning

> [!WARNING]
> Official QANTO team members will **never** DM you first, ask for your seed phrase, or require you to send funds to claim the airdrop. Make sure you are always visiting the verified domain [https://qanto.org](https://qanto.org).
