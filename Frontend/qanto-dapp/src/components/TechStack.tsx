import { Lock, Layers, Network, Brain, EyeOff, Zap } from 'lucide-react';

export function TechStack() {
  const techCards = [
    {
      icon: <Lock className="w-8 h-8 text-cyan-400" />,
      title: 'Lattice Cryptography',
      description: 'Secured by post-quantum Crystals-Dilithium signature algorithms resistant to global decryption vectors.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(6,182,212,0.25)] hover:border-cyan-500/30'
    },
    {
      icon: <Layers className="w-8 h-8 text-purple-400" />,
      title: 'Holographic State Mesh',
      description: 'Spherically propagating transaction waves traversing lock-free concurrent runtime environments.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(139,92,246,0.25)] hover:border-purple-500/30'
    },
    {
      icon: <Network className="w-8 h-8 text-blue-400" />,
      title: 'Deterministic DAG Finality',
      description: 'Continuous event-driven DAG architecture ensuring final confirmation latency of under 200 milliseconds.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(59,130,246,0.25)] hover:border-blue-500/30'
    },
    {
      icon: <Brain className="w-8 h-8 text-emerald-400" />,
      title: 'SAGA AI Core',
      description: 'Autonomous AI treasury dispatcher continuously balancing network throttles and emission ratios.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(16,185,129,0.25)] hover:border-emerald-500/30'
    },
    {
      icon: <EyeOff className="w-8 h-8 text-amber-400" />,
      title: 'ZK-Proofs',
      description: 'Zero-Knowledge Merkle state validation protecting ledger verification logic and user identity.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(245,158,11,0.25)] hover:border-amber-500/30'
    },
    {
      icon: <Zap className="w-8 h-8 text-rose-400" />,
      title: 'Rust-Native Kernel',
      description: 'High-performance engine optimized at hardware limits for maximum thread-concurrency and security.',
      glowClass: 'hover:shadow-[0_0_30px_rgba(244,63,94,0.25)] hover:border-rose-500/30'
    }
  ];

  return (
    <div className="w-full max-w-5xl mx-auto px-4 py-8 relative z-10">
      <div className="text-center mb-10">
        <h2 className="text-2xl md:text-3xl font-bold text-white font-sans tracking-tight">
          QANTO TECHNOLOGY STACK
        </h2>
        <p className="text-sm text-slate-400 font-sans mt-2">
          The sovereign pillars powering our post-quantum Layer-0 infrastructure.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {techCards.map((card, idx) => (
          <div
            key={idx}
            className={`bg-[#0a0a14]/60 backdrop-blur-md border border-white/10 rounded-[22px] p-6 hover:transform hover:-translate-y-1.5 transition-all duration-300 flex flex-col items-start text-left ${card.glowClass}`}
          >
            <div className="mb-4 bg-white/5 p-3 rounded-xl border border-white/5">
              {card.icon}
            </div>
            <h3 className="text-lg font-bold text-white mb-2 font-sans">
              {card.title}
            </h3>
            <p className="text-xs text-slate-400 leading-relaxed font-sans flex-grow">
              {card.description}
            </p>
          </div>
        ))}
      </div>
    </div>
  );
}
