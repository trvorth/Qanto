---
id: wallet-setup
title: How to Set Up Your Qanto Wallet
sidebar_label: Wallet Setup
description: Step-by-step guide to creating and securing your Qanto wallet
tags: [tutorial, wallet, beginner, security]
---

# How to Set Up Your Qanto Wallet

Welcome to Qanto! This comprehensive guide will walk you through creating your first Qanto wallet and securing your digital assets with industry-leading post-quantum cryptography.

## 🎯 What You'll Learn

By the end of this tutorial, you'll know how to:
- ✅ Download and install the official Qanto wallet
- ✅ Create a new wallet with quantum-resistant security
- ✅ Backup and secure your seed phrase
- ✅ Receive your first QNTO tokens
- ✅ Connect to both mainnet and testnet

## 📱 Choose Your Wallet Type

Qanto offers several wallet options to fit your needs:

### Desktop Wallet (Recommended for Beginners)

<div class="feature-card">
<h4>🖥️ Qanto Desktop Wallet</h4>

**Features:**
- Full node capability
- Advanced security features  
- SAGA AI integration
- Built-in staking interface
- Multi-signature support

**Requirements:**
- Windows 10+, macOS 10.15+, or Linux Ubuntu 18.04+
- 4GB RAM minimum (8GB recommended)
- 50GB free disk space
- Internet connection

</div>

### Mobile Wallet

<div class="feature-card">
<h4>📱 Qanto Mobile Wallet</h4>

**Features:**
- Lightweight SPV client
- Biometric authentication
- QR code scanning
- Push notifications
- Cross-platform sync

**Available on:**
- iOS 13.0+ (App Store)
- Android 8.0+ (Google Play)

</div>

### Browser Extension

<div class="feature-card">
<h4>🌐 Qanto Browser Extension</h4>

**Features:**
- DApp integration
- One-click transactions
- Hardware wallet support
- Multiple account management

**Supported Browsers:**
- Chrome/Brave/Edge
- Firefox
- Safari (coming soon)

</div>

## 🚀 Step-by-Step Wallet Setup

### Step 1: Download the Official Wallet

:::warning Security Alert
Always download wallets from official sources to avoid malicious software.
:::

1. Visit [wallet.qanto.org](https://wallet.qanto.org)
2. Select your operating system
3. Verify the download signature:

```bash
# Verify the signature (Linux/macOS)
gpg --verify qanto-wallet-2.0.0-linux.AppImage.sig qanto-wallet-2.0.0-linux.AppImage

# Check SHA256 hash
sha256sum qanto-wallet-2.0.0-linux.AppImage
# Expected: a1b2c3d4e5f6...
```

### Step 2: Install the Wallet

#### Windows Installation

1. Run `qanto-wallet-2.0.0-windows.exe`
2. Follow the installation wizard
3. Choose installation directory
4. Complete the installation
5. Launch Qanto Wallet from Start Menu

#### macOS Installation

1. Open `qanto-wallet-2.0.0-macos.dmg`
2. Drag Qanto Wallet to Applications folder
3. Right-click and select "Open" (first time only)
4. Allow the app to run in System Preferences if prompted

#### Linux Installation

1. Make the AppImage executable:
   ```bash
   chmod +x qanto-wallet-2.0.0-linux.AppImage
   ```
2. Run the wallet:
   ```bash
   ./qanto-wallet-2.0.0-linux.AppImage
   ```

### Step 3: Create Your First Wallet

1. **Launch the wallet application**
2. **Select "Create New Wallet"**
3. **Choose network**:
   - **Mainnet**: For real transactions with real QNTO
   - **Testnet**: For testing and learning (free test tokens)

<div class="highlight-box">
<h4>🎓 Pro Tip for Beginners</h4>

Start with **testnet** to learn without risk! You can always create a mainnet wallet later.

</div>

4. **Set a strong password**:
   - Minimum 12 characters
   - Include uppercase, lowercase, numbers, and symbols
   - Use a password manager if possible

5. **Generate your seed phrase**:

The wallet will display 24 random words. This is your **seed phrase** (also called a mnemonic phrase).

```
abandon ability able about above absent absorb abstract absurd
abuse access accident account accuse achieve acid acoustic acquire
across act action actor actual adapt add address adjust admit
```

:::danger Critical Security Step
Your seed phrase is the master key to your wallet. Anyone with these 24 words can access your funds. Never share it online or store it digitally.
:::

### Step 4: Secure Your Seed Phrase

#### Physical Backup (Recommended)

1. **Write down all 24 words in order** on paper
2. **Double-check each word** for spelling errors
3. **Store in a secure location**:
   - Fireproof safe
   - Bank safety deposit box  
   - Multiple secure locations (recommended)

#### Advanced Security Options

<div class="feature-card">
<h4>🛡️ Steel Backup Plates</h4>

For maximum security, engrave your seed phrase on:
- Stainless steel plates
- Titanium backups
- Cryptosteel devices

These survive fires, floods, and other disasters.

</div>

<div class="feature-card">
<h4>🔐 Shamir's Secret Sharing</h4>

Split your seed phrase into multiple shares:
- Requires X out of N shares to recover
- Distribute shares to trusted parties
- Advanced users only

</div>

### Step 5: Verify Your Backup

1. **Close your wallet**
2. **Reopen and select "Restore Wallet"**  
3. **Enter your 24-word seed phrase**
4. **Confirm your wallet restores correctly**

This verification step is crucial - many users have lost funds by not properly testing their backups!

### Step 6: Configure Your Wallet

#### Security Settings

1. **Enable 2FA** (if available):
   - Use Google Authenticator or Authy
   - Store backup codes securely

2. **Set up biometric authentication** (mobile):
   - Fingerprint unlock
   - Face ID (iOS)

3. **Configure auto-lock**:
   - Set to 5-15 minutes for active use
   - 1-5 minutes for maximum security

#### Network Settings

1. **Choose your preferred RPC endpoint**:
   - Automatic (recommended for beginners)
   - Custom RPC for advanced users

2. **Enable testnet mode** for learning:
   ```json
   {
     "network": "testnet",
     "rpc_endpoint": "https://testnet-rpc.qanto.org",
     "explorer": "https://testnet-explorer.qanto.org"
   }
   ```

## 🎁 Get Your First Tokens

### Mainnet QNTO

Purchase QNTO from supported exchanges:
- **Major Exchanges**: Binance, Coinbase, Kraken
- **DEX Platforms**: QantoSwap, Uniswap (wrapped QNTO)
- **Direct Purchase**: MoonPay, Simplex (in-wallet)

### Testnet Tokens (Free!)

Get free testnet tokens from our faucet:

1. **Visit**: [faucet.qanto.org](https://faucet.qanto.org)
2. **Enter your testnet address**
3. **Complete the captcha**
4. **Receive 100 test QNTO** instantly

```bash
# Command line faucet (advanced)
curl -X POST "https://faucet.qanto.org/api/request" \
  -H "Content-Type: application/json" \
  -d '{"address": "qanto1your_address_here"}'
```

## 📧 Your First Transaction

Let's send your first Qanto transaction:

### Step 1: Find Your Address

Your Qanto address looks like this:
```
qanto1abc123def456ghi789jkl012mno345pqr678stu
```

- **Starts with "qanto1"**
- **43 characters total**
- **Case-sensitive**

### Step 2: Send Tokens

1. **Click "Send"** in your wallet
2. **Enter recipient address**
3. **Specify amount** (minimum 0.000001 QNTO)
4. **Review the fee** (typically ~0.001 QNTO)
5. **Confirm and sign** the transaction

### Step 3: Track Your Transaction

After sending, you'll get a transaction hash:
```
0x1a2b3c4d5e6f7890abcdef1234567890abcdef1234567890abcdef1234567890
```

Track it on the explorer:
- **Mainnet**: [explorer.qanto.org](https://explorer.qanto.org)
- **Testnet**: [testnet-explorer.qanto.org](https://testnet-explorer.qanto.org)

## 🎬 Video Tutorial

<div class="video-container">
<iframe 
  width="560" 
  height="315" 
  src="https://www.youtube.com/embed/dQw4w9WgXcQ" 
  title="Qanto Wallet Setup Tutorial" 
  frameborder="0" 
  allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" 
  allowfullscreen>
</iframe>
</div>

Watch our comprehensive video tutorial covering all the steps in this guide.

## 🔧 Advanced Features

### Hardware Wallet Integration

Support for popular hardware wallets:

- **Ledger Nano S/X**: Connect via USB or Bluetooth
- **Trezor Model T**: Full Qanto app support
- **KeepKey**: Basic transaction signing

### Multi-Signature Wallets

Create wallets requiring multiple signatures:

```bash
# Create 2-of-3 multisig wallet
qanto-cli wallet create-multisig \
  --threshold 2 \
  --keys "pubkey1,pubkey2,pubkey3"
```

### Custom RPC Endpoints

Connect to your own node:

```json
{
  "rpc_endpoint": "http://localhost:8545",
  "websocket_endpoint": "ws://localhost:8546"
}
```

## 🚨 Security Best Practices

### Do's ✅

- ✅ **Always verify download signatures**
- ✅ **Use strong, unique passwords**
- ✅ **Enable 2FA when available**
- ✅ **Keep software updated**
- ✅ **Test backups regularly**
- ✅ **Use hardware wallets for large amounts**
- ✅ **Keep seed phrases offline**

### Don'ts ❌

- ❌ **Never share your seed phrase**
- ❌ **Don't store seeds digitally**
- ❌ **Avoid public Wi-Fi for transactions**
- ❌ **Don't ignore security updates**
- ❌ **Never send funds to unverified addresses**
- ❌ **Don't use obvious passwords**

### Common Scams to Avoid

<div class="highlight-box">
<h4>⚠️ Beware of These Scams</h4>

1. **Fake wallet apps**: Only download from official sources
2. **Phishing websites**: Always check URLs carefully
3. **Seed phrase requests**: Legitimate support never asks for seeds
4. **Too-good-to-be-true offers**: No guaranteed returns in crypto
5. **Fake airdrops**: Don't connect wallets to suspicious sites

</div>

## 🆘 Troubleshooting

### Common Issues

#### "Connection Failed"
```bash
# Solution: Check your internet connection and try different RPC
```

#### "Insufficient Funds for Fee"
```bash
# Solution: You need at least 0.001 QNTO for transaction fees
```

#### "Invalid Address Format"
```bash
# Solution: Qanto addresses must start with "qanto1" and be 43 characters
```

#### "Transaction Stuck"
```bash
# Solution: Check network congestion, consider higher fee
```

### Recovery Options

If you lose access to your wallet:

1. **Seed phrase recovery**: Use your 24 words
2. **Password reset**: Only works if you have seed phrase
3. **Wallet file backup**: Import your wallet.dat file
4. **Hardware wallet recovery**: Use device's recovery process

## 📞 Get Help

Need assistance? Our community is here to help:

- **Discord**: [discord.gg/qanto](https://discord.gg/qanto) - Real-time chat support
- **Telegram**: [t.me/qantosupport](https://t.me/qantosupport) - Community help
- **Email**: [support@qanto.org](mailto:support@qanto.org) - Official support
- **Forum**: [forum.qanto.org](https://forum.qanto.org) - Technical discussions

## 🎯 Next Steps

Congratulations! You've successfully set up your Qanto wallet. Here's what to explore next:

1. **[Understanding Fees](understanding-fees.md)** - Learn about Qanto's dynamic fee system
2. **[First Transaction](first-transaction.md)** - Make your first real transaction
3. **[Backup & Restore](backup-restore.md)** - Master wallet recovery techniques
4. **[Staking Guide](../advanced/staking-guide.md)** - Earn rewards by staking QNTO

---

*This tutorial was last updated for Qanto Wallet v2.0.0. Always ensure you're using the latest version for the best security and features.*
