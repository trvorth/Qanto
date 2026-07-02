/* ═══════════════════════════════════════════════════════════════════════════
   QANTOscan Explorer — App & Dashboard Component
   ────────────────────────────────────────────────────────────────────────────
   Layout: Header → Stats Row → (Blocks Feed | Transactions Feed)
   ═══════════════════════════════════════════════════════════════════════════ */

import { useLiveData } from './hooks/useLiveData';
import { formatQan, shortenHash, timeAgo } from './api';

// ── Sub-components ─────────────────────────────────────────────────────────

function Header({ isConnected }: { isConnected: boolean }) {
  return (
    <header style={styles.header}>
      <div style={styles.headerLeft}>
        <span style={styles.logoIcon}>⬡</span>
        <h1 style={styles.logoText}>
          QANTO<span style={styles.logoAccent}>scan</span>
        </h1>
        <span style={styles.version}>Explorer v1</span>
      </div>
      <div style={styles.headerRight}>
        <div style={{
          ...styles.statusDot,
          background: isConnected ? 'var(--qanto-success)' : 'var(--qanto-danger)',
          boxShadow: isConnected
            ? '0 0 8px var(--qanto-success)'
            : '0 0 8px var(--qanto-danger)',
        }} />
        <span style={styles.statusLabel}>
          {isConnected ? 'LIVE — Node Connected' : 'OFFLINE — Awaiting Node'}
        </span>
      </div>
    </header>
  );
}

function StatCard({ label, value, unit, accent }: {
  label: string; value: string; unit?: string; accent?: boolean;
}) {
  return (
    <div style={{
      ...styles.statCard,
      ...(accent ? styles.statCardAccent : {}),
    }}>
      <span style={styles.statLabel}>{label}</span>
      <div style={styles.statValueRow}>
        <span style={styles.statValue}>{value}</span>
        {unit && <span style={styles.statUnit}>{unit}</span>}
      </div>
    </div>
  );
}

function BlockCard({ block }: { block: import('./api').BlockSummary }) {
  return (
    <div style={styles.feedCard}>
      <div style={styles.feedCardTop}>
        <span style={styles.feedCardTitle}>
          Block <span style={styles.hashMono}>#{block.height}</span>
        </span>
        <span style={styles.feedCardTime}>{timeAgo(block.timestamp)}</span>
      </div>
      <div style={styles.feedCardId}>{shortenHash(block.id, 10)}</div>
      <div style={styles.feedCardMeta}>
        <span>{block.tx_count} txns</span>
        <span style={styles.dot}>·</span>
        <span>{shortenHash(block.validator, 6)}</span>
        <span style={styles.dot}>·</span>
        <span style={styles.rewardText}>{formatQan(block.reward)}</span>
      </div>
    </div>
  );
}

function TxCard({ tx }: { tx: import('./api').TransactionSummary }) {
  const kindColor = tx.transaction_kind === 'TRANSFER' ? 'var(--qanto-accent-glow)' : 'var(--qanto-warning)';
  return (
    <div style={styles.feedCard}>
      <div style={styles.feedCardTop}>
        <span style={styles.feedCardTitle}>
          TX <span style={styles.hashMono}>{shortenHash(tx.id, 8)}</span>
        </span>
        <span style={{ ...styles.kindBadge, color: kindColor, borderColor: kindColor }}>
          {tx.transaction_kind}
        </span>
      </div>
      <div style={styles.txRoute}>
        <span style={styles.hashMono}>{shortenHash(tx.sender, 6)}</span>
        <span style={styles.txArrow}>→</span>
        <span style={styles.hashMono}>{shortenHash(tx.receiver, 6)}</span>
      </div>
      <div style={styles.feedCardMeta}>
        <span>{formatQan(tx.amount)}</span>
        <span style={styles.dot}>·</span>
        <span>Block #{tx.block_height}</span>
      </div>
    </div>
  );
}

function FeedColumn({ title, count, children, accentStrip }: {
  title: string; count: number; children: React.ReactNode; accentStrip?: boolean;
}) {
  return (
    <div style={styles.feedColumn}>
      <div style={styles.feedColumnHeader}>
        <div style={styles.feedColumnTitleRow}>
          {accentStrip && <div style={styles.accentStrip} />}
          <h2 style={styles.feedColumnTitle}>{title}</h2>
        </div>
        <span style={styles.feedColumnCount}>{count}</span>
      </div>
      <div style={styles.feedScroll}>
        {children}
      </div>
    </div>
  );
}

function LoadingState() {
  return (
    <div style={styles.loadingContainer}>
      <div style={styles.loadingHex}>⬡</div>
      <p style={styles.loadingText}>Connecting to Qanto Node...</p>
      <p style={styles.loadingHint}>Awaiting node at {import.meta.env.VITE_QANTO_NODE_URL ?? 'http://127.0.0.1:8081'}</p>
    </div>
  );
}

// ── Main Dashboard ─────────────────────────────────────────────────────────

export default function App() {
  const { stats, blocks, transactions, isConnected, lastError } = useLiveData();

  // Offline / initial loading state
  if (!stats) {
    return (
      <div style={styles.root}>
        <Header isConnected={false} />
        <LoadingState />
        {lastError && <p style={styles.errorBanner}>Error: {lastError}</p>}
      </div>
    );
  }

  return (
    <div style={styles.root}>
      <Header isConnected={isConnected} />

      {/* ── Stats Row ──────────────────────────────────────────────────── */}
      <section style={styles.statsRow}>
        <StatCard
          label="Total Supply"
          value={formatQan(stats.total_supply)}
          accent
        />
        <StatCard
          label="Peers"
          value={String(stats.peer_count)}
        />
        <StatCard
          label="Finality"
          value={String(stats.finality_ms)}
          unit="ms"
        />
        <StatCard
          label="TPS (current)"
          value={stats.tps_current.toFixed(1)}
          accent
        />
        <StatCard
          label="Blocks"
          value={stats.block_count.toLocaleString()}
        />
        <StatCard
          label="Transactions"
          value={stats.total_transactions.toLocaleString()}
        />
        <StatCard
          label="Mempool"
          value={String(stats.mempool_size)}
        />
        <StatCard
          label="Validators"
          value={String(stats.validator_count)}
        />
      </section>

      {/* ── Two-column Feed ────────────────────────────────────────────── */}
      <section style={styles.feedRow}>
        <FeedColumn title="Latest Blocks" count={stats.block_count} accentStrip>
          {blocks.map((b) => (
            <BlockCard key={b.id} block={b} />
          ))}
        </FeedColumn>
        <FeedColumn title="Latest Transactions" count={stats.total_transactions}>
          {transactions.map((tx) => (
            <TxCard key={tx.id} tx={tx} />
          ))}
        </FeedColumn>
      </section>

      {/* ── Footer ─────────────────────────────────────────────────────── */}
      <footer style={styles.footer}>
        <span>QANTO Layer-0 Protocol</span>
        <span style={styles.dot}>·</span>
        <span>Quantum-Resistant</span>
        <span style={styles.dot}>·</span>
        <span>Hybrid PoW + DPoS</span>
        <span style={styles.dot}>·</span>
        <span>Epoch {stats.epoch}</span>
        <span style={styles.dot}>·</span>
        <span>{stats.num_chains} Chains</span>
      </footer>
    </div>
  );
}

// ── Inline Styles (zero-dependency, zero-runtime CSS) ──────────────────────

const styles: Record<string, React.CSSProperties> = {
  root: {
    minHeight: '100vh',
    display: 'flex',
    flexDirection: 'column',
    padding: '0 24px 24px',
    maxWidth: 1440,
    margin: '0 auto',
  },

  // Header
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '18px 0',
    borderBottom: '1px solid var(--qanto-border)',
    marginBottom: 24,
  },
  headerLeft: {
    display: 'flex',
    alignItems: 'center',
    gap: 12,
  },
  logoIcon: {
    fontSize: 28,
    color: 'var(--qanto-accent)',
    filter: 'drop-shadow(0 0 8px rgba(108, 92, 231, 0.5))',
  },
  logoText: {
    fontSize: 22,
    fontWeight: 700,
    letterSpacing: '-0.02em',
    fontFamily: 'var(--qanto-font-mono)',
  },
  logoAccent: {
    color: 'var(--qanto-accent-glow)',
    fontWeight: 400,
  },
  version: {
    fontSize: 11,
    color: 'var(--qanto-text-muted)',
    background: 'var(--qanto-bg-card)',
    padding: '2px 8px',
    borderRadius: 'var(--qanto-radius-sm)',
    fontFamily: 'var(--qanto-font-mono)',
  },
  headerRight: {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: '50%',
    animation: 'pulse-glow 2s ease-in-out infinite',
  },
  statusLabel: {
    fontSize: 12,
    color: 'var(--qanto-text-secondary)',
    fontFamily: 'var(--qanto-font-mono)',
    letterSpacing: '0.04em',
  },

  // Stats
  statsRow: {
    display: 'grid',
    gridTemplateColumns: 'repeat(4, 1fr)',
    gap: 12,
    marginBottom: 24,
  },
  statCard: {
    background: 'var(--qanto-bg-card)',
    border: '1px solid var(--qanto-border)',
    borderRadius: 'var(--qanto-radius-md)',
    padding: '14px 16px',
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
    transition: 'border-color 0.2s, box-shadow 0.2s',
  },
  statCardAccent: {
    borderColor: 'var(--qanto-border-hover)',
    boxShadow: 'var(--qanto-glow-sm)',
  },
  statLabel: {
    fontSize: 11,
    color: 'var(--qanto-text-muted)',
    textTransform: 'uppercase',
    letterSpacing: '0.06em',
    fontFamily: 'var(--qanto-font-mono)',
  },
  statValueRow: {
    display: 'flex',
    alignItems: 'baseline',
    gap: 4,
  },
  statValue: {
    fontSize: 20,
    fontWeight: 700,
    color: 'var(--qanto-text-primary)',
    fontFamily: 'var(--qanto-font-mono)',
  },
  statUnit: {
    fontSize: 12,
    color: 'var(--qanto-text-muted)',
    fontFamily: 'var(--qanto-font-mono)',
  },

  // Feed columns
  feedRow: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    gap: 16,
    flex: 1,
    minHeight: 0,
  },
  feedColumn: {
    display: 'flex',
    flexDirection: 'column',
    background: 'var(--qanto-bg-surface)',
    border: '1px solid var(--qanto-border)',
    borderRadius: 'var(--qanto-radius-lg)',
    overflow: 'hidden',
  },
  feedColumnHeader: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '14px 18px',
    borderBottom: '1px solid var(--qanto-divider)',
    flexShrink: 0,
  },
  feedColumnTitleRow: {
    display: 'flex',
    alignItems: 'center',
    gap: 10,
  },
  accentStrip: {
    width: 3,
    height: 18,
    borderRadius: 2,
    background: 'var(--qanto-accent)',
    boxShadow: 'var(--qanto-glow-accent)',
  },
  feedColumnTitle: {
    fontSize: 15,
    fontWeight: 600,
    color: 'var(--qanto-text-primary)',
  },
  feedColumnCount: {
    fontSize: 11,
    color: 'var(--qanto-text-muted)',
    fontFamily: 'var(--qanto-font-mono)',
    background: 'var(--qanto-bg-card)',
    padding: '3px 10px',
    borderRadius: 'var(--qanto-radius-sm)',
  },
  feedScroll: {
    flex: 1,
    overflowY: 'auto',
    padding: '8px 12px',
    display: 'flex',
    flexDirection: 'column',
    gap: 6,
  },

  // Feed cards
  feedCard: {
    background: 'var(--qanto-bg-card)',
    border: '1px solid var(--qanto-divider)',
    borderRadius: 'var(--qanto-radius-sm)',
    padding: '10px 14px',
    display: 'flex',
    flexDirection: 'column',
    gap: 6,
    animation: 'slide-up 0.3s ease-out',
    transition: 'border-color 0.15s',
  },
  feedCardTop: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
  },
  feedCardTitle: {
    fontSize: 13,
    fontWeight: 600,
    color: 'var(--qanto-text-primary)',
  },
  feedCardTime: {
    fontSize: 11,
    color: 'var(--qanto-text-muted)',
    fontFamily: 'var(--qanto-font-mono)',
  },
  feedCardId: {
    fontSize: 12,
    color: 'var(--qanto-text-secondary)',
    fontFamily: 'var(--qanto-font-mono)',
  },
  feedCardMeta: {
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    fontSize: 11,
    color: 'var(--qanto-text-muted)',
    fontFamily: 'var(--qanto-font-mono)',
  },
  dot: {
    color: 'var(--qanto-text-dim)',
  },
  hashMono: {
    fontFamily: 'var(--qanto-font-mono)',
    color: 'var(--qanto-text-secondary)',
  },
  rewardText: {
    color: 'var(--qanto-success)',
  },
  kindBadge: {
    fontSize: 10,
    fontFamily: 'var(--qanto-font-mono)',
    padding: '1px 6px',
    borderRadius: 3,
    border: '1px solid',
    letterSpacing: '0.03em',
  },
  txRoute: {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    fontSize: 12,
    fontFamily: 'var(--qanto-font-mono)',
  },
  txArrow: {
    color: 'var(--qanto-accent)',
  },

  // Loading
  loadingContainer: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    justifyContent: 'center',
    alignItems: 'center',
    gap: 16,
  },
  loadingHex: {
    fontSize: 64,
    color: 'var(--qanto-accent)',
    filter: 'drop-shadow(0 0 20px rgba(108, 92, 231, 0.6))',
    animation: 'pulse-glow 2s ease-in-out infinite',
  },
  loadingText: {
    fontSize: 18,
    fontWeight: 600,
    color: 'var(--qanto-text-secondary)',
  },
  loadingHint: {
    fontSize: 13,
    color: 'var(--qanto-text-muted)',
    fontFamily: 'var(--qanto-font-mono)',
  },

  // Error
  errorBanner: {
    textAlign: 'center' as const,
    padding: '10px 16px',
    background: 'rgba(248, 113, 113, 0.1)',
    border: '1px solid rgba(248, 113, 113, 0.3)',
    borderRadius: 'var(--qanto-radius-sm)',
    color: 'var(--qanto-danger)',
    fontSize: 13,
    fontFamily: 'var(--qanto-font-mono)',
    marginTop: 12,
  },

  // Footer
  footer: {
    display: 'flex',
    justifyContent: 'center',
    alignItems: 'center',
    gap: 8,
    padding: '20px 0 0',
    marginTop: 24,
    borderTop: '1px solid var(--qanto-divider)',
    fontSize: 11,
    color: 'var(--qanto-text-dim)',
    fontFamily: 'var(--qanto-font-mono)',
  },
};