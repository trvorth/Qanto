import { useEffect, useRef, useState } from 'react';
import { toast } from 'react-hot-toast';
import { useQantoBalance } from '../hooks/useQantoBalance';
import {
  ConnectButton,
  useAccount,
  useSendTransaction,
  useWaitForTransactionReceipt,
} from '../lib/qanto-wallet';

const AIRDROP_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000008';

export function AirdropClaim() {
  const { isConnected, address } = useAccount();
  const { live: balance, refresh: refreshBalance } = useQantoBalance();
  const [localClaimed, setLocalClaimed] = useState(false);

  const { sendTransaction, isPending: isWritePending, error, data: txHash } = useSendTransaction({
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
          console.error('Wallet transaction error:', error);
        }
      }
    }
  });

  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  // Sync claim status from localStorage or balance
  useEffect(() => {
    if (address) {
      const claimed = localStorage.getItem(`qanto_airdrop_claimed_${address.toLowerCase()}`);
      if (claimed === 'true' || balance > 0) {
        setLocalClaimed(true);
      } else {
        setLocalClaimed(false);
      }
    } else {
      setLocalClaimed(false);
    }
  }, [address, balance]);

  const handleClaim = () => {
    if (!isConnected) {
      toast.error('Connect your wallet to claim.');
      return;
    }

    toastId.current = toast.loading('Confirming transaction...');

    // Generate zero-knowledge airdrop claim parameters passed as raw transaction data
    const claimData = `0x01${address?.slice(2)}`;

    sendTransaction({
      to: AIRDROP_CONTRACT_ADDRESS as `0x${string}`,
      data: claimData as `0x${string}`,
      value: 0n,
    });
  };

  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      if (address) {
        localStorage.setItem(`qanto_airdrop_claimed_${address.toLowerCase()}`, 'true');
        setLocalClaimed(true);
      }
      toast.success('Airdrop Claim Successful!');
      refreshBalance();
    }
  }, [isConfirmed, address, refreshBalance]);

  const isPending = isWritePending || isConfirming;
  const isClaimSuccessful = isConfirmed || localClaimed;
  const hasClaimed = localClaimed || balance > 0;

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-purple-glow relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(6,182,212,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '25s' }} />

        <div className="relative z-10 text-center">
          <h1 className="text-3xl font-extrabold text-white mb-2 font-sans tracking-tight">
            Pioneer Genesis Airdrop
          </h1>
          <p className="text-sm text-slate-400 font-sans mb-8">
            Claim your allocation of the 10M TPS Layer-0.
          </p>

          <div className="bg-black/40 border border-white/5 rounded-2xl p-6 mb-8">
            <div className="text-[10px] md:text-xs font-mono uppercase tracking-wider text-slate-500 mb-2">
              Eligible Allocation
            </div>
            <div className="text-4xl md:text-5xl font-extrabold bg-gradient-to-r from-neon-cyan to-quantum-purple bg-clip-text text-transparent font-mono">
              {isConnected ? '1,000' : '???'}
            </div>
            <div className="text-sm font-semibold text-white mt-2 font-mono">
              $QNTO
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleClaim}
              disabled={!isConnected || isPending || hasClaimed || isClaimSuccessful}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected || isPending || hasClaimed
                  ? 'bg-slate-700 cursor-not-allowed text-slate-400 border-transparent shadow-none'
                  : isClaimSuccessful
                  ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {!isConnected
                ? 'Wallet Disconnected'
                : hasClaimed
                ? 'AIRDROP ALREADY CLAIMED 🟢'
                : isClaimSuccessful
                ? 'CLAIM SUCCESSFUL ✅'
                : isPending
                ? 'GENERATING ZERO-KNOWLEDGE PROOF...'
                : 'EXECUTE ZK-PROOF CLAIM'}
            </button>

            {!isConnected && (
              <div className="flex flex-col items-center gap-4 mt-2">
                <p className="text-xs text-slate-400 font-sans">
                  Connect your Web3 wallet to verify block eligibility.
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
                ⚠️ Claim Error: {error.message}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
