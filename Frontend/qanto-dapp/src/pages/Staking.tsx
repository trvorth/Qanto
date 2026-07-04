import { useState, useEffect, useRef } from 'react';
import { toast } from 'react-hot-toast';
import { useQantoBalance } from '../hooks/useQantoBalance';
import {
  useAccount,
  useConnectModal,
  useSendTransaction,
  useWaitForTransactionReceipt,
} from '../lib/qanto-wallet';

const STAKING_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000011';
const REST_BASE = 'https://trvorth-qanto-testnet.hf.space';

interface StakingStats {
  apy: string;
  total_staked: string;
  validator_count: number;
  min_stake: string;
  epoch: number;
}

export const Staking = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();
  const [activeTab, setActiveTab] = useState<'stake' | 'unstake' | 'delegate'>('stake');
  const [stakeAmount, setStakeAmount] = useState('1000');
  const [unstakeAmount, setUnstakeAmount] = useState('500');
  const [delegateAmount, setDelegateAmount] = useState('500');
  const [validatorAddress, setValidatorAddress] = useState('');
  const { confirmed: balance, refresh: refreshBalance } = useQantoBalance();
  const [stakingStats, setStakingStats] = useState<StakingStats | null>(null);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch(`${REST_BASE}/staking`, { signal: AbortSignal.timeout(5000) });
        if (res.ok) {
          const data: StakingStats = await res.json();
          setStakingStats(data);
        }
      } catch (err) {
        console.error('Failed to fetch staking stats', err);
      }
    };
    fetchStats();
    const interval = setInterval(fetchStats, 10000);
    return () => clearInterval(interval);
  }, []);

  const dynamicApy = stakingStats?.apy ?? '18.40';
  const totalStaked = stakingStats ? BigInt(stakingStats.total_staked) : 0n;
  const totalStakedDisplay = totalStaked > 0n
    ? Number(totalStaked / 1_000_000_000n).toLocaleString()
    : (stakingStats?.validator_count ?? 0).toLocaleString();
  const minStake = stakingStats?.min_stake ?? '1000';

  const { sendTransaction, data: txHash, isPending: isWritePending, error: sendError } = useSendTransaction();
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  useEffect(() => {
    if (sendError) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      if (sendError.message?.includes('User rejected') || (sendError as any).code === 4001) {
        toast.error('Transaction cancelled by user.');
      } else if (sendError.message?.includes('insufficient funds')) {
        toast.error('Insufficient QNTO balance for execution.');
      } else {
        toast.error('Blockchain transaction failed.');
        console.error('Wallet transaction error:', sendError);
      }
    }
  }, [sendError]);

  const handleAction = () => {
    if (!isConnected) {
      if (openConnectModal) {
        openConnectModal();
      } else {
        toast.error('Connect your wallet to proceed.');
      }
      return;
    }

    if (activeTab === 'stake') {
      if (!stakeAmount || parseFloat(stakeAmount) <= 0 || isNaN(parseFloat(stakeAmount))) {
        toast.error('Please enter a valid amount to stake.');
        return;
      }
      toastId.current = toast.loading('Confirming stake transaction...');
      sendTransaction({
        to: STAKING_CONTRACT_ADDRESS as `0x${string}`,
        value: BigInt(Math.floor(parseFloat(stakeAmount) * 10 ** 18)),
      });
    } else if (activeTab === 'unstake') {
      if (!unstakeAmount || parseFloat(unstakeAmount) <= 0 || isNaN(parseFloat(unstakeAmount))) {
        toast.error('Please enter a valid amount to unstake.');
        return;
      }
      toastId.current = toast.loading('Confirming unstake transaction...');
      const amountStr = unstakeAmount.trim();
      const encoder = new TextEncoder();
      const bytes = encoder.encode(amountStr);
      let hexData = '0x02';
      for (let i = 0; i < bytes.length; i++) {
        hexData += bytes[i].toString(16).padStart(2, '0');
      }
      sendTransaction({
        to: STAKING_CONTRACT_ADDRESS as `0x${string}`,
        value: 0n,
        data: hexData as `0x${string}`,
      });
    } else if (activeTab === 'delegate') {
      if (!delegateAmount || parseFloat(delegateAmount) <= 0 || isNaN(parseFloat(delegateAmount))) {
        toast.error('Please enter a valid amount to delegate.');
        return;
      }
      if (!validatorAddress || !validatorAddress.startsWith('0x') || validatorAddress.length !== 42) {
        toast.error('Please enter a valid 20-byte Ethereum-style validator address (0x...).');
        return;
      }
      toastId.current = toast.loading('Confirming delegation transaction...');
      const cleanAddr = validatorAddress.slice(2);
      const bytes = new Uint8Array(20);
      for (let i = 0; i < 20; i++) {
        bytes[i] = parseInt(cleanAddr.substring(i * 2, (i + 1) * 2), 16);
      }
      let hexData = '0x03';
      for (let i = 0; i < 20; i++) {
        hexData += bytes[i].toString(16).padStart(2, '0');
      }
      sendTransaction({
        to: STAKING_CONTRACT_ADDRESS as `0x${string}`,
        value: BigInt(Math.floor(parseFloat(delegateAmount) * 10 ** 18)),
        data: hexData as `0x${string}`,
      });
    }
  };

  // Monitor transaction confirmation
  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      if (activeTab === 'stake') {
        toast.success('Sentinel Node Stake Successful!');
      } else if (activeTab === 'unstake') {
        toast.success('Sentinel Node Unstake Successful!');
      } else if (activeTab === 'delegate') {
        toast.success('Staking Power Delegated Successfully!');
      }
      refreshBalance();
    }
  }, [isConfirmed, refreshBalance]);

  const isPending = isConfirming || isWritePending;

  // Determine button details
  let buttonText = 'INITIALIZE SENTINEL NODE';
  let buttonColorClass = 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]';
  
  if (!isConnected) {
    buttonText = 'Connect Wallet';
  } else if (isPending) {
    buttonText = 'Processing Transaction...';
  } else {
    if (activeTab === 'stake') {
      buttonText = 'INITIALIZE SENTINEL NODE';
      buttonColorClass = 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]';
    } else if (activeTab === 'unstake') {
      buttonText = 'RELEASE SENTINEL STAKE';
      buttonColorClass = 'bg-violet-600 hover:bg-violet-500 text-white shadow-[0_0_20px_rgba(139,92,246,0.4)] border-transparent hover:scale-[1.02]';
    } else if (activeTab === 'delegate') {
      buttonText = 'DELEGATE VOTING POWER';
      buttonColorClass = 'bg-emerald-600 hover:bg-emerald-500 text-white shadow-[0_0_20px_rgba(16,185,129,0.4)] border-transparent hover:scale-[1.02]';
    }
  }

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-[0_0_50px_rgba(6,182,212,0.1)] relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(6,182,212,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '25s' }} />

        <div className="relative z-10 text-center">
          <h1 className="text-3xl font-extrabold text-white mb-2 font-sans tracking-tight">
            Sentinel Node Staking
          </h1>
          <p className="text-sm text-slate-400 font-sans mb-8">
            Manage Sentinel registration, stake withdrawals, and voting power delegation.
          </p>

          {/* Staking Stats Grid */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8 w-full">
            <div className="bg-black/40 border border-white/5 rounded-2xl p-4">
              <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">
                Estimated APY
              </div>
              <div className="text-2xl font-bold text-cyan-400 font-mono">
                {dynamicApy}%
              </div>
            </div>
            <div className="bg-black/40 border border-white/5 rounded-2xl p-4">
              <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">
                Total Staked
              </div>
              <div className="text-2xl font-bold text-violet-400 font-mono">
                {totalStakedDisplay} QNTO
              </div>
            </div>
            <div className="bg-black/40 border border-white/5 rounded-2xl p-4">
              <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">
                Active Sentinels
              </div>
              <div className="text-2xl font-bold text-emerald-400 font-mono">
                {stakingStats?.validator_count?.toLocaleString() ?? '—'}
              </div>
            </div>
          </div>

          {/* Tab Selector */}
          <div className="flex border-b border-white/10 mb-6 font-mono text-xs">
            <button
              onClick={() => setActiveTab('stake')}
              disabled={isPending}
              className={`flex-1 pb-3 text-center transition-all ${
                activeTab === 'stake' ? 'text-cyan-400 border-b-2 border-cyan-400 font-bold' : 'text-slate-400 hover:text-white'
              }`}
            >
              STAKE
            </button>
            <button
              onClick={() => setActiveTab('unstake')}
              disabled={isPending}
              className={`flex-1 pb-3 text-center transition-all ${
                activeTab === 'unstake' ? 'text-violet-400 border-b-2 border-violet-400 font-bold' : 'text-slate-400 hover:text-white'
              }`}
            >
              UNSTAKE
            </button>
            <button
              onClick={() => setActiveTab('delegate')}
              disabled={isPending}
              className={`flex-1 pb-3 text-center transition-all ${
                activeTab === 'delegate' ? 'text-emerald-400 border-b-2 border-emerald-400 font-bold' : 'text-slate-400 hover:text-white'
              }`}
            >
              DELEGATE
            </button>
          </div>

          {/* Inputs Section */}
          <div className="mb-8 text-left">
            {activeTab === 'stake' && (
              <div>
                <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
                  Stake Amount ($QNTO)
                </label>
                <div className="relative">
                  <input
                    type="number"
                    value={stakeAmount}
                    onChange={(e) => setStakeAmount(e.target.value)}
                    disabled={isPending}
                    className="w-full bg-black/40 border border-white/10 focus:border-cyan-500/50 rounded-xl py-4 px-5 text-white font-mono text-lg outline-none transition-all"
                    placeholder="1000"
                  />
                  <span className="absolute right-5 top-1/2 -translate-y-1/2 font-mono text-sm text-slate-500">
                    QNTO
                  </span>
                </div>
                <div className="mt-2 text-[10px] text-slate-500 font-mono flex justify-between">
                  <span>Minimum Stake: {Number(minStake).toLocaleString()} $QNTO</span>
                  <span>Available: {isConnected ? `${balance.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })} QNTO` : '0.00 QNTO'}</span>
                </div>
              </div>
            )}

            {activeTab === 'unstake' && (
              <div>
                <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
                  Unstake Amount ($QNTO)
                </label>
                <div className="relative">
                  <input
                    type="number"
                    value={unstakeAmount}
                    onChange={(e) => setUnstakeAmount(e.target.value)}
                    disabled={isPending}
                    className="w-full bg-black/40 border border-white/10 focus:border-violet-500/50 rounded-xl py-4 px-5 text-white font-mono text-lg outline-none transition-all"
                    placeholder="500"
                  />
                  <span className="absolute right-5 top-1/2 -translate-y-1/2 font-mono text-sm text-slate-500">
                    QNTO
                  </span>
                </div>
                <div className="mt-2 text-[10px] text-slate-500 font-mono flex justify-between">
                  <span>Unstake Cooldown: 0 epochs</span>
                  <span>Available: {isConnected ? `${balance.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })} QNTO` : '0.00 QNTO'}</span>
                </div>
              </div>
            )}

            {activeTab === 'delegate' && (
              <div className="space-y-4">
                <div>
                  <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
                    Delegate Amount ($QNTO)
                  </label>
                  <div className="relative">
                    <input
                      type="number"
                      value={delegateAmount}
                      onChange={(e) => setDelegateAmount(e.target.value)}
                      disabled={isPending}
                      className="w-full bg-black/40 border border-white/10 focus:border-emerald-500/50 rounded-xl py-4 px-5 text-white font-mono text-lg outline-none transition-all"
                      placeholder="500"
                    />
                    <span className="absolute right-5 top-1/2 -translate-y-1/2 font-mono text-sm text-slate-500">
                      QNTO
                    </span>
                  </div>
                </div>
                <div>
                  <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
                    Validator Address (Hex)
                  </label>
                  <input
                    type="text"
                    value={validatorAddress}
                    onChange={(e) => setValidatorAddress(e.target.value)}
                    disabled={isPending}
                    className="w-full bg-black/40 border border-white/10 focus:border-emerald-500/50 rounded-xl py-3 px-5 text-white font-mono text-sm outline-none transition-all"
                    placeholder="0x9F00..."
                  />
                </div>
                <div className="mt-2 text-[10px] text-slate-500 font-mono flex justify-between">
                  <span>Assign voting/reward power to another Sentinel</span>
                  <span>Available: {isConnected ? `${balance.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })} QNTO` : '0.00 QNTO'}</span>
                </div>
              </div>
            )}
            
            {/* Details */}
            <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mt-4 mb-6 text-xs font-mono text-slate-400 space-y-2">
              <div className="flex justify-between text-sm">
                <span className="text-slate-400">Network Fee</span>
                <span className="text-emerald-400 font-bold drop-shadow-[0_0_8px_rgba(52,211,153,0.8)] animate-pulse">Sponsored by SAGA 🧠</span>
              </div>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleAction}
              disabled={isPending}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                isPending
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : buttonColorClass
              }`}
            >
              {buttonText}
            </button>

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all">
                <div>Staking Sentinel Address: {address}</div>
                {txHash && <div className="text-cyan-400">Transaction Hash: {txHash}</div>}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
export default Staking;
