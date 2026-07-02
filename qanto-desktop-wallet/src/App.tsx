import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useDispatch, useSelector } from "react-redux";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { refreshBalance, setCurrentAddress, type AppDispatch, type RootState } from "./store";
import "./App.css";

type NavKey = "overview" | "send" | "receive" | "transactions";

type NavItem = {
  key: NavKey;
  label: string;
  icon: (active: boolean) => ReactNode;
};

type TxItem = {
  tx_hash: string;
  sender: string;
  receiver: string;
  amount_base_units: string;
  block_timestamp: number;
  confirmations: number;
  is_finalized: boolean;
  in_mempool: boolean;
  status: string;
};

function sortTransactions(items: TxItem[]): TxItem[] {
  return [...items].sort((a, b) => {
    if (a.in_mempool !== b.in_mempool) {
      return a.in_mempool ? -1 : 1;
    }
    if (a.block_timestamp !== b.block_timestamp) {
      return b.block_timestamp - a.block_timestamp;
    }
    return b.confirmations - a.confirmations;
  });
}

function mergeTransaction(items: TxItem[], incoming: TxItem): TxItem[] {
  const next = new Map(items.map((item) => [item.tx_hash, item]));
  const current = next.get(incoming.tx_hash);
  next.set(incoming.tx_hash, current ? { ...current, ...incoming } : incoming);
  return sortTransactions([...next.values()]);
}

function IconOverview(active: boolean) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      className={active ? "nav-icon nav-icon-active" : "nav-icon"}
    >
      <path
        d="M4 13.5V20h6.5v-6.5H4Z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
      <path
        d="M13.5 4H20v6.5h-6.5V4Z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
      <path
        d="M4 4h6.5v6.5H4V4Z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
      <path
        d="M13.5 13.5H20V20h-6.5v-6.5Z"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function IconSend(active: boolean) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      className={active ? "nav-icon nav-icon-active" : "nav-icon"}
    >
      <path
        d="M4 12h14"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M13 7l5 5-5 5"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function IconReceive(active: boolean) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      className={active ? "nav-icon nav-icon-active" : "nav-icon"}
    >
      <path
        d="M20 12H6"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M11 7l-5 5 5 5"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function IconTx(active: boolean) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      className={active ? "nav-icon nav-icon-active" : "nav-icon"}
    >
      <path
        d="M7 7h11"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M7 12h7"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M7 17h11"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M18 12l-2-2m2 2l-2 2"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function QantoLogo() {
  return (
    <div className="brand">
      <div className="brand-mark" aria-hidden="true">
        <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
          <defs>
            <linearGradient id="qanto-g" x1="2" y1="2" x2="24" y2="24">
              <stop stopColor="#00D1FF" />
              <stop offset="1" stopColor="#7B61FF" />
            </linearGradient>
          </defs>
          <rect x="1.5" y="1.5" width="23" height="23" rx="7" fill="#0A0D1F" />
          <rect
            x="1.5"
            y="1.5"
            width="23"
            height="23"
            rx="7"
            stroke="url(#qanto-g)"
            strokeWidth="1.5"
          />
          <path
            d="M13 6.7c3.48 0 6.3 2.82 6.3 6.3 0 3.48-2.82 6.3-6.3 6.3s-6.3-2.82-6.3-6.3c0-3.48 2.82-6.3 6.3-6.3Z"
            stroke="url(#qanto-g)"
            strokeWidth="1.7"
          />
          <path
            d="M10.2 14.7V11.3l2.8 1.6 2.8-1.6v3.4"
            stroke="url(#qanto-g)"
            strokeWidth="1.7"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
      <div className="brand-copy">
        <div className="brand-name">QANTO</div>
        <div className="brand-sub">Desktop Wallet</div>
      </div>
    </div>
  );
}

type OverviewProps = {
  recentTransactions: TxItem[];
};

function Overview({ recentTransactions }: OverviewProps) {
  const dispatch = useDispatch<AppDispatch>();
  const balance = useSelector((state: RootState) => state.wallet.balance);
  const currentAddress = useSelector((state: RootState) => state.wallet.currentAddress);
  const balanceStatus = useSelector((state: RootState) => state.wallet.balanceStatus);
  const balanceError = useSelector((state: RootState) => state.wallet.balanceError);

  useEffect(() => {
    if (!currentAddress) return;
    void dispatch(refreshBalance({ address: currentAddress }));
  }, [currentAddress, dispatch]);

  return (
    <div className="view">
      <div className="view-header">
        <div>
          <div className="view-title">Overview</div>
          <div className="view-caption">
            {currentAddress ? `Address: ${currentAddress}` : "No address selected. Generate one in Receive."}
          </div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <button
            type="button"
            className="nav-item"
            disabled={!currentAddress || balanceStatus === "loading"}
            onClick={() => {
              if (!currentAddress) return;
              void dispatch(refreshBalance({ address: currentAddress }));
            }}
          >
            {balanceStatus === "loading" ? "Refreshing..." : "Refresh"}
          </button>
          <div className="pill">
            {balanceError ? "RPC Error" : "Mainnet: Coming Soon"}
          </div>
        </div>
      </div>

      {balanceError ? (
        <section className="panel">
          <div className="panel-label">Balance Refresh Failed</div>
          <div className="panel-hint">{balanceError}</div>
        </section>
      ) : null}

      <div className="grid-3">
        <section className="panel">
          <div className="panel-label">Available</div>
          <div className="panel-value">{balance.available}</div>
          <div className="panel-hint">Spendable balance</div>
        </section>
        <section className="panel">
          <div className="panel-label">Pending</div>
          <div className="panel-value">{balance.pending}</div>
          <div className="panel-hint">Awaiting confirmations</div>
        </section>
        <section className="panel">
          <div className="panel-label">Total</div>
          <div className="panel-value">{balance.total}</div>
          <div className="panel-hint">Available + Pending</div>
        </section>
      </div>

      <section className="panel panel-table">
        <div className="panel-head">
          <div className="panel-title">Recent Transactions</div>
          <div className="panel-meta">
            {recentTransactions.length ? `${recentTransactions.length} live entries` : "No live transactions yet"}
          </div>
        </div>
        <div className="tx-table" role="table" aria-label="Recent transactions">
          {recentTransactions.length === 0 ? (
            <div className="tx-row" role="row">
              <div className="tx-cell tx-primary" role="cell">
                <div className="tx-label">No transactions yet</div>
                <div className="tx-id">Live mempool and on-chain updates will appear here once activity starts.</div>
              </div>
              <div className="tx-cell tx-amount" role="cell">
                -
              </div>
              <div className="tx-cell tx-status" role="cell">
                <span className="status">Empty</span>
              </div>
            </div>
          ) : recentTransactions.map((tx) => (
            <div className="tx-row" role="row" key={tx.tx_hash}>
              <div className="tx-cell tx-primary" role="cell">
                <div className="tx-label">{tx.sender === currentAddress ? "Sent" : "Received"}</div>
                <div className="tx-id">{new Date(tx.block_timestamp * 1000).toLocaleString()} • {tx.tx_hash}</div>
              </div>
              <div className="tx-cell tx-amount" role="cell">
                {formatBaseUnitsToQNTO(tx.amount_base_units)}
              </div>
              <div className="tx-cell tx-status" role="cell">
                <span className="status">
                  {tx.in_mempool ? "Pending" : tx.is_finalized ? "Finalized" : "Confirmed"}
                </span>
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function ReceiveView() {
  const dispatch = useDispatch<AppDispatch>();
  const currentAddress = useSelector((state: RootState) => state.wallet.currentAddress);
  const [hasWallet, setHasWallet] = useState<boolean | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const [createPassword, setCreatePassword] = useState("");
  const [createConfirm, setCreateConfirm] = useState("");
  const [unlockPassword, setUnlockPassword] = useState("");
  const [mnemonicPassword, setMnemonicPassword] = useState("");
  const [mnemonic, setMnemonic] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("has_wallet")
      .then((v) => {
        if (cancelled) return;
        setHasWallet(v);
      })
      .catch(() => {
        if (cancelled) return;
        setHasWallet(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const createWallet = async () => {
    setIsBusy(true);
    setActionError(null);
    setCopyState("idle");
    setMnemonic(null);
    try {
      if (createPassword !== createConfirm) {
        throw new Error("Passwords do not match");
      }
      const address = await invoke<string>("create_wallet", { password: createPassword });
      dispatch(setCurrentAddress(address));
      void dispatch(refreshBalance({ address }));
      setHasWallet(true);
      setCreatePassword("");
      setCreateConfirm("");
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const unlockWallet = async () => {
    setIsBusy(true);
    setActionError(null);
    try {
      const address = await invoke<string>("unlock_wallet", { password: unlockPassword });
      dispatch(setCurrentAddress(address));
      void dispatch(refreshBalance({ address }));
      setUnlockPassword("");
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const exportMnemonic = async () => {
    setIsBusy(true);
    setActionError(null);
    setMnemonic(null);
    try {
      const words = await invoke<string>("export_mnemonic", { password: mnemonicPassword });
      setMnemonic(words);
      setMnemonicPassword("");
      window.setTimeout(() => setMnemonic(null), 30_000);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBusy(false);
    }
  };

  const copy = async () => {
    if (!currentAddress) return;
    try {
      await navigator.clipboard.writeText(currentAddress);
      setCopyState("copied");
      window.setTimeout(() => setCopyState("idle"), 1600);
    } catch {
      setCopyState("failed");
    }
  };

  return (
    <div className="view">
      <div className="view-header">
        <div>
          <div className="view-title">Receive</div>
          <div className="view-caption">Generate a new QANTO address via secure IPC and copy it instantly.</div>
        </div>
        <div className="pill">PQC: Enabled</div>
      </div>

      <section className="panel">
        <div className="panel-label">Wallet Address</div>
        <div className="panel-value">{currentAddress ?? "—"}</div>
        <div className="panel-hint">
          {actionError ? actionError : "Address format: 64 hex chars. Balance queries use WalletService (string u128-safe)."}
        </div>
      </section>

      <div className="grid-3">
        <section className="panel">
          <div className="panel-label">Keystore</div>
          <div className="panel-hint">
            {hasWallet === null ? "Checking keystore..." : hasWallet ? "Keystore detected" : "No wallet found"}
          </div>
          {hasWallet ? (
            <div className="stack" style={{ marginTop: 10 }}>
              <label className="field">
                <div className="field-label">Payment Password</div>
                <input
                  className="input"
                  type="password"
                  value={unlockPassword}
                  onChange={(e) => setUnlockPassword(e.currentTarget.value)}
                  placeholder="Enter password to unlock"
                  autoComplete="current-password"
                />
              </label>
              <button type="button" className="nav-item nav-item-active" onClick={unlockWallet} disabled={isBusy || !unlockPassword}>
                Unlock Wallet (15 min)
              </button>
            </div>
          ) : (
            <div className="stack" style={{ marginTop: 10 }}>
              <label className="field">
                <div className="field-label">Create Password</div>
                <input
                  className="input"
                  type="password"
                  value={createPassword}
                  onChange={(e) => setCreatePassword(e.currentTarget.value)}
                  placeholder="8+ chars, letters + digits + special"
                  autoComplete="new-password"
                />
              </label>
              <label className="field">
                <div className="field-label">Confirm Password</div>
                <input
                  className="input"
                  type="password"
                  value={createConfirm}
                  onChange={(e) => setCreateConfirm(e.currentTarget.value)}
                  placeholder="Re-enter password"
                  autoComplete="new-password"
                />
              </label>
              <button
                type="button"
                className="nav-item nav-item-active"
                onClick={createWallet}
                disabled={isBusy || !createPassword || !createConfirm}
              >
                Create Wallet
              </button>
            </div>
          )}
        </section>
        <section className="panel">
          <div className="panel-label">Copy</div>
          <div className="panel-hint">One-click copy for fast deposits.</div>
          <div style={{ marginTop: 10, display: "flex", gap: 10 }}>
            <button type="button" className="nav-item" onClick={copy} disabled={!currentAddress}>
              {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy Failed" : "Copy Address"}
            </button>
          </div>
        </section>
        <section className="panel">
          <div className="panel-label">Mnemonic Export</div>
          <div className="panel-hint">Requires re-auth. Do not store or copy automatically.</div>
          <div className="stack" style={{ marginTop: 10 }}>
            <label className="field">
              <div className="field-label">Re-enter Password</div>
              <input
                className="input"
                type="password"
                value={mnemonicPassword}
                onChange={(e) => setMnemonicPassword(e.currentTarget.value)}
                placeholder="Confirm password"
                autoComplete="current-password"
              />
            </label>
            <button
              type="button"
              className="nav-item"
              onClick={exportMnemonic}
              disabled={isBusy || !mnemonicPassword}
            >
              Export Mnemonic (30s)
            </button>
            {mnemonic ? <div className="mnemonic">{mnemonic}</div> : null}
          </div>
        </section>
      </div>
    </div>
  );
}

function parseQntoToBaseUnits(amount: string): bigint | null {
  const s = amount.trim();
  if (!s) return null;
  if (s.startsWith("-")) return null;
  const parts = s.split(".");
  if (parts.length > 2) return null;
  const intPart = parts[0];
  const fracPart = parts.length === 2 ? parts[1] : "";
  if (!/^\d+$/.test(intPart)) return null;
  if (fracPart && !/^\d+$/.test(fracPart)) return null;
  if (fracPart.length > 9) return null;
  const fracPadded = (fracPart + "0".repeat(9)).slice(0, 9);
  const combined = (intPart + fracPadded).replace(/^0+/, "") || "0";
  try {
    return BigInt(combined);
  } catch {
    return null;
  }
}

function formatBaseUnitsToQNTO(baseUnits: string): string {
  const smallestUnits = 1_000_000_000n;
  const raw = BigInt(baseUnits);
  const intPart = raw / smallestUnits;
  const frac9 = raw % smallestUnits;
  return `${intPart.toString()}.${frac9.toString().padStart(9, "0")} QNTO`;
}

function SendView() {
  const currentAddress = useSelector((state: RootState) => state.wallet.currentAddress);
  const availableBaseUnits = useSelector((state: RootState) => state.wallet.balanceBaseUnits.available);
  const dispatch = useDispatch<AppDispatch>();
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [password, setPassword] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);

  const recipientError = useMemo(() => {
    const v = recipient.trim();
    if (!v) return "Recipient address is required";
    if (v.length !== 64) return "Recipient address must be 64 hex characters";
    if (!/^[0-9a-fA-F]+$/.test(v)) return "Recipient address must be hex";
    return null;
  }, [recipient]);

  const amountBase = useMemo(() => parseQntoToBaseUnits(amount), [amount]);

  const amountError = useMemo(() => {
    if (!amount.trim()) return "Amount is required";
    if (amountBase === null) return "Invalid amount (up to 9 decimals)";
    if (amountBase <= 0n) return "Amount must be greater than zero";
    try {
      const available = BigInt(availableBaseUnits);
      if (amountBase > available) return "Insufficient available balance";
    } catch {
      return "Balance unavailable";
    }
    return null;
  }, [amount, amountBase, availableBaseUnits]);

  const passwordError = useMemo(() => {
    if (!password) return "Password is required";
    return null;
  }, [password]);

  const canSend = Boolean(currentAddress && !recipientError && !amountError && !passwordError && !isSending);

  const send = async () => {
    if (!canSend) return;
    setIsSending(true);
    setSendError(null);
    setTxHash(null);
    try {
      const resp = await invoke<{ tx_hash: string }>("send_transaction", {
        recipient: recipient.trim(),
        amount_string: amount.trim(),
        password,
      });
      setTxHash(resp.tx_hash);
      setPassword("");
      if (currentAddress) {
        void dispatch(refreshBalance({ address: currentAddress }));
      }
    } catch (err) {
      setSendError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSending(false);
    }
  };

  return (
    <div className="view">
      <div className="view-header">
        <div>
          <div className="view-title">Send</div>
          <div className="view-caption">
            {currentAddress ? `From: ${currentAddress}` : "No address selected. Create + unlock in Receive."}
          </div>
        </div>
        <div className="pill">Dual-Sign: Ed25519 + Dilithium3</div>
      </div>

      <section className="panel">
        <div className="form">
          <label className="field">
            <div className="field-label">Recipient Address</div>
            <input className="input" value={recipient} onChange={(e) => setRecipient(e.currentTarget.value)} placeholder="64-hex address" />
            {recipientError ? <div className="field-error">{recipientError}</div> : null}
          </label>
          <label className="field">
            <div className="field-label">Amount (QNTO)</div>
            <input className="input" value={amount} onChange={(e) => setAmount(e.currentTarget.value)} placeholder="0.000000000" />
            {amountError ? <div className="field-error">{amountError}</div> : null}
          </label>
          <label className="field">
            <div className="field-label">Payment Password</div>
            <input
              className="input"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              placeholder="Required for signing"
              autoComplete="current-password"
            />
            {passwordError ? <div className="field-error">{passwordError}</div> : null}
          </label>

          <div className="actions">
            <button type="button" className={canSend ? "nav-item nav-item-active" : "nav-item"} onClick={send} disabled={!canSend}>
              {isSending ? "Sending..." : "Send QNTO"}
            </button>
            <div className="hint">Available: {formatBaseUnitsToQNTO(availableBaseUnits)}</div>
          </div>

          {sendError ? (
            <div className="alert alert-error">
              <div className="alert-title">Send Failed</div>
              <div className="alert-body">{sendError}</div>
            </div>
          ) : null}

          {txHash ? (
            <div className="alert alert-ok">
              <div className="alert-title">Broadcasted</div>
              <div className="alert-body">TxHash: {txHash}</div>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

type TransactionsViewProps = {
  currentAddress: string | null;
  items: TxItem[];
  hasMore: boolean;
  isLoading: boolean;
  error: string | null;
  onRefresh: () => void;
  onLoadMore: () => void;
};

function TransactionsView({
  currentAddress,
  items,
  hasMore,
  isLoading,
  error,
  onRefresh,
  onLoadMore,
}: TransactionsViewProps) {
  const getBadge = (tx: TxItem) => {
    if (tx.in_mempool) {
      return { label: "Pending", className: "status status-pending" };
    }
    if (tx.is_finalized) {
      return { label: `Finalized · ${tx.confirmations} conf`, className: "status status-finalized" };
    }
    if (tx.confirmations > 0) {
      return { label: `Confirmed · ${tx.confirmations} conf`, className: "status status-confirmed" };
    }
    return { label: "Pending", className: "status status-pending" };
  };

  return (
    <div className="view">
      <div className="view-header">
        <div>
          <div className="view-title">Transactions</div>
          <div className="view-caption">
            {currentAddress ? `Address: ${currentAddress}` : "No address selected. Create + unlock in Receive."}
          </div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <button type="button" className="nav-item" onClick={onRefresh} disabled={!currentAddress || isLoading}>
            {isLoading ? "Loading..." : "Refresh"}
          </button>
          <div className="pill">Live gRPC Stream + Mempool</div>
        </div>
      </div>

      {error ? (
        <section className="panel">
          <div className="panel-label">Load Failed</div>
          <div className="panel-hint">{error}</div>
        </section>
      ) : null}

      <section className="panel panel-table">
        <div className="panel-head">
          <div className="panel-title">History</div>
          <div className="panel-meta">{items.length ? `${items.length} loaded` : "No transactions"}</div>
        </div>
        <div className="tx-table" role="table" aria-label="Transaction history">
          {items.length === 0 && !isLoading ? (
            <div className="tx-row" role="row">
              <div className="tx-cell tx-primary" role="cell">
                <div className="tx-label">No transactions yet</div>
                <div className="tx-id">No on-chain or mempool transactions found for this address.</div>
              </div>
              <div className="tx-cell tx-amount" role="cell">
                —
              </div>
              <div className="tx-cell tx-status" role="cell">
                <span className="status">Empty</span>
              </div>
            </div>
          ) : null}
          {items.map((tx) => {
            const badge = getBadge(tx);
            const localTime = new Date(tx.block_timestamp * 1000).toLocaleString();
            return (
              <div className="tx-row" role="row" key={tx.tx_hash}>
                <div className="tx-cell tx-primary" role="cell">
                  <div className="tx-label">{tx.sender === currentAddress ? "Sent" : "Received"}</div>
                  <div className="tx-id">
                    {localTime} • {tx.tx_hash}
                  </div>
                  <div className="tx-meta">
                    {tx.in_mempool
                      ? `Mempool Seen: ${localTime} · Awaiting block inclusion`
                      : `Block Time: ${localTime} · ${tx.is_finalized ? "DAG Finality Reached" : `${tx.confirmations} confirmation(s)`}`}
                  </div>
                </div>
                <div className="tx-cell tx-amount" role="cell">
                  {formatBaseUnitsToQNTO(tx.amount_base_units)}
                </div>
                <div className="tx-cell tx-status" role="cell">
                  <span className={badge.className}>{badge.label}</span>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {hasMore ? (
        <div style={{ marginTop: 12, display: "flex" }}>
          <button type="button" className="nav-item" onClick={onLoadMore} disabled={isLoading}>
            Load More
          </button>
        </div>
      ) : null}
    </div>
  );
}

function App() {
  const dispatch = useDispatch<AppDispatch>();
  const currentAddress = useSelector((state: RootState) => state.wallet.currentAddress);
  const [active, setActive] = useState<NavKey>("overview");
  const [txItems, setTxItems] = useState<TxItem[]>([]);
  const [txPage, setTxPage] = useState(1);
  const [txHasMore, setTxHasMore] = useState(false);
  const [txLoading, setTxLoading] = useState(false);
  const [txError, setTxError] = useState<string | null>(null);

  const loadTransactions = useCallback(
    async (nextPage: number, replace: boolean) => {
      if (!currentAddress) return;
      setTxLoading(true);
      setTxError(null);
      try {
        const resp = await invoke<{ items: TxItem[]; page: number; page_size: number; has_more: boolean }>(
          "get_transaction_history",
          { address: currentAddress, page: nextPage, page_size: 20 },
        );
        setTxItems((prev) => {
          const merged = replace ? resp.items : [...prev, ...resp.items];
          return sortTransactions(merged);
        });
        setTxPage(resp.page);
        setTxHasMore(resp.has_more);
      } catch (err) {
        setTxError(err instanceof Error ? err.message : String(err));
      } finally {
        setTxLoading(false);
      }
    },
    [currentAddress],
  );

  useEffect(() => {
    setTxItems([]);
    setTxPage(1);
    setTxHasMore(false);
    setTxError(null);
    if (!currentAddress) return;
    void loadTransactions(1, true);
    void invoke("subscribe_transactions", { address: currentAddress }).catch((err) => {
      setTxError(err instanceof Error ? err.message : String(err));
    });
  }, [currentAddress, loadTransactions]);

  useEffect(() => {
    let mounted = true;
    const unlistenPromise = listen<TxItem>("tx-update", (event) => {
      if (!mounted || !currentAddress) return;
      const nextTx = event.payload;
      const isRelevant = nextTx.sender === currentAddress || nextTx.receiver === currentAddress;
      if (!isRelevant) return;
      setTxItems((prev) => mergeTransaction(prev, nextTx));
      setTxError(null);
      void dispatch(refreshBalance({ address: currentAddress }));
    });

    return () => {
      mounted = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [currentAddress, dispatch]);

  const navItems: NavItem[] = useMemo(
    () => [
      { key: "overview", label: "Overview", icon: IconOverview },
      { key: "send", label: "Send", icon: IconSend },
      { key: "receive", label: "Receive", icon: IconReceive },
      { key: "transactions", label: "Transactions", icon: IconTx },
    ],
    [],
  );

  const overviewTransactions = useMemo(() => txItems.slice(0, 3), [txItems]);

  const content = useMemo(() => {
    switch (active) {
      case "overview":
        return <Overview recentTransactions={overviewTransactions} />;
      case "send":
        return <SendView />;
      case "receive":
        return <ReceiveView />;
      case "transactions":
        return (
          <TransactionsView
            currentAddress={currentAddress}
            items={txItems}
            hasMore={txHasMore}
            isLoading={txLoading}
            error={txError}
            onRefresh={() => void loadTransactions(1, true)}
            onLoadMore={() => void loadTransactions(txPage + 1, false)}
          />
        );
      default:
        return null;
    }
  }, [active, currentAddress, loadTransactions, overviewTransactions, txError, txHasMore, txItems, txLoading, txPage]);

  return (
    <div className="app-shell">
      <div className="app-noise" aria-hidden="true" />
      <aside className="sidebar">
        <QantoLogo />
        <nav className="nav" aria-label="Primary navigation">
          {navItems.map((item) => {
            const isActive = item.key === active;
            return (
              <button
                key={item.key}
                type="button"
                className={isActive ? "nav-item nav-item-active" : "nav-item"}
                onClick={() => setActive(item.key)}
              >
                {item.icon(isActive)}
                <span className="nav-label">{item.label}</span>
              </button>
            );
          })}
        </nav>
        <div className="sidebar-footer">
          <div className="footer-chip">QANTO Layer-0</div>
          <div className="footer-chip footer-chip-muted">PQC Ready</div>
        </div>
      </aside>
      <main className="content">{content}</main>
    </div>
  );
}

export default App;
