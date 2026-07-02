import { useState } from 'react';
import { useAccount, useSendTransaction } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useQuery } from '@tanstack/react-query';
import { request, gql } from 'graphql-request';
import { AddressDisplay } from './AddressDisplay';
import { CreateProposal } from './CreateProposal';

const GOVERNANCE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000010';
const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';

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
  const { sendTransaction, isPending, isSuccess, error, data: txHash } = useSendTransaction();
  
  const [votedProposalId, setVotedProposalId] = useState<string | null>(null);
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

  const handleVote = (proposalId: string, support: number) => {
    const dir = support === 1 ? 'FOR' : 'AGAINST';
    setVotedProposalId(proposalId);
    setVotedDirection(dir);

    const hexProposalId = Array.from(new TextEncoder().encode(proposalId))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
    const data = `0x${support.toString().padStart(2, '0')}${hexProposalId}`;

    sendTransaction({
      to: GOVERNANCE_CONTRACT_ADDRESS as `0x${string}`,
      data: data as `0x${string}`,
      value: 0n,
    });
  };

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
              {/* Dynamic Proposals from GraphQL Node */}
              {proposalsData?.proposals && proposalsData.proposals.length > 0 ? (
                <div className="space-y-6">
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
              ) : (
                <div className="text-center py-12 px-4 bg-black/40 border border-white/5 rounded-2xl">
                  <div className="text-3xl mb-2">🏛️</div>
                  <div className="text-sm font-bold text-white font-sans mb-1">Awaiting DAO Activity</div>
                  <div className="text-xs text-slate-400 font-sans">
                    No active proposals found in the current epoch. Use the button above to create a new decentralized proposal!
                  </div>
                </div>
              )}

              {/* Status Message */}
              <div className="flex flex-col gap-4 mt-6">
                {!isConnected && (
                  <div className="flex flex-col items-center gap-4">
                    <p className="text-xs text-slate-400 font-sans">
                      Connect your wallet to sign the voting parameters.
                    </p>
                    <div className="flex justify-center">
                      <ConnectButton label="Connect Wallet" />
                    </div>
                  </div>
                )}

                {isConnected && address && (
                  <div className="flex flex-col items-center gap-2 text-[10px] font-mono text-slate-500 text-center">
                    <div className="flex items-center gap-2">
                      <span>Signer Address:</span>
                      <AddressDisplay address={address} />
                    </div>
                    {isSuccess && (
                      <div className="mt-4 p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs">
                        <div className="font-bold mb-1">🎉 VOTE CAST SUCCESSFUL!</div>
                        <div>Vote transaction confirmed on Qanto Testnet.</div>
                        {txHash && <div className="mt-1 font-mono text-[10px] break-all">Hash: {txHash}</div>}
                        <div className="mt-1 text-slate-400">
                          Voted {votedDirection} on proposal {votedProposalId ? `${votedProposalId.slice(0, 15)}...` : ''}
                        </div>
                      </div>
                    )}
                  </div>
                )}
                {error && (
                  <div className="text-xs font-mono text-rose-500 bg-rose-500/10 border border-rose-500/20 py-2 px-3 rounded-lg text-left break-words mt-2">
                    ⚠️ Voting Error: {error.message}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
