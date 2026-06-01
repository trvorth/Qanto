import { useState } from 'react';

export const Codex = () => {
  const [activeTab, setActiveTab] = useState('architecture');

  const docTabs = [
    { id: 'architecture', name: 'Architecture' },
    { id: 'consensus', name: 'Consensus' },
    { id: 'cryptography', name: 'Cryptography' },
    { id: 'tokenomics', name: 'Tokenomics' }
  ];

  return (
    <div className="flex flex-col md:flex-row gap-8 w-full max-w-6xl mx-auto py-4">
      {/* Left Sidebar Navigation */}
      <div className="w-full md:w-1/4 flex flex-col gap-2">
        <div className="backdrop-blur-md bg-white/[0.01] border border-white/5 rounded-2xl p-4">
          <h3 className="text-xl font-bold mb-4 bg-gradient-to-r from-cyan-400 to-violet-400 bg-clip-text text-transparent font-sans tracking-tight">
            The Codex
          </h3>
          <div className="flex flex-col gap-1.5">
            {docTabs.map(tab => (
              <button 
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`text-left px-4 py-3 rounded-xl transition-all duration-200 border text-sm font-medium font-sans ${
                  activeTab === tab.id 
                    ? 'bg-cyan-500/10 border-cyan-500/30 text-cyan-400 shadow-[0_0_15px_rgba(6,182,212,0.15)]' 
                    : 'bg-transparent border-transparent text-slate-400 hover:text-white hover:bg-white/5'
                }`}
              >
                {tab.name}
              </button>
            ))}
          </div>
        </div>
      </div>
      
      {/* Right Content Area */}
      <div className="w-full md:w-3/4 bg-white/[0.02] border border-white/10 rounded-3xl p-6 md:p-8 backdrop-blur-xl relative overflow-hidden shadow-purple-glow">
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(6,182,212,0.02)_0%,transparent_60%)] pointer-events-none" />
        
        <div className="relative z-10">
          {activeTab === 'architecture' && (
            <div className="prose prose-invert max-w-none">
              <h1 className="text-3xl font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 via-indigo-300 to-purple-500 mb-6 font-sans tracking-tight">
                Holographic State Mesh
              </h1>
              <p className="text-slate-300 leading-relaxed text-base mb-6 font-sans">
                QANTO discards traditional linear blockchain architectures in favor of a <strong>Holographic State Mesh (HSM)</strong>. In the HSM model, transaction waves propagate spherically through a lock-free, concurrent Rust environment. Every consensus node maintains a localized holographic slice of the state, allowing sub-millisecond local validations before global state finality is reached.
              </p>
              <div className="bg-black/40 border border-white/5 rounded-xl p-5 mb-6 font-mono text-xs text-slate-400 leading-relaxed">
                <span className="text-cyan-400 font-bold">QantoOS Engine Core:</span>
                <pre className="mt-2 text-[11px] text-cyan-300/80 overflow-x-auto">
{`struct StateWave {
    epoch: u64,
    merkle_root: [u8; 32],
    holographic_vector: Vec<f64>,
    quantum_entropy: u256,
}`}
                </pre>
              </div>
              <h2 className="text-xl font-bold text-white mb-3 font-sans">Key Architectural Features</h2>
              <ul className="list-disc list-inside text-slate-300 space-y-2 mb-6 text-sm font-sans">
                <li><strong>Lock-free Concurrency:</strong> Zero-mutex execution pipelines optimized for AMD Threadripper and modern multi-core server nodes.</li>
                <li><strong>Dynamic State Sharding:</strong> The state mesh divides and merges autonomously based on transaction density waves, preventing gas spikes.</li>
                <li><strong>State Wave Propagation:</strong> Sub-second transaction confirmation times leveraging a modified Kademlia routing architecture.</li>
              </ul>
            </div>
          )}

          {activeTab === 'consensus' && (
            <div className="prose prose-invert max-w-none">
              <h1 className="text-3xl font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 via-indigo-300 to-purple-500 mb-6 font-sans tracking-tight">
                Deterministic DAG Finality
              </h1>
              <p className="text-slate-300 leading-relaxed text-base mb-6 font-sans">
                QANTO achieves finality through a <strong>Deterministic Directed Acyclic Graph (DAG)</strong> consensus mechanism. Unlike traditional Proof-of-Work or standard Proof-of-Stake protocols, transactions are directly linked to previous transactions, acting as validation checkpoints. This structural design guarantees that double-spends are mathematically impossible.
              </p>
              <div className="border-l-4 border-violet-500 bg-violet-950/20 rounded-r-xl p-5 mb-6 text-sm text-slate-300 font-sans leading-relaxed">
                <strong>Formal Consensus Bound:</strong> QANTO finality satisfies a bounded asynchronous assumption. If network partitions occur, the DAG branches independently and merges deterministically using Liveness Edge Selection algorithms.
              </div>
              <h2 className="text-xl font-bold text-white mb-3 font-sans">Consensus Metrics</h2>
              <table className="w-full text-left border-collapse border border-white/5 text-sm font-sans mb-6">
                <thead>
                  <tr className="bg-white/5 text-slate-200 font-semibold border-b border-white/5">
                    <th className="p-3">Parameter</th>
                    <th className="p-3">Specification</th>
                  </tr>
                </thead>
                <tbody className="text-slate-400">
                  <tr className="border-b border-white/5 hover:bg-white/[0.01]">
                    <td className="p-3 font-mono">Block Time</td>
                    <td className="p-3">Continuous (No Blocks, Event-Driven)</td>
                  </tr>
                  <tr className="border-b border-white/5 hover:bg-white/[0.01]">
                    <td className="p-3 font-mono">Finality Latency</td>
                    <td className="p-3">~ 200 ms (Deterministic)</td>
                  </tr>
                  <tr className="border-b border-white/5 hover:bg-white/[0.01]">
                    <td className="p-3 font-mono">Throughput Cap</td>
                    <td className="p-3">10M+ Transactions Per Second (TPS)</td>
                  </tr>
                </tbody>
              </table>
            </div>
          )}

          {activeTab === 'cryptography' && (
            <div className="prose prose-invert max-w-none">
              <h1 className="text-3xl font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 via-indigo-300 to-purple-500 mb-6 font-sans tracking-tight">
                Post-Quantum Cryptography
              </h1>
              <p className="text-slate-300 leading-relaxed text-base mb-6 font-sans">
                Conventional cryptography (ECDSA, RSA) is vulnerable to quantum computer attacks. QANTO is secured natively using <strong>Crystals-Dilithium</strong> lattice-based signature schemes and <strong>Crystals-Kyber</strong> for key exchange protocols. This safeguards all on-chain funds and state variables against future decrypt-now-decrypt-later vectors.
              </p>
              <h2 className="text-xl font-bold text-white mb-3 font-sans">Lattice Cryptographic Specs</h2>
              <ul className="list-disc list-inside text-slate-300 space-y-2 mb-6 text-sm font-sans">
                <li><strong>Dilithium-5:</strong> Provides security parameters matching Level 5 (equivalent to AES-256 brute force complexity).</li>
                <li><strong>Zero-Knowledge Lattice Proofs:</strong> Allows state transitions to be proved without revealing sender addresses or transaction amounts.</li>
                <li><strong>Compact Public Keys:</strong> Key size optimized to ~1.9KB to ensure low bandwidth consumption during high network loads.</li>
              </ul>
            </div>
          )}

          {activeTab === 'tokenomics' && (
            <div className="prose prose-invert max-w-none">
              <h1 className="text-3xl font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 via-indigo-300 to-purple-500 mb-6 font-sans tracking-tight">
                QNTO Token Distribution
              </h1>
              <p className="text-slate-300 leading-relaxed text-base mb-6 font-sans">
                The native asset of the Qanto Layer-0 Mesh is <strong>$QNTO</strong>. It serves as gas fee payment, staking collateral for Sentinel Nodes, and governance voting weight.
              </p>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-6">
                <div className="bg-black/30 border border-white/5 rounded-xl p-4 text-center">
                  <div className="text-xs text-slate-500 font-mono">TOTAL SUPPLY</div>
                  <div className="text-xl font-bold text-cyan-400 font-mono mt-1">100,000,000</div>
                </div>
                <div className="bg-black/30 border border-white/5 rounded-xl p-4 text-center">
                  <div className="text-xs text-slate-500 font-mono">TGE SALE</div>
                  <div className="text-xl font-bold text-purple-400 font-mono mt-1">25,000,000</div>
                </div>
                <div className="bg-black/30 border border-white/5 rounded-xl p-4 text-center">
                  <div className="text-xs text-slate-500 font-mono">STAKING EMISSIONS</div>
                  <div className="text-xl font-bold text-indigo-400 font-mono mt-1">45,000,000</div>
                </div>
              </div>
              <h2 className="text-xl font-bold text-white mb-3 font-sans">Allocation Breakdown</h2>
              <ul className="list-disc list-inside text-slate-300 space-y-2 mb-6 text-sm font-sans">
                <li><strong>45% (45M $QNTO):</strong> Stakeholder Rewards & Sentinel Node Incentives (emitted linearly over 120 months).</li>
                <li><strong>25% (25M $QNTO):</strong> Public Token Generation Event (TGE).</li>
                <li><strong>15% (15M $QNTO):</strong> Ecosystem Development, ZK-SDK Grants, and Core Dev Funding.</li>
                <li><strong>10% (10M $QNTO):</strong> Pioneer Genesis Airdrop (ZKP verified).</li>
                <li><strong>5% (5M $QNTO):</strong> Liquidity pools and initial market support.</li>
              </ul>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
export default Codex;
