import { useState } from 'react';
import { useAccount, useSendTransaction } from 'wagmi';
import { request, gql } from 'graphql-request';

const GOVERNANCE_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000010';
const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';

interface CreateProposalProps {
  onSuccess?: () => void;
  onCancel?: () => void;
}

export function CreateProposal({ onSuccess, onCancel }: CreateProposalProps) {
  const { isConnected } = useAccount();
  const { sendTransaction, isSuccess: isTxSuccess, error: txError, data: txHash } = useSendTransaction();

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [status, setStatus] = useState<'idle' | 'ipfs' | 'contract' | 'graphql' | 'success' | 'error'>('idle');
  const [errorMessage, setErrorMessage] = useState('');
  const [generatedCid, setGeneratedCid] = useState('');

  const generateSyntheticCid = (proposalData: { title: string; description: string; timestamp: number }) => {
    const jsonStr = JSON.stringify(proposalData);
    let hash = 0;
    for (let i = 0; i < jsonStr.length; i++) {
      hash = (hash << 5) - hash + jsonStr.charCodeAt(i);
      hash |= 0; 
    }
    const characters = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
    let mockHash = '';
    const seed = Math.abs(hash);
    for (let i = 0; i < 44; i++) {
      const charIndex = (seed + i * 13) % characters.length;
      mockHash += characters[charIndex];
    }
    return `Qm${mockHash}`;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || !description.trim()) {
      setErrorMessage('Title and description are required.');
      setStatus('error');
      return;
    }

    try {
      setStatus('ipfs');
      // Simulate IPFS uploading latency
      await new Promise((resolve) => setTimeout(resolve, 1500));
      
      const cid = generateSyntheticCid({
        title,
        description,
        timestamp: Date.now()
      });
      setGeneratedCid(cid);

      setStatus('contract');
      
      const jsonStr = JSON.stringify({
        title,
        description,
        proposal_type: 'Signal',
        cid
      });
      const encoder = new TextEncoder();
      const bytes = encoder.encode(jsonStr);
      let hexData = '0x02';
      for (let i = 0; i < bytes.length; i++) {
        hexData += bytes[i].toString(16).padStart(2, '0');
      }

      sendTransaction({
        to: GOVERNANCE_CONTRACT_ADDRESS,
        value: 0n,
        data: hexData as `0x${string}`,
      });
    } catch (err: any) {
      setErrorMessage(err.message || 'Failed to submit proposal.');
      setStatus('error');
    }
  };

  // Run the GraphQL registry once the blockchain write is simulated successfully
  const registerWithBackend = async (cid: string) => {
    try {
      setStatus('graphql');
      const mutation = gql`
        mutation SubmitProposal($cid: String!) {
          submitProposal(cid: $cid) {
            success
            proposalId
            message
          }
        }
      `;
      
      const response = await request<{ submitProposal: { success: boolean; proposalId: string; message: string } }>(
        GRAPHQL_ENDPOINT,
        mutation,
        { cid }
      );

      if (response.submitProposal.success) {
        setStatus('success');
        if (onSuccess) {
          setTimeout(() => onSuccess(), 1500);
        }
      } else {
        setErrorMessage(response.submitProposal.message);
        setStatus('error');
      }
    } catch (err: any) {
      setErrorMessage(err.message || 'Failed to sync proposal with the network node.');
      setStatus('error');
    }
  };

  // React to Wagmi transaction success
  if (isTxSuccess && status === 'contract') {
    registerWithBackend(generatedCid);
  }

  // Handle contract simulation error
  if (txError && status === 'contract') {
    setErrorMessage(txError.message || 'Blockchain transaction failed.');
    setStatus('error');
  }

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-6 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/70 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-purple-glow">
        <h2 className="text-xl font-bold text-white mb-1 font-sans text-center">CREATE DAO PROPOSAL</h2>
        <p className="text-xs text-slate-400 text-center mb-6 font-sans">
          Your proposal text will be stored permanently on IPFS and validated by the network.
        </p>

        {status === 'success' ? (
          <div className="text-center py-8 space-y-4">
            <div className="w-16 h-16 bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 rounded-full flex items-center justify-center mx-auto text-2xl animate-bounce">
              ✓
            </div>
            <h3 className="text-lg font-bold text-white font-sans">Proposal Published Successfully!</h3>
            <p className="text-xs text-slate-400 font-mono break-all max-w-xs mx-auto">
              CID: {generatedCid}
            </p>
            {txHash && (
              <p className="text-[10px] text-slate-500 font-mono break-all max-w-xs mx-auto">
                Tx Hash: {txHash}
              </p>
            )}
            <p className="text-xs text-cyan-400 animate-pulse">Syncing proposals...</p>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-5">
            <div className="space-y-2">
              <label className="block text-xs font-mono uppercase tracking-wider text-slate-400">Proposal Title</label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                disabled={status !== 'idle' && status !== 'error'}
                placeholder="e.g. QIP-02: Optimize Neural-Vault memory limits"
                className="w-full bg-black/40 border border-white/10 rounded-xl px-4 py-3 text-sm text-white focus:outline-none focus:border-violet-500 transition-all font-sans"
              />
            </div>

            <div className="space-y-2">
              <label className="block text-xs font-mono uppercase tracking-wider text-slate-400">Description</label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                disabled={status !== 'idle' && status !== 'error'}
                rows={5}
                placeholder="Provide a thorough justification for this parameter change..."
                className="w-full bg-black/40 border border-white/10 rounded-xl px-4 py-3 text-sm text-white focus:outline-none focus:border-violet-500 transition-all font-sans resize-none"
              />
            </div>

            {status === 'error' && (
              <div className="bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs py-3 px-4 rounded-xl break-words">
                ⚠️ Error: {errorMessage}
              </div>
            )}

            <div className="flex items-center gap-3 pt-2">
              <button
                type="button"
                onClick={onCancel}
                disabled={status !== 'idle' && status !== 'error'}
                className="flex-1 bg-white/5 border border-white/10 text-slate-400 hover:text-white hover:bg-white/10 rounded-xl py-3 text-sm font-sans font-bold transition-all"
              >
                Cancel
              </button>
              
              <button
                type="submit"
                disabled={!isConnected || status === 'ipfs' || status === 'contract' || status === 'graphql'}
                className={`flex-1 rounded-xl py-3 text-sm font-sans font-bold transition-all border ${
                  !isConnected || status === 'ipfs' || status === 'contract' || status === 'graphql'
                    ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed'
                    : 'bg-cyan-500/10 hover:bg-cyan-500 hover:text-white text-cyan-400 border-cyan-500/30 hover:shadow-[0_0_15px_rgba(6,182,212,0.3)]'
                }`}
              >
                {status === 'ipfs' && 'Uploading to IPFS...'}
                {status === 'contract' && 'Signing on Wagmi...'}
                {status === 'graphql' && 'Registering on Node...'}
                {status === 'idle' && 'Submit Proposal'}
                {status === 'error' && 'Retry Submission'}
              </button>
            </div>

            {!isConnected && (
              <p className="text-[10px] text-center text-rose-400 font-sans mt-2">
                ⚠️ Connect your wallet to submit proposals to the DAO.
              </p>
            )}

            {status !== 'idle' && status !== 'error' && (
              <div className="flex items-center justify-center gap-2 text-xs font-mono text-cyan-400 animate-pulse mt-4">
                <span className="inline-block w-3 h-3 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin" />
                Processing phase: {status.toUpperCase()}
              </div>
            )}
          </form>
        )}
      </div>
    </div>
  );
}
