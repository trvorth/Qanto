import { useState, useEffect, useRef } from 'react';
import { toast } from 'react-hot-toast';
import { useQantoBalance } from '../hooks/useQantoBalance';
import { privateKeyToAccount } from 'viem/accounts';
import { keccak256, stringToBytes } from 'viem';
import {
  useAccount,
  useConnectModal,
  useSendTransaction,
  useWaitForTransactionReceipt,
} from '../lib/qanto-wallet';

const BRIDGE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000013';
const REST_BASE = 'https://trvorth-qanto-testnet.hf.space';

// Helper functions for hex conversion
const hexToBytes = (hex: string): Uint8Array => {
  const cleanHex = hex.trim().startsWith('0x') ? hex.trim().substring(2) : hex.trim();
  const bytes = [];
  for (let c = 0; c < cleanHex.length; c += 2) {
    bytes.push(parseInt(cleanHex.substring(c, c + 2), 16));
  }
  return new Uint8Array(bytes);
};

const bytesToHex = (bytes: Uint8Array): string => {
  const hex = [];
  for (let i = 0; i < bytes.length; i++) {
    let current = bytes[i].toString(16);
    if (current.length < 2) {
      current = "0" + current;
    }
    hex.push(current);
  }
  return hex.join("");
};

const encodeBridgeLockData = (targetChain: string, recipientAddress: string): string => {
  const chainBytes = new TextEncoder().encode(targetChain);
  const chainLen = chainBytes.length;
  
  const recipientBytes = hexToBytes(recipientAddress);
  
  const dataBytes = new Uint8Array(2 + chainLen + recipientBytes.length);
  dataBytes[0] = 1; // prefix [1]
  dataBytes[1] = chainLen; // chain length
  dataBytes.set(chainBytes, 2);
  dataBytes.set(recipientBytes, 2 + chainLen);
  
  return '0x' + bytesToHex(dataBytes);
};

const encodeBridgeClaimData = (payload: any): string => {
  const jsonString = JSON.stringify(payload);
  const jsonBytes = new TextEncoder().encode(jsonString);
  
  const dataBytes = new Uint8Array(1 + jsonBytes.length);
  dataBytes[0] = 2; // prefix [2]
  dataBytes.set(jsonBytes, 1);
  
  return '0x' + bytesToHex(dataBytes);
};

export const Bridge = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();

  const [activeTab, setActiveTab] = useState<'lock' | 'claim'>('lock');
  const [fromChain, setFromChain] = useState('Qanto Testnet');
  const [toChain, setToChain] = useState('Ethereum Sepolia');
  const [amount, setAmount] = useState('500');
  const [recipient, setRecipient] = useState('');
  const { confirmed: balance, refresh: refreshBalance } = useQantoBalance();

  // Claim specific fields
  const [ethTxHash, setEthTxHash] = useState('');
  const [blockHeader, setBlockHeader] = useState('');
  const [merkleProof, setMerkleProof] = useState('');
  const [receiptProof, setReceiptProof] = useState('');
  const [relayerSigs, setRelayerSigs] = useState('');

  // Metrics
  const [bridgeStats, setBridgeStats] = useState({ total_locked: '0', total_claimed: '0' });

  const { sendTransaction, data: txHash, isPending: isWritePending, error: writeError } = useSendTransaction();
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  // Fetch bridge metrics from REST API
  const fetchBridgeStats = async () => {
    try {
      const res = await fetch(`${REST_BASE}/bridge`);
      if (res.ok) {
        const data = await res.json();
        if (data.total_locked) {
          setBridgeStats({
            total_locked: data.total_locked,
            total_claimed: data.total_claimed,
          });
        }
      }
    } catch (err) {
      console.error('Failed to fetch bridge stats', err);
    }
  };

  useEffect(() => {
    fetchBridgeStats();
    const interval = setInterval(fetchBridgeStats, 10000);
    return () => clearInterval(interval);
  }, []);

  // Sync default recipient address with connected wallet address
  useEffect(() => {
    if (address && !recipient) {
      setRecipient(address);
    }
  }, [address, recipient]);

  // Generate test claim proof (testnet/development mode only)
  const handleGenerateTestClaim = async () => {
    if (!address) {
      toast.error('Connect wallet to generate test claim.');
      return;
    }
    if (!amount || parseFloat(amount) <= 0 || isNaN(parseFloat(amount))) {
      toast.error('Please enter a valid amount.');
      return;
    }

    const testTxHash = '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
    setEthTxHash(testTxHash);

    const testBlockHeader = '0x0000000000000000000000000000000000000000000000000000000000000001';
    setBlockHeader(testBlockHeader);

    const testMerkleProof = '0x0000000000000000000000000000000000000000000000000000000000000002';
    setMerkleProof(testMerkleProof);

    const testReceiptProof = '0x0000000000000000000000000000000000000000000000000000000000000003';
    setReceiptProof(testReceiptProof);

    // Build the message data to sign
    const chainBytes = stringToBytes(fromChain);
    const txHashBytes = stringToBytes(testTxHash);
    const amountVal = Math.floor(parseFloat(amount) * 10 ** 18).toString();
    const amountBytes = stringToBytes(amountVal);
    const recipientBytes = stringToBytes(address);

    const totalBytes = new Uint8Array(chainBytes.length + txHashBytes.length + amountBytes.length + recipientBytes.length);
    totalBytes.set(chainBytes, 0);
    totalBytes.set(txHashBytes, chainBytes.length);
    totalBytes.set(amountBytes, chainBytes.length + txHashBytes.length);
    totalBytes.set(recipientBytes, chainBytes.length + txHashBytes.length + amountBytes.length);

    // Compute Keccak256 hash of message
    const msgHash = keccak256(totalBytes);

    // Standard Hardhat dev private key #0
    const devPrivKey = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80';
    try {
      const account = privateKeyToAccount(devPrivKey);
      const signature = await account.sign({ hash: msgHash });
      setRelayerSigs(signature);
      toast.success('Test claim proof generated successfully for testnet!');
    } catch (e) {
      toast.error('Failed to generate test signature: ' + (e as Error).message);
    }
  };

  const handleBridge = () => {
    if (!isConnected) {
      if (openConnectModal) {
        openConnectModal();
      } else {
        toast.error('Connect wallet to bridge tokens.');
      }
      return;
    }

    if (!amount || parseFloat(amount) <= 0 || isNaN(parseFloat(amount))) {
      toast.error('Please enter a valid amount.');
      return;
    }

    if (!recipient || !recipient.startsWith('0x') || recipient.length !== 42) {
      toast.error('Please enter a valid recipient EVM address.');
      return;
    }

    toastId.current = toast.loading('Initiating cross-chain bridge transfer...');

    if (activeTab === 'lock') {
      const lockData = encodeBridgeLockData(toChain, recipient);
      sendTransaction({
        to: BRIDGE_CONTRACT_ADDRESS as `0x${string}`,
        value: BigInt(Math.floor(parseFloat(amount) * 10 ** 18)),
        data: lockData as `0x${string}`,
      });
    } else {
      if (!ethTxHash || !blockHeader || !merkleProof || !receiptProof || !relayerSigs) {
        toast.dismiss(toastId.current!);
        toastId.current = null;
        toast.error('Please complete all proof parameters for claiming.');
        return;
      }

      const claimPayload = {
        source_tx_hash: ethTxHash,
        amount: Math.floor(parseFloat(amount) * 10 ** 18).toString(),
        recipient: recipient,
        source_chain: fromChain,
        relayer_signatures: relayerSigs.split(',').map(s => s.trim()),
        merkle_proof: merkleProof,
        receipt_proof: receiptProof,
        block_header: blockHeader,
      };

      const claimData = encodeBridgeClaimData(claimPayload);
      sendTransaction({
        to: BRIDGE_CONTRACT_ADDRESS as `0x${string}`,
        value: 0n,
        data: claimData as `0x${string}`,
      });
    }
  };

  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      toast.success(activeTab === 'lock' ? 'Cross-chain Lock Confirmed!' : 'Cross-chain Claim Confirmed!');
      refreshBalance();
      fetchBridgeStats();
    }
  }, [isConfirmed, refreshBalance]);

  useEffect(() => {
    if (writeError) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      toast.error(`Bridge failed: ${writeError.message || 'Transaction rejected'}`);
    }
  }, [writeError]);

  const isPending = isWritePending || isConfirming;
  const isBridgeSuccess = isConfirmed;

  let bridgeButtonText = activeTab === 'lock' ? 'INITIATE LOCK & DEPOSIT' : 'EXECUTE CLAIM';
  if (!isConnected) {
    bridgeButtonText = 'Connect Wallet';
  } else if (isPending) {
    bridgeButtonText = 'Transferring...';
  } else if (isBridgeSuccess) {
    bridgeButtonText = 'Transfer Confirmed ✅';
  }

  // Detect testnet mode to restrict Test Claim Generator
  const isTestnetMode = fromChain.includes('Testnet') || fromChain.includes('Sepolia');

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-[0_0_50px_rgba(139,92,246,0.1)] relative overflow-hidden">
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(139,92,246,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '30s' }} />

        <div className="relative z-10">
          <h1 className="text-3xl font-extrabold text-white text-center mb-2 font-sans tracking-tight">
            Quantum Lattice Bridge
          </h1>
          <p className="text-sm text-slate-400 text-center font-sans mb-6">
            Bridge your $QNTO assets securely with full cryptographic proof validation.
          </p>

          {/* Tab Selector */}
          <div className="flex bg-black/40 border border-white/10 rounded-2xl p-1 mb-6 relative z-10">
            <button
              onClick={() => {
                setActiveTab('lock');
                setFromChain('Qanto Testnet');
                setToChain('Ethereum Sepolia');
              }}
              disabled={isPending}
              className={`flex-1 py-2.5 text-xs md:text-sm font-bold rounded-xl transition-all duration-300 ${
                activeTab === 'lock'
                  ? 'bg-cyan-500 text-white shadow-[0_0_15px_rgba(6,182,212,0.4)]'
                  : 'text-slate-400 hover:text-white'
              } ${isPending ? 'opacity-50 cursor-not-allowed' : ''}`}
            >
              LOCK / DEPOSIT
            </button>
            <button
              onClick={() => {
                setActiveTab('claim');
                setFromChain('Ethereum Sepolia');
                setToChain('Qanto Testnet');
              }}
              disabled={isPending}
              className={`flex-1 py-2.5 text-xs md:text-sm font-bold rounded-xl transition-all duration-300 ${
                activeTab === 'claim'
                  ? 'bg-cyan-500 text-white shadow-[0_0_15px_rgba(6,182,212,0.4)]'
                  : 'text-slate-400 hover:text-white'
              } ${isPending ? 'opacity-50 cursor-not-allowed' : ''}`}
            >
              CLAIM
            </button>
          </div>

          <div className="space-y-4 mb-6">
            {/* Chain Selection Row */}
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase">From</span>
                {activeTab === 'lock' ? (
                  <span className="text-sm font-bold font-mono text-white p-0">Qanto Testnet</span>
                ) : (
                  <select
                    value={fromChain}
                    onChange={(e) => setFromChain(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-sm font-bold font-mono text-white outline-none border-none p-0 cursor-pointer focus:ring-0"
                  >
                    <option value="Ethereum Sepolia" className="bg-slate-900">Ethereum Sepolia</option>
                    <option value="Arbitrum Sepolia" className="bg-slate-900">Arbitrum Sepolia</option>
                  </select>
                )}
              </div>

              <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase">To</span>
                {activeTab === 'lock' ? (
                  <select
                    value={toChain}
                    onChange={(e) => setToChain(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-sm font-bold font-mono text-white outline-none border-none p-0 cursor-pointer focus:ring-0"
                  >
                    <option value="Ethereum Sepolia" className="bg-slate-900">Ethereum Sepolia</option>
                    <option value="Arbitrum Sepolia" className="bg-slate-900">Arbitrum Sepolia</option>
                  </select>
                ) : (
                  <span className="text-sm font-bold font-mono text-white p-0">Qanto Testnet</span>
                )}
              </div>
            </div>

            {/* Input Amount */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div className="flex justify-between text-xs text-slate-500 font-mono">
                <span>Amount to Bridge</span>
                <span>Available: {isConnected ? `${balance.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })} QNTO` : '0.00 QNTO'}</span>
              </div>
              <div className="flex justify-between items-center gap-4">
                <input
                  type="number"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  disabled={isPending || isBridgeSuccess}
                  className="bg-transparent text-2xl font-bold font-mono text-white outline-none border-none w-full p-0 focus:ring-0"
                  placeholder="0.0"
                />
                <span className="bg-white/5 border border-white/10 text-white font-bold font-mono px-3.5 py-1.5 rounded-xl text-sm select-none">
                  QNTO
                </span>
              </div>
            </div>

            {/* Recipient Address */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <span className="text-xs text-slate-500 font-mono">Recipient Address</span>
              <input
                type="text"
                value={recipient}
                onChange={(e) => setRecipient(e.target.value)}
                disabled={isPending || isBridgeSuccess}
                className="bg-transparent text-sm font-mono text-white outline-none border-none w-full p-0 focus:ring-0"
                placeholder="0x..."
              />
            </div>

            {/* Claim Specific Fields */}
            {activeTab === 'claim' && (
              <div className="space-y-4 border-t border-white/5 pt-4">
                <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
                  <div className="flex justify-between items-center">
                    <span className="text-xs text-slate-500 font-mono">Source Transaction Hash</span>
                    {isTestnetMode && (
                      <button
                        onClick={handleGenerateTestClaim}
                        disabled={isPending}
                        className="text-[10px] font-mono text-cyan-400 hover:text-cyan-300 font-semibold underline"
                      >
                        [Generate Test Claim / Proof]
                      </button>
                    )}
                  </div>
                  <input
                    type="text"
                    value={ethTxHash}
                    onChange={(e) => setEthTxHash(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-sm font-mono text-white outline-none border-none w-full p-0 focus:ring-0 text-slate-300"
                    placeholder="0x..."
                  />
                </div>

                <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
                  <span className="text-xs text-slate-500 font-mono">Block Header (Hex)</span>
                  <input
                    type="text"
                    value={blockHeader}
                    onChange={(e) => setBlockHeader(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-xs font-mono text-white outline-none border-none w-full p-0 focus:ring-0 text-slate-300"
                    placeholder="0x..."
                  />
                </div>

                <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
                  <span className="text-xs text-slate-500 font-mono">Merkle Proof (Hex)</span>
                  <input
                    type="text"
                    value={merkleProof}
                    onChange={(e) => setMerkleProof(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-xs font-mono text-white outline-none border-none w-full p-0 focus:ring-0 text-slate-300"
                    placeholder="0x..."
                  />
                </div>

                <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
                  <span className="text-xs text-slate-500 font-mono">Receipt Proof (Hex)</span>
                  <input
                    type="text"
                    value={receiptProof}
                    onChange={(e) => setReceiptProof(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-xs font-mono text-white outline-none border-none w-full p-0 focus:ring-0 text-slate-300"
                    placeholder="0x..."
                  />
                </div>

                <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
                  <span className="text-xs text-slate-500 font-mono">Relayer Signatures</span>
                  <textarea
                    value={relayerSigs}
                    onChange={(e) => setRelayerSigs(e.target.value)}
                    disabled={isPending || isBridgeSuccess}
                    className="bg-transparent text-xs font-mono text-white outline-none border-none w-full p-0 focus:ring-0 text-slate-300 min-h-[60px] resize-none"
                    placeholder="Comma-separated relayer signatures (0x...)"
                  />
                </div>
              </div>
            )}
          </div>

          {/* Details */}
          <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mb-8 text-xs font-mono text-slate-400 space-y-2">
            <div className="flex justify-between">
              <span>Bridge Fee:</span>
              <span className="text-emerald-400">0.00 QNTO</span>
            </div>
            <div className="flex justify-between">
              <span>Total Network Locked:</span>
              <span className="text-cyan-400">{parseFloat(bridgeStats.total_locked).toLocaleString()} QNTO</span>
            </div>
            <div className="flex justify-between">
              <span>Total Network Claimed:</span>
              <span className="text-cyan-400">{parseFloat(bridgeStats.total_claimed).toLocaleString()} QNTO</span>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleBridge}
              disabled={isPending || (isConnected && isBridgeSuccess)}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected
                  ? 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
                  : isPending
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : isBridgeSuccess
                  ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white hover:shadow-[0_0_20px_rgba(6,182,212,0.6)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {bridgeButtonText}
            </button>

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all text-center">
                <div>Source Wallet: {address}</div>
                {txHash && <div className="text-cyan-400">Bridge Tx Hash: {txHash}</div>}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
export default Bridge;
