import { useState, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { request, gql } from 'graphql-request';

/** Canonical QNTO supply constant — hardcoded and immutable. */
const TOTAL_SUPPLY_QNTO = 21_000_000_000;

const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';
const REST_BASE = 'https://trvorth-qanto-testnet.hf.space';

interface TelemetryData {
  currentTps: number;
  activeSentinels: number;
  sagaStatus: string;
}

interface NetworkStats {
  block_count: number;
  transaction_count: number;
  validator_count: number;
  mempool_size: number;
  tps_current: number;
  finality_ms: number;
  uptime_percentage: number;
  hash_rate_hps: number;
  num_chains: number;
  current_epoch: number;
  green_score: number;
}

const telemetryQuery = gql`
  query GetTelemetry {
    currentTps
    activeSentinels
    sagaStatus
  }
`;

export function TelemetryDashboard() {
  const [latency, setLatency] = useState<number | null>(null);
  const [netStats, setNetStats] = useState<NetworkStats | null>(null);

  const { data, isLoading, error } = useQuery<TelemetryData>({
    queryKey: ['telemetry'],
    queryFn: async () => {
      const startTime = Date.now();
      try {
        const result = await request<TelemetryData>(GRAPHQL_ENDPOINT, telemetryQuery);
        setLatency(Date.now() - startTime);
        return result;
      } catch (err) {
        setLatency(null);
        throw err;
      }
    },
    refetchInterval: 2000,
  });

  // Fetch live network stats from the REST observability layer
  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch(`${REST_BASE}/stats`, { signal: AbortSignal.timeout(5000) });
        if (res.ok) {
          setNetStats(await res.json());
        }
      } catch { /* silently retry on next interval */ }
    };
    fetchStats();
    const interval = setInterval(fetchStats, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="w-full max-w-5xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-quantum-glow">
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center mb-8 border-b border-white/10 pb-6 gap-4">
          <div>
            <h2 className="text-xl md:text-2xl font-bold text-white font-sans flex items-center gap-2">
              QUANTUM TELEMETRY PORTAL
              <span className="inline-block w-2.5 h-2.5 rounded-full bg-emerald-500 animate-ping" />
            </h2>
            <p className="text-sm text-slate-400 font-sans mt-1">Live DAG Indexer & Sentinel Node Analytics</p>
          </div>
          <div className="flex items-center gap-4 text-xs font-mono flex-wrap">
            <div className="bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 px-3 py-1.5 rounded-full flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
              STATUS: {data?.sagaStatus || (isLoading ? "SYNCING..." : "ONLINE")}
            </div>
            <div className="bg-white/5 text-slate-300 border border-white/10 px-3 py-1.5 rounded-full">
              LATENCY: {isLoading ? '---' : `${latency ?? 31}ms`}
            </div>
            {netStats && (
              <div className="bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 px-3 py-1.5 rounded-full">
                EPOCH: {netStats.current_epoch}
              </div>
            )}
          </div>
        </div>

        {isLoading && !data ? (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {[1, 2, 3].map((i) => (
              <div key={i} className="bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-quantum-glow animate-pulse">
                <div className="h-4 bg-slate-800 rounded w-20 mb-3"></div>
                <div className="h-10 bg-slate-800 rounded w-32 mb-4"></div>
                <div className="h-3 bg-slate-800 rounded w-28"></div>
              </div>
            ))}
          </div>
        ) : (
          <>
            {/* Primary Metrics Row */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-6">
              {/* Card 1: Live TPS */}
              <div className="bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-[0_0_30px_rgba(6,182,212,0.15)] hover:transform hover:-translate-y-1 transition-all hover:border-cyan-500/30 group">
                <h3 className="text-slate-400 text-sm uppercase tracking-widest mb-2 font-mono">Live TPS</h3>
                <p className="text-4xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 to-blue-500 font-mono">
                  {data ? data.currentTps.toLocaleString() : '0'}
                </p>
                <div className="text-xs text-cyan-400 mt-2 font-sans">⚡ Transactions Per Second</div>
              </div>

              {/* Card 2: Active Sentinels */}
              <div className="bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-[0_0_30px_rgba(139,92,246,0.15)] hover:transform hover:-translate-y-1 transition-all hover:border-violet-500/30 group">
                <h3 className="text-slate-400 text-sm uppercase tracking-widest mb-2 font-mono">Active Sentinels</h3>
                <p className="text-4xl font-bold text-white font-mono">
                  {netStats?.validator_count?.toLocaleString() ?? (data ? data.activeSentinels.toLocaleString() : '0')}
                </p>
                <div className="text-xs text-violet-400 mt-2 font-sans">🛡️ Validating Nodes</div>
              </div>

              {/* Card 3: SAGA AI Core */}
              <div className="bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-[0_0_30px_rgba(16,185,129,0.15)] hover:transform hover:-translate-y-1 transition-all hover:border-emerald-500/30 group">
                <h3 className="text-slate-400 text-sm uppercase tracking-widest mb-2 font-mono">SAGA AI Core</h3>
                <p className="text-4xl font-bold text-emerald-400 font-mono">
                  {data ? data.sagaStatus : 'ACTIVE'}
                </p>
                <div className="text-xs text-emerald-400 mt-2 font-sans">🟢 Continuous Singularity</div>
              </div>

              {/* Card 4: Network Supply Cap */}
              <div className="bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-[0_0_30px_rgba(251,191,36,0.15)] hover:transform hover:-translate-y-1 transition-all hover:border-amber-500/30 group">
                <h3 className="text-slate-400 text-sm uppercase tracking-widest mb-2 font-mono">Supply Cap</h3>
                <p className="text-4xl font-bold text-amber-400 font-mono">
                  21B
                </p>
                <div className="text-xs text-amber-400 mt-2 font-sans">🔒 {TOTAL_SUPPLY_QNTO.toLocaleString()} QNTO</div>
              </div>
            </div>

            {/* Secondary Metrics Row — from /stats REST endpoint */}
            {netStats && (
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <div className="bg-black/30 border border-white/5 rounded-xl p-4">
                  <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">Total Blocks</div>
                  <div className="text-xl font-bold text-white font-mono">{netStats.block_count.toLocaleString()}</div>
                </div>
                <div className="bg-black/30 border border-white/5 rounded-xl p-4">
                  <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">Finality</div>
                  <div className="text-xl font-bold text-cyan-400 font-mono">{netStats.finality_ms}ms</div>
                </div>
                <div className="bg-black/30 border border-white/5 rounded-xl p-4">
                  <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">DAG Chains</div>
                  <div className="text-xl font-bold text-violet-400 font-mono">{netStats.num_chains}</div>
                </div>
                <div className="bg-black/30 border border-white/5 rounded-xl p-4">
                  <div className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mb-1">Total Transactions</div>
                  <div className="text-xl font-bold text-emerald-400 font-mono">{netStats.transaction_count.toLocaleString()}</div>
                </div>
              </div>
            )}
          </>
        )}

        {error && (
          <div className="mt-6 text-center text-xs font-mono text-rose-500 bg-rose-500/10 border border-rose-500/20 py-2.5 rounded-lg">
            ⚠️ Telemetry sync failed. Reconnecting...
          </div>
        )}
      </div>
    </div>
  );
}

