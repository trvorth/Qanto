import { useRef } from 'react';
import { QuantumCanvas } from './components/QuantumCanvas';
import { Hero } from './components/Hero';
import { TelemetryDashboard } from './components/TelemetryDashboard';
import { Explorer } from './components/Explorer';
import { Governance } from './components/Governance';
import { AirdropClaim } from './components/AirdropClaim';
import { TokenSale } from './components/TokenSale';

export default function App() {
  const explorerRef = useRef<HTMLDivElement | null>(null);
  const governanceRef = useRef<HTMLDivElement | null>(null);
  const claimRef = useRef<HTMLDivElement | null>(null);
  const saleRef = useRef<HTMLDivElement | null>(null);

  const scrollToExplorer = () => {
    if (explorerRef.current) {
      explorerRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const scrollToGovernance = () => {
    if (governanceRef.current) {
      governanceRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const scrollToAirdrop = () => {
    if (claimRef.current) {
      claimRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const scrollToSale = () => {
    if (saleRef.current) {
      saleRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  return (
    <div className="relative min-h-screen text-slate-100 flex flex-col">
      {/* 3D Canvas Background Mesh */}
      <QuantumCanvas />

      {/* Main Content Area */}
      <div className="relative z-10 flex-grow">
        {/* Hero Section */}
        <Hero
          onAirdropClick={scrollToAirdrop}
          onSaleClick={scrollToSale}
          onExplorerClick={scrollToExplorer}
          onGovernanceClick={scrollToGovernance}
        />

        {/* Telemetry Dashboard Portal */}
        <div className="py-12">
          <TelemetryDashboard />
        </div>

        {/* Omniscient Explorer Section */}
        <div ref={explorerRef} id="explorer" className="py-12 border-t border-white/5 bg-black/10">
          <Explorer />
        </div>

        {/* Sovereign DAO Governance Section */}
        <div ref={governanceRef} id="governance" className="py-12 border-t border-white/5 bg-black/20">
          <Governance />
        </div>

        {/* Claim Airdrop Section */}
        <div ref={claimRef} id="airdrop" className="py-16 md:py-24 border-t border-white/5">
          <AirdropClaim />
        </div>

        {/* Token Sale (TGE) Section */}
        <div ref={saleRef} id="tge" className="py-16 md:py-24 border-t border-white/5 bg-black/20">
          <TokenSale />
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

