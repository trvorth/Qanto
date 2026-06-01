import { useState } from 'react';
import { useAccount, useWriteContract, useReadContract } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import { formatEther } from 'viem';

const TGE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000009';
const RATE = 10000; // 1 ETH = 10,000 QNTO

/**
 * Canonical supply constants — mathematically sealed.
 * No purchase interaction can exceed the total network capacity.
 */
const TOTAL_SUPPLY_QNTO = 21_000_000_000;
const COMMUNITY_ALLOCATION_QNTO = TOTAL_SUPPLY_QNTO * 0.80; // 16,800,000,000
const MAX_PURCHASABLE_QNTO = COMMUNITY_ALLOCATION_QNTO; // Hard ceiling for TGE

const QantoTgeAbi = [
  {
    type: 'function',
    name: 'buyTokens',
    stateMutability: 'payable',
    inputs: [],
    outputs: []
  },
  {
    type: 'function',
    name: 'totalRaised',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }]
  }
] as const;

export function TokenSale() {
  const { isConnected, address } = useAccount();
  const [ethAmount, setEthAmount] = useState('0.1');
  const { writeContract, isPending, isSuccess, error, data: txHash } = useWriteContract();

  const { data: totalRaised } = useReadContract({
    abi: QantoTgeAbi,
    address: TGE_CONTRACT_ADDRESS,
    functionName: 'totalRaised',
    query: {
      refetchInterval: 5000,
    }
  });

  const handleBuy = () => {
    if (!ethAmount || parseFloat(ethAmount) <= 0 || isNaN(parseFloat(ethAmount))) return;
    
    writeContract({
      abi: QantoTgeAbi,
      address: TGE_CONTRACT_ADDRESS,
      functionName: 'buyTokens',
      value: BigInt(Math.floor(parseFloat(ethAmount) * 10 ** 18)),
    } as any);
  };

  const rawQnto = ethAmount && !isNaN(parseFloat(ethAmount))
    ? parseFloat(ethAmount) * RATE
    : 0;
  // Overflow guard: clamp to the maximum purchasable supply ceiling
  const clampedQnto = Math.min(rawQnto, MAX_PURCHASABLE_QNTO);
  const calculatedQnto = clampedQnto.toLocaleString();
  const isOverCap = rawQnto > MAX_PURCHASABLE_QNTO;

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-[0_0_50px_rgba(139,92,246,0.1)] relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(139,92,246,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '30s' }} />

        <div className="relative z-10">
          <h1 className="text-3xl font-extrabold text-white text-center mb-2 font-sans tracking-tight">
            Qanto Fair Launch Participation
          </h1>
          <p className="text-sm text-slate-400 text-center font-sans mb-4">
            Fair Launch: 21,000,000,000 QNTO — 80% Community, 15% Ecosystem, 5% Liquidity.
          </p>
          <p className="text-sm text-slate-400 text-center font-sans mb-8">
            1 ETH = 10,000 $QNTO. Zero VC. Zero team tokens. Zero pre-mine.
          </p>

          {/* Progress bar or info */}
          <div className="bg-black/40 border border-white/5 rounded-2xl p-6 mb-8 text-center">
            <div className="text-[10px] md:text-xs font-mono uppercase tracking-wider text-slate-500 mb-1">
              Total Raised
            </div>
            <div className="text-3xl font-extrabold text-white font-mono mb-2">
              {totalRaised ? `${parseFloat(formatEther(totalRaised)).toFixed(4)} ETH` : '0.0000 ETH'}
            </div>
            <div className="w-full bg-slate-800 h-2 rounded-full overflow-hidden">
              <div className="bg-gradient-to-r from-cyan-400 to-violet-500 h-full w-[12%]" />
            </div>
            <div className="flex justify-between text-[9px] text-slate-500 font-mono mt-1.5">
              <span>Community: 80% (16.8B QNTO)</span>
              <span>Total Supply: 21B QNTO</span>
            </div>
          </div>

          {/* Input field */}
          <div className="mb-6">
            <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
              Purchase Amount (ETH)
            </label>
            <div className="relative">
              <input
                type="number"
                step="0.01"
                min="0.001"
                value={ethAmount}
                onChange={(e) => setEthAmount(e.target.value)}
                disabled={isPending}
                className="w-full bg-black/40 border border-white/10 focus:border-cyan-500/50 rounded-xl py-4 px-5 text-white font-mono text-lg outline-none transition-all"
                placeholder="0.1"
              />
              <span className="absolute right-5 top-1/2 -translate-y-1/2 font-mono text-sm text-slate-500">
                ETH
              </span>
            </div>
          </div>

          {/* Output allocation */}
          <div className="bg-white/5 border border-white/5 rounded-xl p-4 mb-4 flex justify-between items-center">
            <span className="text-xs font-sans text-slate-400">Tokens to Receive:</span>
            <span className="text-xl font-bold text-cyan-400 font-mono">{calculatedQnto} $QNTO</span>
          </div>

          {isOverCap && (
            <div className="text-xs font-mono text-amber-500 mb-4 bg-amber-500/10 border border-amber-500/20 py-2 px-3 rounded-lg text-center">
              ⚠️ Purchase clamped to maximum supply ceiling: {MAX_PURCHASABLE_QNTO.toLocaleString()} QNTO (Community 80%)
            </div>
          )}

          <div className="text-[9px] text-slate-500 font-mono text-center mb-6">
            Hard Cap: {TOTAL_SUPPLY_QNTO.toLocaleString()} QNTO · Community: 80% · Eco/Dev: 15% · Liquidity: 5%
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleBuy}
              disabled={!isConnected || isPending || !ethAmount || parseFloat(ethAmount) <= 0 || isOverCap}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected || isPending || !ethAmount || parseFloat(ethAmount) <= 0 || isOverCap
                  ? 'bg-slate-700 cursor-not-allowed text-slate-400 border-transparent shadow-none'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {!isConnected
                ? 'Wallet Disconnected'
                : isPending
                ? 'EXECUTING TGE TRANSACTION...'
                : isSuccess
                ? 'PURCHASE SUCCESSFUL ✅'
                : 'BUY $QNTO'}
            </button>

            {!isConnected && (
              <div className="flex flex-col items-center gap-4 mt-2">
                <p className="text-xs text-slate-400 font-sans">
                  Connect your Web3 wallet to participate.
                </p>
                <div className="flex justify-center">
                  <ConnectButton label="Connect Wallet" />
                </div>
              </div>
            )}

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all text-center">
                <div>Connected Address: {address}</div>
                {txHash && <div className="text-cyan-400">Transaction Hash: {txHash}</div>}
              </div>
            )}

            {error && (
              <div className="text-xs font-mono text-rose-500 mt-2 bg-rose-500/10 border border-rose-500/20 py-2 px-3 rounded-lg text-left break-words">
                ⚠️ Buy Error: {error.message}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
