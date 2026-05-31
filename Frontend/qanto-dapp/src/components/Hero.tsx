import { ConnectButton } from '@rainbow-me/rainbowkit';

interface HeroProps {
  onAirdropClick: () => void;
  onSaleClick: () => void;
}

export function Hero({ onAirdropClick, onSaleClick }: HeroProps) {
  return (
    <div className="relative z-10 w-full">
      {/* Navigation Header */}
      <nav className="border-b border-white/5 backdrop-blur-md bg-black/40 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16 md:h-20">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full border-2 border-cyan-500/50 flex items-center justify-center bg-black/60 shadow-quantum-glow">
                <span className="text-cyan-400 font-mono font-bold text-lg">Q</span>
              </div>
              <span className="text-white font-sans font-bold text-lg tracking-wider">Qanto</span>
            </div>

            {/* Nav Menu */}
            <div className="hidden lg:flex items-center gap-6 text-sm font-medium text-slate-300">
              <a href="#home" className="hover:text-white transition-colors">Home</a>
              <a href="https://qanto.org/dex" className="hover:text-white transition-colors">DEX</a>
              <a href="https://qanto.org/staking" className="hover:text-white transition-colors">Staking</a>
              <a href="https://qanto.org/bridge" className="hover:text-white transition-colors">Bridge</a>
              <a href="#airdrop" onClick={(e) => { e.preventDefault(); onAirdropClick(); }} className="text-cyan-400 hover:text-cyan-300 transition-colors">Airdrop</a>
              <a href="#tge" onClick={(e) => { e.preventDefault(); onSaleClick(); }} className="hover:text-white hover:text-cyan-400 transition-colors">TGE Sale</a>
              <a href="https://qanto.org/codex" className="hover:text-white transition-colors">Codex</a>
              <a href="https://qanto.org/analytics" className="hover:text-white transition-colors">Analytics</a>
            </div>

            {/* Wallet Connect Button */}
            <div className="flex items-center gap-4">
              <ConnectButton showBalance={false} chainStatus="icon" />
            </div>
          </div>
        </div>
      </nav>

      {/* Hero Body */}
      <section id="home" className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-20 md:py-32 flex flex-col items-center text-center">
        {/* Glow Effects */}
        <div className="absolute top-1/4 left-1/2 -translate-x-1/2 w-72 h-72 bg-gradient-to-r from-neon-cyan to-quantum-purple rounded-full blur-[100px] opacity-20 pointer-events-none" />

        <div className="max-w-3xl flex flex-col items-center">
          <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full border border-violet-500/30 bg-violet-500/10 text-xs font-semibold text-violet-300 mb-6 uppercase tracking-wider font-mono">
            🛡️ Mainnet Alpha: Neural Network Active
          </div>

          <h1 className="text-4xl sm:text-6xl font-extrabold tracking-tight text-white mb-6 font-sans leading-tight">
            Quantum-Resistant Blockchain
            <span className="block mt-2 bg-gradient-to-r from-cyan-400 via-violet-400 to-indigo-400 bg-clip-text text-transparent">
              Built for Tomorrow's Threats
            </span>
          </h1>

          <p className="text-base sm:text-xl text-slate-400 font-sans max-w-2xl mb-10 leading-relaxed">
            Qanto combines post-quantum security, deterministic DAG finality, and SAGA AI-governed treasury execution to establish the foundation of self-sovereign Web3.
          </p>

          <div className="flex flex-col sm:flex-row gap-4 justify-center w-full max-w-sm">
            <button
              onClick={onAirdropClick}
              className="px-8 py-4 rounded-xl bg-gradient-to-r from-neon-cyan to-quantum-purple text-white font-bold font-sans text-sm hover:scale-[1.02] transition-transform duration-200 shadow-lg shadow-quantum-purple/10 flex items-center justify-center gap-2 group"
            >
              <span>Claim Pioneer Airdrop</span>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" className="group-hover:translate-x-0.5 transition-transform">
                <path d="M5 12h14M12 5l7 7-7 7" />
              </svg>
            </button>
            <a
              href="https://qanto.org/doc"
              target="_blank"
              rel="noopener noreferrer"
              className="px-8 py-4 rounded-xl border border-white/10 hover:border-white/20 text-white font-bold font-sans text-sm transition-colors flex items-center justify-center gap-2 bg-white/5"
            >
              <span>ZK-SDK Docs</span>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3" />
              </svg>
            </a>
          </div>
        </div>
      </section>
    </div>
  );
}
