import { useState, useEffect, useRef } from 'react';
import { useAccount, useWriteContract, useWaitForTransactionReceipt } from 'wagmi';
import { useConnectModal } from '@rainbow-me/rainbowkit';
import { toast } from 'react-hot-toast';

const STAKING_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000011';

const QantoStakingAbi = [
  {
    type: 'function',
    name: 'stake',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'amount', type: 'uint256' }
    ],
    outputs: []
  }
] as const;

export const Staking = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();
  const [stakeAmount, setStakeAmount] = useState('1000');
  
  const { writeContract, data: txHash, isPending: isWritePending } = useWriteContract({
    mutation: {
      onError: (error: any) => {
        if (toastId.current) {
          toast.dismiss(toastId.current);
          toastId.current = null;
        }
        if (error.message?.includes('User rejected') || error.code === 4001) {
          toast.error('Transaction cancelled by user.');
        } else if (error.message?.includes('insufficient funds')) {
          toast.error('Insufficient QNTO balance for execution.');
        } else {
          toast.error('Blockchain transaction failed.');
          console.error('Wagmi Core Error:', error);
        }
      }
    }
  });
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  const handleStake = () => {
    if (!isConnected) {
      if (openConnectModal) {
        openConnectModal();
      } else {
        toast.error('Connect your wallet to stake.');
      }
      return;
    }

    if (!stakeAmount || parseFloat(stakeAmount) <= 0 || isNaN(parseFloat(stakeAmount))) {
      toast.error('Please enter a valid amount to stake.');
      return;
    }

    // Initialize loading toast
    toastId.current = toast.loading('Confirming transaction...');

    writeContract({
      abi: QantoStakingAbi,
      address: STAKING_CONTRACT_ADDRESS,
      functionName: 'stake',
      args: [
        BigInt(parseFloat(stakeAmount) * 10 ** 9) // 9 decimals for QNTO
      ]
    } as any);
  };

  // Monitor transaction confirmation
  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      toast.success('Sentinel Node Initialized!');
    }
  }, [isConfirmed]);



  const isPending = isWritePending || isConfirming;
  const isStakedSuccessfully = isConfirmed;

  // Determine button text
  let buttonText = 'INITIALIZE SENTINEL NODE';
  if (!isConnected) {
    buttonText = 'Connect Sentinel';
  } else if (isPending) {
    buttonText = 'Initializing Node...';
  } else if (isStakedSuccessfully) {
    buttonText = 'Node Active ✅';
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
            Stake $QNTO to register your Sentinel node and earn 18.4% dynamic network rewards.
          </p>

          {/* Staking Stats Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-8 w-full">
            <div className="bg-black/40 border border-white/5 rounded-2xl p-4">
              <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">
                Estimated APY
              </div>
              <div className="text-2xl font-bold text-cyan-400 font-mono">
                18.42%
              </div>
            </div>
            <div className="bg-black/40 border border-white/5 rounded-2xl p-4">
              <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">
                Total Staked Mesh
              </div>
              <div className="text-2xl font-bold text-violet-400 font-mono">
                24,912,410
              </div>
            </div>
          </div>

          {/* Input Amount */}
          <div className="mb-8 text-left">
            <label className="block text-xs font-mono uppercase tracking-wider text-slate-400 mb-2">
              Stake Amount ($QNTO)
            </label>
            <div className="relative">
              <input
                type="number"
                value={stakeAmount}
                onChange={(e) => setStakeAmount(e.target.value)}
                disabled={isPending || isStakedSuccessfully}
                className="w-full bg-black/40 border border-white/10 focus:border-cyan-500/50 rounded-xl py-4 px-5 text-white font-mono text-lg outline-none transition-all"
                placeholder="1000"
              />
              <span className="absolute right-5 top-1/2 -translate-y-1/2 font-mono text-sm text-slate-500">
                QNTO
              </span>
            </div>
            <div className="mt-2 text-[10px] text-slate-500 font-mono flex justify-between">
              <span>Minimum Stake: 1,000 $QNTO</span>
              <span>Available: {isConnected ? '5,000 QNTO' : '0.00 QNTO'}</span>
            </div>
            
            {/* Details */}
            <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mt-4 mb-6 text-xs font-mono text-slate-400 space-y-2">
              <div className="flex justify-between text-sm mt-4">
                <span className="text-slate-400">Network Fee</span>
                <span className="text-emerald-400 font-bold drop-shadow-[0_0_8px_rgba(52,211,153,0.8)] animate-pulse">Sponsored by SAGA 🧠</span>
              </div>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleStake}
              disabled={isPending || (isConnected && isStakedSuccessfully)}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                isPending
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : isStakedSuccessfully
                  ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
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
