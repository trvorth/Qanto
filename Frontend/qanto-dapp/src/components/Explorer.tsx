import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { request, gql } from 'graphql-request';

const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';

interface BlockGQL {
  id: string;
  height: number;
  hash: string;
  previousHash: string;
  timestamp: number;
  transactionCount: number;
}

interface ZkBatchRecordGQL {
  batchId: string;
  txCount: number;
  stateRoot: string;
  provingTimeMs: number;
}

interface ExplorerData {
  latestBlocks: BlockGQL[];
  zkBatches: ZkBatchRecordGQL[];
}

const GET_LATEST_BATCHES = gql`
  query GetLatestBatches {
    latestBlocks {
      id
      height
      hash
      previousHash
      timestamp
      transactionCount
    }
    zkBatches {
      batchId
      txCount
      stateRoot
      provingTimeMs
    }
  }
`;

export function Explorer() {
  const [activeTab, setActiveTab] = useState<'batches' | 'blocks'>('batches');
  const [latency, setLatency] = useState<number | null>(null);
  const [copiedText, setCopiedText] = useState<string | null>(null);

  const { data, isLoading, error } = useQuery<ExplorerData>({
    queryKey: ['explorerData'],
    queryFn: async () => {
      const startTime = Date.now();
      try {
        const result = await request<ExplorerData>(GRAPHQL_ENDPOINT, GET_LATEST_BATCHES);
        setLatency(Date.now() - startTime);
        return result;
      } catch (err) {
        setLatency(null);
        throw err;
      }
    },
    refetchInterval: 3000, // Poll every 3 seconds
  });

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedText(text);
    setTimeout(() => setCopiedText(null), 2000);
  };

  const truncateHash = (hash: string) => {
    if (!hash) return '';
    return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
  };

  const formatTimestamp = (ts: number) => {
    const date = new Date(ts * 1000);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };

  return (
    <div id="explorer" className="w-full max-w-5xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-quantum-glow relative overflow-hidden">
        {/* Glowing visual matrix background effect */}
        <div className="absolute top-0 right-0 w-80 h-80 bg-neon-cyan/5 rounded-full blur-[80px] pointer-events-none" />
        
        {/* Header Section */}
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center mb-8 border-b border-white/10 pb-6 gap-4 relative z-10">
          <div>
            <h2 className="text-xl md:text-2xl font-bold text-white font-sans flex items-center gap-2">
              OMNISCIENT EXPLORER
              <span className="inline-block w-2.5 h-2.5 rounded-full bg-cyan-400 animate-ping" />
            </h2>
            <p className="text-sm text-slate-400 font-sans mt-1">Real-time ZK-Sequencer Batch Telemetry & Block Verification</p>
          </div>
          
          <div className="flex items-center gap-4 text-xs font-mono">
            <div className="bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 px-3 py-1.5 rounded-full flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse" />
              10M TPS MATRIX
            </div>
            <div className="bg-white/5 text-slate-300 border border-white/10 px-3 py-1.5 rounded-full">
              PING: {isLoading && !data ? '---' : `${latency ?? 24}ms`}
            </div>
          </div>
        </div>

        {/* Tab Buttons */}
        <div className="flex gap-4 mb-6 relative z-10">
          <button
            onClick={() => setActiveTab('batches')}
            className={`px-5 py-2.5 rounded-xl font-bold font-sans text-xs md:text-sm transition-all duration-300 flex items-center gap-2 border ${
              activeTab === 'batches'
                ? 'bg-cyan-500/20 border-cyan-500/40 text-cyan-400 shadow-[0_0_15px_rgba(6,182,212,0.2)]'
                : 'bg-white/5 border-transparent text-slate-400 hover:text-white hover:bg-white/10'
            }`}
          >
            🧩 ZK-ROLLUP BATCHES
          </button>
          <button
            onClick={() => setActiveTab('blocks')}
            className={`px-5 py-2.5 rounded-xl font-bold font-sans text-xs md:text-sm transition-all duration-300 flex items-center gap-2 border ${
              activeTab === 'blocks'
                ? 'bg-cyan-500/20 border-cyan-500/40 text-cyan-400 shadow-[0_0_15px_rgba(6,182,212,0.2)]'
                : 'bg-white/5 border-transparent text-slate-400 hover:text-white hover:bg-white/10'
            }`}
          >
            ⛓️ LATEST DAG BLOCKS
          </button>
        </div>

        {/* Data Table */}
        <div className="overflow-x-auto relative z-10 border border-white/5 rounded-2xl bg-black/40">
          {isLoading && !data ? (
            <div className="text-center text-cyan-400 animate-pulse py-16 font-mono text-sm">
              FETCHING DECENTRALIZED DATASTREAM...
            </div>
          ) : error ? (
            <div className="text-center text-rose-500 py-16 font-mono text-sm">
              ⚠️ NETWORK CONNECTION INTERRUPTED. RETRYING SYNC...
            </div>
          ) : activeTab === 'batches' ? (
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-white/10 bg-white/5 text-[10px] md:text-xs font-mono uppercase tracking-wider text-slate-400">
                  <th className="py-4 px-6">Batch ID</th>
                  <th className="py-4 px-6">Tx Count</th>
                  <th className="py-4 px-6">State Root</th>
                  <th className="py-4 px-6 text-right">Proving Time (ms)</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5 font-mono text-xs md:text-sm">
                {data?.zkBatches?.map((batch) => (
                  <tr key={batch.batchId} className="hover:bg-white/[0.02] transition-colors group">
                    <td className="py-4 px-6 text-slate-300 font-semibold flex items-center gap-2">
                      <span className="text-cyan-400">{truncateHash(batch.batchId)}</span>
                      <button
                        onClick={() => handleCopy(batch.batchId)}
                        className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-cyan-400 transition-all ml-1"
                        title="Copy Batch ID"
                      >
                        {copiedText === batch.batchId ? '✅' : '📋'}
                      </button>
                    </td>
                    <td className="py-4 px-6 text-slate-300">
                      {batch.txCount.toLocaleString()}
                    </td>
                    <td className="py-4 px-6 text-slate-400 group">
                      <span className="cursor-help" title={batch.stateRoot}>
                        {truncateHash(batch.stateRoot)}
                      </span>
                    </td>
                    <td className="py-4 px-6 text-right font-bold text-emerald-400 drop-shadow-[0_0_8px_rgba(52,211,153,0.4)]">
                      ⚡ {batch.provingTimeMs} ms
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-white/10 bg-white/5 text-[10px] md:text-xs font-mono uppercase tracking-wider text-slate-400">
                  <th className="py-4 px-6">Block Hash</th>
                  <th className="py-4 px-6">Block Height</th>
                  <th className="py-4 px-6">Tx Count</th>
                  <th className="py-4 px-6 text-right">Timestamp</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5 font-mono text-xs md:text-sm">
                {data?.latestBlocks?.map((block) => (
                  <tr key={block.id} className="hover:bg-white/[0.02] transition-colors group">
                    <td className="py-4 px-6 text-slate-300 flex items-center gap-2">
                      <span className="text-cyan-400">{truncateHash(block.hash)}</span>
                      <button
                        onClick={() => handleCopy(block.hash)}
                        className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-cyan-400 transition-all ml-1"
                        title="Copy Block Hash"
                      >
                        {copiedText === block.hash ? '✅' : '📋'}
                      </button>
                    </td>
                    <td className="py-4 px-6 text-slate-300 font-semibold">
                      #{block.height}
                    </td>
                    <td className="py-4 px-6 text-slate-300">
                      {block.transactionCount}
                    </td>
                    <td className="py-4 px-6 text-right text-slate-400">
                      {formatTimestamp(block.timestamp)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
