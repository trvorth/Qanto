import { useCallback, useEffect, useRef, useState } from 'react';
import {
  type BlockSummary,
  type MempoolEntry,
  type NetworkStats,
  type TransactionSummary,
  fetchBlocks,
  fetchHealth,
  fetchMempool,
  fetchNetworkStats,
  fetchTransactions,
} from '../api';

const DEFAULT_POLL_MS = Number(import.meta.env.VITE_REFRESH_INTERVAL_MS) || 3000;

export interface LiveData {
  stats: NetworkStats | null;
  blocks: BlockSummary[];
  transactions: TransactionSummary[];
  mempool: MempoolEntry[];
  isConnected: boolean;
  lastError: string | null;
}

export function useLiveData(pollMs: number = DEFAULT_POLL_MS): LiveData {
  const [stats, setStats] = useState<NetworkStats | null>(null);
  const [blocks, setBlocks] = useState<BlockSummary[]>([]);
  const [transactions, setTransactions] = useState<TransactionSummary[]>([]);
  const [mempool, setMempool] = useState<MempoolEntry[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const abortRef = useRef<AbortController | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mempoolTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const fetchLoop = useCallback(async () => {
    const ac = new AbortController();
    abortRef.current?.abort();
    abortRef.current = ac;

    try {
      await fetchHealth(); // quick liveness probe
      const [statsRes, blocksRes, txsRes] = await Promise.all([
        fetchNetworkStats(),
        fetchBlocks(25, 0),
        fetchTransactions(25, 0),
      ]);

      if (!ac.signal.aborted) {
        setStats(statsRes);
        setBlocks(blocksRes.data);
        setTransactions(txsRes.data);
        setIsConnected(true);
        setLastError(null);
      }
    } catch (err: any) {
      if (!ac.signal.aborted) {
        setIsConnected(false);
        setLastError(err.message ?? 'Connection refused');
      }
    }
  }, []);

  const fetchMempoolLoop = useCallback(async () => {
    try {
      const entries = await fetchMempool();
      setMempool(entries);
    } catch {
      // mempool is optional, don't flag as error
    }
  }, []);

  useEffect(() => {
    fetchLoop();
    fetchMempoolLoop();

    timerRef.current = setInterval(fetchLoop, pollMs);
    mempoolTimerRef.current = setInterval(fetchMempoolLoop, pollMs * 2);

    return () => {
      abortRef.current?.abort();
      if (timerRef.current) clearInterval(timerRef.current);
      if (mempoolTimerRef.current) clearInterval(mempoolTimerRef.current);
    };
  }, [fetchLoop, fetchMempoolLoop, pollMs]);

  return { stats, blocks, transactions, mempool, isConnected, lastError };
}