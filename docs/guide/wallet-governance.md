# Qanto Wallet & Governance Guide (Phase 3)

This guide covers the advanced features of the **NEURAL-VAULT™** (Qanto Wallet), including Hierarchical Deterministic (HD) wallet derivation and participation in the **ΛΣ-ΩMEGA** governance protocol.

## 1. Wallet Setup and Management

### Generating a Master Wallet
To create a new secure wallet (Master Vault):

```bash
cargo run --release --bin qantowallet -- generate --output master.wallet
```

You will be prompted to create a secure password. **Backup your mnemonic phrase** securely using:

```bash
cargo run --release --bin qantowallet -- show --wallet master.wallet --keys
```

### HD Wallet Derivation (New!)
Qanto now supports deterministic wallet derivation. You can generate unlimited child wallets from your master wallet's mnemonic without needing new backups.

**Command:** `derive`

To derive the child wallet at index `1` and display its address:
```bash
cargo run --release --bin qantowallet -- derive --wallet master.wallet --index 1
```

To derive and save it to a new file (e.g., for a specific application or user):
```bash
cargo run --release --bin qantowallet -- derive --wallet master.wallet --index 1 --save child_1.wallet
```

**Scheme:** `Child_Seed = HMAC-SHA256(Root_Seed, "QANTO_HD" || Index)`

## 2. Governance Participation

Participate in the **Orchestrated Multi-dimensional Execution & Governance Architecture (ΩMEGA)** directly from your CLI.

### Submitting a Proposal
Propose changes to the network, parameter updates, or ecosystem initiatives.

**Command:** `governance propose`

```bash
cargo run --release --bin qantowallet -- governance propose \
  --title "Increase Block Size" \
  --description "Proposal to increase block size to 2MB for higher throughput" \
  --security-impact "Medium" \
  --wallet master.wallet
```

**Parameters:**
- `--title`: Short title of the proposal.
- `--description`: Detailed description.
- `--security-impact`: Assessed impact (`None`, `Low`, `Medium`, `High`, `Critical`). High/Critical proposals require **ΩMEGA Reflection** approval.
- `--execution-code`: (Optional) Rust/WASM code payload for automatic execution.

### Voting on Proposals
Cast your vote on active proposals using your staked QAN.

**Command:** `governance vote`

```bash
cargo run --release --bin qantowallet -- governance vote \
  --proposal-id 105 \
  --vote "yes" \
  --wallet master.wallet
```

**Options:** `yes`, `no`, `abstain`.

## 3. Security Best Practices

1.  **Cold Storage**: Keep your `master.wallet` offline. Use derived child wallets for daily operations with limited funds.
2.  **Mnemonic Safety**: Your mnemonic phrase is the root of all derived keys. Never share it or type it into unverified software.
3.  **Password Strength**: Use a strong, unique password for each wallet file.
4.  **Verification**: Always verify the `security-impact` of proposals before voting. High-impact proposals can alter consensus rules.

## 4. Troubleshooting

- **"Wallet file not found"**: Ensure you provided the correct path to `--wallet`.
- **"Invalid password"**: Passwords are case-sensitive. If you lost your password, recover using `import --mnemonic`.
- **"Proposal rejected"**: If the ΩMEGA protocol rejects your proposal immediately, check the logs for "Security risk assessment failed". You may need to lower the security impact or provide better justification.
