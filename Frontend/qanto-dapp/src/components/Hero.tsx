import { Link } from 'react-router-dom';

export function Hero() {
  return (
    <div className="relative z-10 w-full">
      {/* Hero Body */}
      <section id="home" className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-20 md:py-32 flex flex-col items-center text-center">
        {/* Glow Effects */}
        <div className="absolute top-1/4 left-1/2 -translate-x-1/2 w-72 h-72 bg-gradient-to-r from-neon-cyan to-quantum-purple rounded-full blur-[100px] opacity-20 pointer-events-none" />

        <div className="max-w-3xl flex flex-col items-center">
          <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full border border-violet-500/30 bg-violet-500/10 text-xs font-semibold text-violet-300 mb-6 uppercase tracking-wider font-mono">
            🛡️ Public Testnet Alpha: Neural Network Active
          </div>

          <h1 className="text-4xl sm:text-6xl font-extrabold tracking-tight text-white mb-6 font-sans leading-tight">
            Quantum-Resistant Blockchain
            <span className="block mt-2 bg-gradient-to-r from-cyan-400 via-violet-400 to-indigo-400 bg-clip-text text-transparent">
              Built for Tomorrow's Threats
            </span>
          </h1>

          <p className="text-base sm:text-xl text-slate-400 font-sans max-w-2xl mb-10 leading-relaxed">
            21 Billion QNTO. Fair Launch. Zero VC. Qanto combines post-quantum security, deterministic DAG finality at 10M+ TPS, and SAGA AI-subsidized micro-fees to establish the cheapest, most secure Layer-0 in Web3.
          </p>

          <div className="flex flex-col sm:flex-row gap-4 justify-center w-full max-w-sm">
            <Link
              to="/airdrop"
              className="px-8 py-4 rounded-xl bg-gradient-to-r from-neon-cyan to-quantum-purple text-white font-bold font-sans text-sm hover:scale-[1.02] transition-transform duration-200 shadow-lg shadow-quantum-purple/10 flex items-center justify-center gap-2 group"
            >
              <span>Claim Pioneer Airdrop</span>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="group-hover:translate-x-0.5 transition-transform">
                <path d="M5 12h14M12 5l7 7-7 7" />
              </svg>
            </Link>
            <Link
              to="/codex"
              className="px-8 py-4 rounded-xl border border-white/10 hover:border-white/20 text-white font-bold font-sans text-sm transition-colors flex items-center justify-center gap-2 bg-white/5"
            >
              <span>ZK-SDK Docs</span>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3" />
              </svg>
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
