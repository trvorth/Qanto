import { useState } from 'react';
import { useAccount, useWriteContract } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';

const GOVERNANCE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000010';

const QantoGovAbi = [
  {
    type: 'function',
    name: 'castVote',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'proposalId', type: 'uint256' },
      { name: 'support', type: 'uint8' } // 0 = Against, 1 = For, 2 = Abstain
    ],
    outputs: []
  }
] as const;

export function Governance() {
  const { isConnected, address } = useAccount();
  const { writeContract, isPending, isSuccess, error, data: txHash } = useWriteContract();
  
  // Simulated vote counts
  const [votesFor, setVotesFor] = useState(1420069);
  const [votesAgainst, setVotesAgainst] = useState(231500);
  const [votedDirection, setVotedDirection] = useState<'FOR' | 'AGAINST' | null>(null);

  const handleVote = (support: number) => {
    writeContract({
      abi: QantoGovAbi,
      address: GOVERNANCE_CONTRACT_ADDRESS,
      functionName: 'castVote',
      args: [
        BigInt(1), // Proposal ID 1
        support // 0 for AGAINST, 1 for FOR
      ],
    });
    
    // Simulate updating UI on local success
    if (support === 1) {
      setVotedDirection('FOR');
      setVotesFor((prev) => prev + 100000); // Add voter weight
    } else {
      setVotedDirection('AGAINST');
      setVotesAgainst((prev) => prev + 100000); // Add voter weight
    }
  };

  const totalVotes = votesFor + votesAgainst;
  const pctFor = ((votesFor / totalVotes) * 100).toFixed(1);
  const pctAgainst = ((votesAgainst / totalVotes) * 100).toFixed(1);

  return (
    <div id="governance" className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-purple-glow relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(139,92,246,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '35s' }} />

        <div className="relative z-10">
          <h2 className="text-xl md:text-2xl font-bold text-white text-center mb-1 font-sans tracking-tight">
            SOVEREIGN DAO
          </h2>
          <p className="text-xs text-slate-400 text-center font-sans mb-8">
            Cast your cryptographic weight to direct Qanto Layer-0 SAGA AI parameters.
          </p>

          {/* Proposal Card */}
          <div className="bg-black/50 border border-white/10 rounded-2xl p-6 mb-8 hover:border-violet-500/20 transition-all duration-300">
            <div className="flex justify-between items-center mb-3">
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 uppercase tracking-wider">
                Active Proposal
              </span>
              <span className="text-[10px] font-mono text-slate-500">
                Ends in 3 days
              </span>
            </div>

            <h3 className="text-lg font-bold text-white mb-2 font-sans">
              QIP-01: Increase SAGA AI Base Throttle to 64 BPS
            </h3>
            
            <p className="text-xs text-slate-400 font-sans mb-6 leading-relaxed">
              This proposal raises the SAGA AI decentralized network throttle threshold from 32 BPS to 64 BPS to allow for autonomous transaction prioritization under dense mempool conditions.
            </p>

            {/* Voting Metrics / Progress Bar */}
            <div className="space-y-4 font-mono text-xs mb-2">
              <div className="flex justify-between text-slate-300">
                <span>FOR: {votesFor.toLocaleString()} $QNTO ({pctFor}%)</span>
                <span>AGAINST: {votesAgainst.toLocaleString()} $QNTO ({pctAgainst}%)</span>
              </div>
              
              <div className="w-full bg-slate-800 h-2.5 rounded-full overflow-hidden flex">
                <div className="bg-cyan-400 h-full transition-all duration-500" style={{ width: `${pctFor}%` }} />
                <div className="bg-rose-500 h-full transition-all duration-500" style={{ width: `${pctAgainst}%` }} />
              </div>
            </div>
          </div>

          {/* Voting Controls */}
          <div className="flex flex-col gap-4">
            <div className="grid grid-cols-2 gap-4">
              <button
                onClick={() => handleVote(1)}
                disabled={!isConnected || isPending || isSuccess}
                className={`py-3.5 px-6 rounded-xl font-bold font-sans text-xs md:text-sm border transition-all duration-300 ${
                  !isConnected || isPending || isSuccess
                    ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed'
                    : 'bg-cyan-500/10 hover:bg-cyan-500 hover:text-white text-cyan-400 border-cyan-500/30 hover:shadow-[0_0_15px_rgba(6,182,212,0.3)] hover:scale-[1.02]'
                }`}
              >
                VOTE FOR
              </button>
              
              <button
                onClick={() => handleVote(0)}
                disabled={!isConnected || isPending || isSuccess}
                className={`py-3.5 px-6 rounded-xl font-bold font-sans text-xs md:text-sm border transition-all duration-300 ${
                  !isConnected || isPending || isSuccess
                    ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed'
                    : 'bg-rose-500/10 hover:bg-rose-500 hover:text-white text-rose-400 border-rose-500/30 hover:shadow-[0_0_15px_rgba(244,63,94,0.3)] hover:scale-[1.02]'
                }`}
              >
                VOTE AGAINST
              </button>
            </div>

            {!isConnected && (
              <div className="flex flex-col items-center gap-4 mt-2">
                <p className="text-xs text-slate-400 font-sans">
                  Connect your wallet to sign the voting parameters.
                </p>
                <div className="flex justify-center">
                  <ConnectButton label="Connect Wallet" />
                </div>
              </div>
            )}

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all text-center">
                <div>Signer Address: {address}</div>
                {isSuccess && (
                  <div className="mt-4 p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs">
                    <div className="font-bold mb-1">🎉 VOTE CAST SUCCESSFUL!</div>
                    <div>Simulated transaction confirmed.</div>
                    {txHash && <div className="mt-1 font-mono text-[10px] break-all">Hash: {txHash}</div>}
                    <div className="mt-1 text-slate-400">
                      Voted {votedDirection} with your address weight.
                    </div>
                  </div>
                )}
              </div>
            )}

            {error && (
              <div className="text-xs font-mono text-rose-500 mt-2 bg-rose-500/10 border border-rose-500/20 py-2 px-3 rounded-lg text-left break-words">
                ⚠️ Voting Error: {error.message}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
