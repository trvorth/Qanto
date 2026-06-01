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
        <footer className="relative z-10 border-t border-white/5 bg-black/40 py-8 text-center text-xs text-slate-500 font-mono">
          <div>&copy; {new Date().getFullYear()} QANTO LAYER-0 MESH. ALL RIGHTS RESERVED.</div>
          <div className="mt-1 text-slate-600 font-sans">
            SECURED BY POST-QUANTUM CRYPTO (CRYSTALS-DILITHIUM)
          </div>
        </footer>
      </div>
    </BrowserRouter>
  );
}
