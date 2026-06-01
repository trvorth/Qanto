import { useState, useEffect, useRef } from 'react';
import { useAccount, useWriteContract, useWaitForTransactionReceipt } from 'wagmi';
import { useConnectModal } from '@rainbow-me/rainbowkit';
import { toast } from 'react-hot-toast';

const BRIDGE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000013';

const QantoBridgeAbi = [
  {
    type: 'function',
    name: 'bridgeTokens',
    stateMutability: 'payable',
    inputs: [
      { name: 'destinationChainId', type: 'uint32' },
      { name: 'recipient', type: 'address' },
      { name: 'amount', type: 'uint256' }
    ],
    outputs: []
  }
] as const;

export const Bridge = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();

  const [fromChain, setFromChain] = useState('Qanto Testnet');
  const [toChain, setToChain] = useState('Ethereum Sepolia');
  const [amount, setAmount] = useState('500');
  const [recipient, setRecipient] = useState('');

  const { writeContract, data: txHash, isPending: isWritePending, error: writeError } = useWriteContract();
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  // Sync default recipient address with connected wallet address
  useEffect(() => {
    if (address && !recipient) {
      setRecipient(address);
    }
  }, [address, recipient]);

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

    // Dest chains: 1 = Sepolia, 2 = Arbitrum Sepolia
    const destChainId = toChain.includes('Ethereum') ? 1 : 2;

    writeContract({
      abi: QantoBridgeAbi,
      address: BRIDGE_CONTRACT_ADDRESS,
      functionName: 'bridgeTokens',
      args: [
        destChainId,
        recipient as `0x${string}`,
        BigInt(parseFloat(amount) * 10 ** 9) // 9 decimals for QNTO
      ]
    } as any);
  };

  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      toast.success('Cross-chain Transfer Confirmed!');
    }
  }, [isConfirmed]);

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

  let bridgeButtonText = 'INITIATE BRIDGE';
  if (!isConnected) {
    bridgeButtonText = 'Connect Wallet';
  } else if (isPending) {
    bridgeButtonText = 'Transferring...';
  } else if (isBridgeSuccess) {
    bridgeButtonText = 'Transfer Confirmed ✅';
  }

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-[0_0_50px_rgba(139,92,246,0.1)] relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(139,92,246,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '30s' }} />

        <div className="relative z-10">
          <h1 className="text-3xl font-extrabold text-white text-center mb-2 font-sans tracking-tight">
            Quantum Lattice Bridge
          </h1>
          <p className="text-sm text-slate-400 text-center font-sans mb-8">
            Bridge your $QNTO assets securely to external Ethereum Virtual Machine (EVM) layers.
          </p>

          <div className="space-y-4 mb-6">
            {/* Chain Selection Row */}
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase">From</span>
                <select
                  value={fromChain}
                  onChange={(e) => setFromChain(e.target.value)}
                  disabled={isPending || isBridgeSuccess}
                  className="bg-transparent text-sm font-bold font-mono text-white outline-none border-none p-0 cursor-pointer focus:ring-0"
                >
                  <option value="Qanto Testnet" className="bg-slate-900">Qanto Testnet</option>
                </select>
              </div>

              <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase">To</span>
                <select
                  value={toChain}
                  onChange={(e) => setToChain(e.target.value)}
                  disabled={isPending || isBridgeSuccess}
                  className="bg-transparent text-sm font-bold font-mono text-white outline-none border-none p-0 cursor-pointer focus:ring-0"
                >
                  <option value="Ethereum Sepolia" className="bg-slate-900">Ethereum Sepolia</option>
                  <option value="Arbitrum Sepolia" className="bg-slate-900">Arbitrum Sepolia</option>
                </select>
              </div>
            </div>

            {/* Input Amount */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div className="flex justify-between text-xs text-slate-500 font-mono">
                <span>Amount to Bridge</span>
                <span>Available: {isConnected ? '5,000 QNTO' : '0'}</span>
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
          </div>

          {/* Details */}
          <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mb-8 text-xs font-mono text-slate-400 space-y-2">
            <div className="flex justify-between">
              <span>Bridge Fee:</span>
              <span className="text-emerald-400">0.00 QNTO</span>
            </div>
            <div className="flex justify-between">
              <span>Estimated Duration:</span>
              <span>~ 15 Seconds</span>
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
