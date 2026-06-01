import { useState } from 'react';
import { useAccount, useWriteContract } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useQuery } from '@tanstack/react-query';
import { request, gql } from 'graphql-request';
import { AddressDisplay } from './AddressDisplay';
import { CreateProposal } from './CreateProposal';

const GOVERNANCE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000010';
const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';

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

interface ProposalGQL {
  id: string;
  proposer: string;
  proposalType: string;
  votesFor: number;
  votesAgainst: number;
  status: string;
  creationEpoch: number;
  justification: string | null;
}

const GET_PROPOSALS = gql`
  query GetProposals {
    proposals {
      id
      proposer
      proposalType
      votesFor
      votesAgainst
      status
      creationEpoch
      justification
    }
  }
`;

export function Governance() {
  const { isConnected, address } = useAccount();
  const { writeContract, isPending, isSuccess, error, data: txHash } = useWriteContract();
  
  // Simulated vote counts
  const [votesFor, setVotesFor] = useState(1420069);
  const [votesAgainst, setVotesAgainst] = useState(231500);
  const [votedDirection, setVotedDirection] = useState<'FOR' | 'AGAINST' | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);

  const { data: proposalsData, refetch: refetchProposals } = useQuery<{ proposals: ProposalGQL[] }>({
    queryKey: ['proposalsData'],
    queryFn: async () => {
      try {
        const result = await request<{ proposals: ProposalGQL[] }>(GRAPHQL_ENDPOINT, GET_PROPOSALS);
        return result;
      } catch (err) {
        return { proposals: [] };
      }
    },
    refetchInterval: 3000,
  });

  const handleVote = (proposalId: string | number, support: number) => {
    let numericId = 1n;
    if (typeof proposalId === 'string') {
      let hash = 0n;
      for (let i = 0; i < proposalId.length; i++) {
        hash = (hash * 31n + BigInt(proposalId.charCodeAt(i))) % (2n ** 256n);
      }
      numericId = hash;
    } else {
      numericId = BigInt(proposalId);
    }

    writeContract({
      abi: QantoGovAbi,
      address: GOVERNANCE_CONTRACT_ADDRESS,
      functionName: 'castVote',
      args: [
        numericId,
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
          <p className="text-xs text-slate-400 text-center font-sans mb-6">
            Cast your cryptographic weight to direct Qanto Layer-0 SAGA AI parameters.
          </p>

          {/* Header Action to Create Proposal */}
          <div className="flex justify-between items-center mb-6">
            <button
              onClick={() => setShowCreateForm(!showCreateForm)}
              className="w-full py-2.5 px-4 rounded-xl bg-violet-600/20 hover:bg-violet-600 hover:text-white text-violet-300 border border-violet-500/30 transition-all duration-300 font-bold font-sans text-xs md:text-sm"
            >
              {showCreateForm ? '← VIEW ACTIVE PROPOSALS' : '➕ CREATE NEW PROPOSAL'}
            </button>
          </div>

          {showCreateForm ? (
            <CreateProposal 
              onSuccess={() => {
                setShowCreateForm(false);
                refetchProposals();
              }}
              onCancel={() => setShowCreateForm(false)}
            />
          ) : (
            <>
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
                    onClick={() => handleVote(1, 1)}
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
                    onClick={() => handleVote(1, 0)}
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

                {isConnected && address && (
                  <div className="flex flex-col items-center gap-2 mt-4 text-[10px] font-mono text-slate-500 text-center">
                    <div className="flex items-center gap-2">
                      <span>Signer Address:</span>
                      <AddressDisplay address={address} />
                    </div>
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

              {/* Dynamic Proposals from GraphQL Node */}
              {proposalsData?.proposals && proposalsData.proposals.length > 0 && (
                <div className="space-y-6 mt-8">
                  <h3 className="text-xs font-mono uppercase tracking-wider text-slate-400 border-b border-white/10 pb-2">
                    Decentralized IPFS Proposals
                  </h3>
                  {proposalsData.proposals.map((proposal) => {
                    const isIpfs = proposal.id.startsWith('ipfs-');
                    const titleText = isIpfs ? `IPFS Proposal: ${proposal.id.slice(5, 17)}...` : proposal.id;
                    const descText = proposal.justification || `Proposal type: ${proposal.proposalType}`;
                    
                    const propTotalVotes = proposal.votesFor + proposal.votesAgainst;
                    const propPctFor = propTotalVotes > 0 ? ((proposal.votesFor / propTotalVotes) * 100).toFixed(1) : '0.0';
                    const propPctAgainst = propTotalVotes > 0 ? ((proposal.votesAgainst / propTotalVotes) * 100).toFixed(1) : '0.0';

                    return (
                      <div key={proposal.id} className="bg-black/50 border border-white/10 rounded-2xl p-6 hover:border-violet-500/20 transition-all duration-300">
                        <div className="flex justify-between items-start mb-3">
                          <span className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase tracking-wider ${
                            proposal.status === 'Voting'
                              ? 'bg-cyan-500/10 text-cyan-400 border-cyan-500/20'
                              : 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                          }`}>
                            {proposal.status}
                          </span>
                          <div className="flex items-center gap-1">
                            <span className="text-[9px] font-mono text-slate-500">Proposer:</span>
                            <AddressDisplay address={proposal.proposer} />
                          </div>
                        </div>

                        <h3 className="text-sm font-bold text-white mb-2 font-sans break-all">
                          {titleText}
                        </h3>
                        
                        <p className="text-xs text-slate-400 font-mono mb-4 leading-relaxed bg-white/5 p-3 rounded-xl border border-white/5 break-all">
                          {descText}
                        </p>

                        {/* Progress & Voting Info */}
                        <div className="space-y-4 font-mono text-[10px] mb-4">
                          <div className="flex justify-between text-slate-400">
                            <span>FOR: {proposal.votesFor.toLocaleString()} weight ({propPctFor}%)</span>
                            <span>AGAINST: {proposal.votesAgainst.toLocaleString()} weight ({propPctAgainst}%)</span>
                          </div>
                          
                          <div className="w-full bg-slate-800 h-1.5 rounded-full overflow-hidden flex">
                            <div className="bg-cyan-400 h-full transition-all duration-500" style={{ width: `${propPctFor}%` }} />
                            <div className="bg-rose-500 h-full transition-all duration-500" style={{ width: `${propPctAgainst}%` }} />
                          </div>
                        </div>

                        {proposal.status === 'Voting' && (
                          <div className="grid grid-cols-2 gap-3">
                            <button
                              onClick={() => handleVote(proposal.id, 1)}
                              disabled={!isConnected || isPending}
                              className="py-2 px-4 rounded-lg font-bold font-sans text-xs border bg-cyan-500/5 hover:bg-cyan-500 hover:text-white text-cyan-400 border-cyan-500/20 transition-all duration-300"
                            >
                              VOTE FOR
                            </button>
                            <button
                              onClick={() => handleVote(proposal.id, 0)}
                              disabled={!isConnected || isPending}
                              className="py-2 px-4 rounded-lg font-bold font-sans text-xs border bg-rose-500/5 hover:bg-rose-500 hover:text-white text-rose-400 border-rose-500/20 transition-all duration-300"
                            >
                              VOTE AGAINST
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
