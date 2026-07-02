/**
 * QANTOscan Explorer — API Service Layer
 * Connects to the Qanto node REST API at http://127.0.0.1:8081
 */

// In production, prefer a relative `/api` prefix so the explorer can sit behind
// a reverse proxy without hardcoding the RPC node origin.
export const NODE_API_BASE =
  import.meta.env.VITE_QANTO_NODE_URL ??
  (import.meta.env.DEV ? 'http://127.0.0.1:8081' : '/api');

// ── Types ──────────────────────────────────────────────────────────────────

export interface BlockSummary {
  id: string;
  chain_id: number;
  height: number;
  timestamp: number;
  validator: string;
  miner: string;
  difficulty: number;
  nonce: number;
  tx_count: number;
  reward: string;
  merkle_root: string;
  parents: string[];
  epoch: number;
}

export interface TransactionSummary {
  id: string;
  sender: string;
  receiver: string;
  amount: string;
  fee: string;
  timestamp: number;
  block_id: string;
  block_height: number;
  transaction_kind: string;
}

export interface NetworkStats {
  block_count: number;
  total_transactions: number;
  validator_count: number;
  mempool_size: number;
  peer_count: number;
  tps_current: number;
  tps_average_1h: number;
  tps_peak_24h: number;
  finality_ms: number;
  uptime_pct: number;
  hash_rate: number;
  blocks_produced: number;
  consensus_uptime_pct: number;
  cpu_usage_pct: number;
  memory_usage_mb: number;
  db_size: number;
  disk_usage_pct: number;
  total_bridge_locked: string;
  total_bridge_claimed: string;
  transactions_submitted: number;
  transactions_accepted: number;
  transactions_rejected: number;
  total_supply: string;
  green_score: number;
  epoch: number;
  num_chains: number;
  current_difficulty: number;
  avg_utxo_size_bytes: number;
  active_utxos: number;
}

export interface MempoolEntry {
  id: string;
  sender: string;
  receiver: string;
  amount: string;
  fee: string;
  gas_limit: string;
  timestamp: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  limit: number;
  offset: number;
}

// ── API Client ─────────────────────────────────────────────────────────────

class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchJson<T>(endpoint: string, params?: URLSearchParams): Promise<T> {
  const base = NODE_API_BASE.replace(/\/$/, '');
  const url = base.startsWith('http://') || base.startsWith('https://')
    ? new URL(endpoint, `${base}/`)
    : new URL(`${base}${endpoint}`, window.location.origin);
  if (params) url.search = params.toString();

  const res = await fetch(url.toString());
  if (!res.ok) {
    throw new ApiError(res.status, `API ${endpoint} returned ${res.status}`);
  }
  return res.json() as Promise<T>;
}

// ── Endpoints ──────────────────────────────────────────────────────────────

/** Fetch live network stats from /stats */
export async function fetchNetworkStats(): Promise<NetworkStats> {
  return fetchJson<NetworkStats>('/stats');
}

/** Fetch paginated blocks from /blocks?full=true */
export async function fetchBlocks(
  limit = 20,
  offset = 0,
): Promise<PaginatedResponse<BlockSummary>> {
  const params = new URLSearchParams();
  params.set('limit', String(limit));
  params.set('offset', String(offset));
  params.set('full', 'true');

  const raw = await fetchJson<{
    blocks: BlockSummary[];
    total: number;
    limit: number;
    offset: number;
  }>('/blocks', params);

  return { data: raw.blocks, total: raw.total, limit: raw.limit, offset: raw.offset };
}

/** Fetch paginated transactions from /transactions */
export async function fetchTransactions(
  limit = 20,
  offset = 0,
): Promise<PaginatedResponse<TransactionSummary>> {
  const params = new URLSearchParams();
  params.set('limit', String(limit));
  params.set('offset', String(offset));

  const raw = await fetchJson<{
    transactions: TransactionSummary[];
    total: number;
    limit: number;
    offset: number;
  }>('/transactions', params);

  return { data: raw.transactions, total: raw.total, limit: raw.limit, offset: raw.offset };
}

/** Fetch full mempool contents */
export async function fetchMempool(): Promise<MempoolEntry[]> {
  const raw = await fetchJson<Record<string, MempoolEntry>>('/mempool');
  return Object.values(raw);
}

/** Fetch a single block by ID */
export async function fetchBlock(id: string): Promise<BlockSummary> {
  const raw = await fetchJson<any>(`/block/${encodeURIComponent(id)}`);
  return {
    id: raw.id,
    chain_id: raw.chain_id,
    height: raw.height,
    timestamp: raw.timestamp,
    validator: raw.validator,
    miner: raw.miner,
    difficulty: raw.difficulty,
    nonce: raw.nonce,
    tx_count: raw.transactions?.length ?? 0,
    reward: String(raw.reward),
    merkle_root: raw.merkle_root,
    parents: raw.parents ?? [],
    epoch: raw.epoch,
  };
}

/** Fetch a single transaction by hash */
export async function fetchTransaction(hash: string): Promise<TransactionSummary> {
  const raw = await fetchJson<any>(`/tx/${encodeURIComponent(hash)}`);
  return {
    id: raw.id,
    sender: raw.sender,
    receiver: raw.receiver,
    amount: raw.amount,
    fee: raw.fee,
    timestamp: raw.timestamp,
    block_id: raw.block_id,
    block_height: raw.block_height,
    transaction_kind: raw.transaction_kind,
  };
}

/** Check node health */
export async function fetchHealth(): Promise<{ status: string }> {
  return fetchJson('/health');
}

// ── Formatting helpers ─────────────────────────────────────────────────────

const QAN_SCALE = 1_000_000_000;

export function formatQan(baseUnits: string | number): string {
  const raw = typeof baseUnits === 'string' ? BigInt(baseUnits) : BigInt(baseUnits);
  const whole = raw / BigInt(QAN_SCALE);
  const frac = raw % BigInt(QAN_SCALE);
  const fracStr = String(frac).padStart(9, '0').replace(/0+$/, '');
  if (fracStr.length === 0) return `${whole} QAN`;
  return `${whole}.${fracStr.slice(0, 6)} QAN`;
}

export function shortenHash(hash: string, chars = 8): string {
  if (hash.length <= chars * 2 + 3) return hash;
  return `${hash.slice(0, chars)}...${hash.slice(-chars)}`;
}

export function timeAgo(ts: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - ts;
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}
