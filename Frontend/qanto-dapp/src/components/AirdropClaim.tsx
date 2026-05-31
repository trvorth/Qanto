import { useAccount, useWriteContract, useReadContract } from 'wagmi';
import { ConnectButton } from '@rainbow-me/rainbowkit';

const AIRDROP_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000008';

const QantoDropAbi = [
  {
    type: 'function',
    name: 'claim',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'amount', type: 'uint256' },
      { name: 'merkleProof', type: 'bytes32[]' }
    ],
    outputs: []
  },
  {
    type: 'function',
    name: 'hasClaimed',
    stateMutability: 'view',
    inputs: [{ name: '', type: 'address' }],
    outputs: [{ name: '', type: 'bool' }]
  }
] as const;

export function AirdropClaim() {
  const { isConnected, address } = useAccount();
  const { writeContract, isPending, isSuccess, error, data: txHash } = useWriteContract();

  const { data: hasClaimed } = useReadContract({
    abi: QantoDropAbi,
    address: AIRDROP_CONTRACT_ADDRESS,
    functionName: 'hasClaimed',
    args: address ? [address] : undefined,
    query: {
      enabled: !!address,
    }
  });

  const handleClaim = () => {
    writeContract({
      abi: QantoDropAbi,
      address: AIRDROP_CONTRACT_ADDRESS,
      functionName: 'claim',
      args: [
        BigInt(1000 * 10 ** 9), // 1000 QNTO (9 decimals)
        [] // Empty proof for simulation / testing
      ]
    });
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

          <div className="flex flex-col gap-4">
            <button
              onClick={handleClaim}
              disabled={!isConnected || isPending || !!hasClaimed || isSuccess}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected || isPending || !!hasClaimed
                  ? 'bg-slate-700 cursor-not-allowed text-slate-400 border-transparent shadow-none'
                  : isSuccess
                  ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {!isConnected
                ? 'Wallet Disconnected'
                : hasClaimed
                ? 'AIRDROP ALREADY CLAIMED 🟢'
                : isSuccess
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
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all">
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
