import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QuantumCanvas } from './components/QuantumCanvas';
import { Layout } from './components/Layout';
import { Hero } from './components/Hero';
import { TelemetryDashboard } from './components/TelemetryDashboard';
import { HoloExplorer } from './components/HoloExplorer';
import { Governance } from './components/Governance';
import { AirdropClaim } from './components/AirdropClaim';
import { TokenSale } from './components/TokenSale';
import { DEX } from './pages/DEX';
import { Staking } from './pages/Staking';
import { Bridge } from './pages/Bridge';
import { Codex } from './pages/Codex';
import { TechStack } from './components/TechStack';

// Home view displaying the main landing components
const Home = () => {
  return (
    <div className="space-y-12">
      <Hero />
      <div className="py-8">
        <TelemetryDashboard />
      </div>
      <div className="py-8">
        <TechStack />
      </div>
      <div className="py-12 border-t border-white/5 bg-black/20 rounded-[28px] overflow-hidden">
        <TokenSale />
      </div>
    </div>
  );
};

export default function App() {
  return (
    <BrowserRouter>
      <div className="relative min-h-screen text-slate-100 flex flex-col">
        {/* 3D Canvas Background Mesh */}
        <QuantumCanvas />

        {/* Routed Views */}
        <div className="relative z-10 flex-grow">
          <Routes>
            <Route path="/" element={<Layout />}>
              <Route index element={<Home />} />
              <Route path="explorer" element={<HoloExplorer />} />
              <Route path="dex" element={<DEX />} />
              <Route path="staking" element={<Staking />} />
              <Route path="bridge" element={<Bridge />} />
              <Route path="airdrop" element={<AirdropClaim />} />
              <Route path="codex" element={<Codex />} />
              <Route path="saga" element={<Governance />} />
            </Route>
          </Routes>
        </div>

        {/* Futuristic footer */}
        <footer className="relative z-10 border-t border-white/5 bg-black/40 py-10 text-center text-xs text-slate-500 font-mono">
          <div className="max-w-7xl mx-auto px-4 flex flex-col md:flex-row justify-between items-center gap-6">
            <div className="text-center md:text-left space-y-1">
              <div>&copy; {new Date().getFullYear()} QANTO LAYER-0 MESH. ALL RIGHTS RESERVED.</div>
              <div className="text-slate-600 font-sans">
                SECURED BY POST-QUANTUM CRYPTO (CRYSTALS-DILITHIUM)
              </div>
            </div>
            <div className="flex flex-wrap justify-center items-center gap-6 text-sm">
              <a 
                href="https://github.com/trvorth/qanto" 
                target="_blank" 
                rel="noopener noreferrer" 
                className="flex items-center gap-2 text-slate-400 hover:text-cyan-400 transition-colors"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4" /><path d="M9 18c-4.51 2-5-2-7-2" /></svg>
                GitHub
              </a>
              <a 
                href="https://x.com/QantoLayer0" 
                target="_blank" 
                rel="noopener noreferrer" 
                className="flex items-center gap-2 text-slate-400 hover:text-cyan-400 transition-colors"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 4l11.733 16h4.267l-11.733 -16z" /><path d="M4 20l6.768 -6.768m2.46 -2.46l6.772 -6.772" /></svg>
                X (Twitter)
              </a>
              <a 
                href="https://huggingface.co/spaces/trvorth/qanto-testnet" 
                target="_blank" 
                rel="noopener noreferrer" 
                className="flex items-center gap-2 text-slate-400 hover:text-cyan-400 transition-colors"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect width="20" height="8" x="2" y="2" rx="2" ry="2" /><rect width="20" height="8" x="2" y="14" rx="2" ry="2" /><line x1="6" x2="6.01" y1="6" y2="6" /><line x1="6" x2="6.01" y1="18" y2="18" /><line x1="10" x2="10.01" y1="6" y2="6" /><line x1="10" x2="10.01" y1="18" y2="18" /></svg>
                HF Space Node
              </a>
            </div>
          </div>
        </footer>
      </div>
    </BrowserRouter>
  );
}
