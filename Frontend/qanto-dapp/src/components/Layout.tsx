import { Outlet, Link, useLocation } from 'react-router-dom';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import { Toaster } from 'react-hot-toast';

export const Layout = () => {
  const location = useLocation();
  const navLinks = [
    { name: 'Home', path: '/' },
    { name: 'Explorer', path: '/explorer' },
    { name: 'DEX', path: '/dex' },
    { name: 'Staking', path: '/staking' },
    { name: 'Bridge', path: '/bridge' },
    { name: 'Airdrop', path: '/airdrop' },
    { name: 'Codex', path: '/codex' },
    { name: 'SAGA AI', path: '/saga' }
  ];

  return (
    <div className="min-h-screen bg-[#050505] text-white font-sans selection:bg-cyan-500/30">
      <Toaster 
        position="top-right" 
        toastOptions={{ 
          style: { 
            background: '#1e293b', 
            color: '#fff', 
            border: '1px solid rgba(6,182,212,0.3)',
            borderRadius: '12px',
            fontFamily: 'sans-serif'
          } 
        }} 
      />
      
      {/* Global Navigation - Fixed padding and alignment */}
      <nav className="w-full border-b border-white/5 bg-black/50 backdrop-blur-md sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-20 flex items-center justify-between">
          <Link to="/" className="text-2xl font-bold tracking-tighter flex items-center gap-2">
            <span className="w-8 h-8 rounded-full bg-gradient-to-tr from-cyan-500 to-purple-600 shadow-[0_0_15px_rgba(6,182,212,0.5)]"></span>
            QANTO
          </Link>
          <div className="hidden md:flex space-x-1">
            {navLinks.map(link => (
              <Link 
                key={link.name} 
                to={link.path} 
                className={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                  location.pathname === link.path 
                    ? 'bg-white/10 text-cyan-400 shadow-[0_0_10px_rgba(6,182,212,0.15)]' 
                    : 'text-slate-400 hover:text-white hover:bg-white/5'
                }`}
              >
                {link.name}
              </Link>
            ))}
          </div>
          <div className="flex items-center">
            <ConnectButton showBalance={false} chainStatus="icon" />
          </div>
        </div>
      </nav>

      {/* Global Main Content Wrapper - Fixes margin efficiency */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        <Outlet />
      </main>
    </div>
  );
};
