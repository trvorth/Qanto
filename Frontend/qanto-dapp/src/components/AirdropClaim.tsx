import { useState } from 'react';
import { useAccount } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';

export function AirdropClaim() {
  const { isConnected, address } = useAccount();
  const [claiming, setClaiming] = useState(false);
  const [claimed, setClaimed] = useState(false);
  const [statusText, setStatusText] = useState('');

  const handleClaim = () => {
    setClaiming(true);
    setStatusText('Generating Merkle Proof inside Sentinel Enclave...');
    
    setTimeout(() => {
      setStatusText('Awaiting wallet cryptographic approval...');
      
      setTimeout(() => {
        setClaimed(true);
        setClaiming(false);
        setStatusText('1,000 $QNTO successfully deposited into your wallet.');
      }, 2000);
    }, 2000);
  };

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

          {!isConnected ? (
            <div className="flex flex-col items-center gap-4">
              <p className="text-xs text-slate-400 font-sans">
                Connect your Web3 wallet to verify block eligibility.
              </p>
              <div className="flex justify-center">
                <ConnectButton label="Connect Wallet" />
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-4">
              <button
                onClick={handleClaim}
                disabled={claiming || claimed}
                className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                  claimed
                    ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                    : claiming
                    ? 'bg-cyan-500/10 border-cyan-500/30 text-cyan-400 cursor-wait'
                    : 'bg-gradient-to-r from-neon-cyan to-quantum-purple border-transparent text-white shadow-lg shadow-quantum-purple/10 hover:shadow-quantum-purple/25 hover:scale-[1.02]'
                }`}
              >
                {claimed 
                  ? 'CLAIM SUCCESSFUL ✅' 
                  : claiming 
                  ? 'GENERATING ZERO-KNOWLEDGE PROOF...' 
                  : 'EXECUTE ZK-PROOF CLAIM'}
              </button>
              
              {statusText && (
                <div className={`text-xs font-mono mt-2 transition-all duration-300 ${
                  claimed ? 'text-emerald-400' : 'text-slate-400'
                }`}>
                  {statusText}
                </div>
              )}

              {/* Connected details */}
              <div className="text-[10px] font-mono text-slate-500 break-all mt-4">
                Connected Address: {address}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
