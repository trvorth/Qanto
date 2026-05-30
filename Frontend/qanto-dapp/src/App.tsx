import { useRef } from 'react';
import { QuantumCanvas } from './components/QuantumCanvas';
import { Hero } from './components/Hero';
import { TelemetryDashboard } from './components/TelemetryDashboard';
import { AirdropClaim } from './components/AirdropClaim';

export default function App() {
  const claimRef = useRef<HTMLDivElement | null>(null);

  const scrollToAirdrop = () => {
    if (claimRef.current) {
      claimRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  return (
    <div className="relative min-h-screen text-slate-100 flex flex-col">
      {/* 3D Canvas Background Mesh */}
      <QuantumCanvas />

      {/* Main Content Area */}
      <div className="relative z-10 flex-grow">
        {/* Hero Section */}
        <Hero onAirdropClick={scrollToAirdrop} />

        {/* Telemetry Dashboard Portal */}
        <div className="py-12">
          <TelemetryDashboard />
        </div>

        {/* Claim Airdrop Section */}
        <div ref={claimRef} id="airdrop" className="py-16 md:py-24">
          <AirdropClaim />
        </div>
      </div>

      {/* Futuristic footer */}
      <footer className="relative z-10 border-t border-white/5 bg-black/40 py-8 text-center text-xs text-slate-500 font-mono">
        <div>&copy; {new Date().getFullYear()} QANTO LAYER-0 MESH. ALL RIGHTS RESERVED.</div>
        <div className="mt-1 text-slate-600">SECURED BY POST-QUANTUM CRYPTO (CRYSTALS-DILITHIUM)</div>
      </footer>
    </div>
  );
}
