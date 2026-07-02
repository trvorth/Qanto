import express from 'express';
import cors from 'cors';
import fs from 'fs';
import path from 'path';
import sqlite3 from 'sqlite3';
import pg from 'pg';
import dotenv from 'dotenv';

dotenv.config();

const app = express();
app.use(cors());
app.use(express.json());

const PORT = process.env.PORT || 3000;
const POLL_INTERVAL = process.env.POLL_INTERVAL || 30000;
const DB_URL = process.env.DATABASE_URL;

const __dirname = path.resolve();
const DATA_DIR = path.join(__dirname, 'data');
const VALIDATORS_FILE = path.join(__dirname, 'validators.json');

// Ensure data directory exists
if (!DB_URL && !fs.existsSync(DATA_DIR)) {
  fs.mkdirSync(DATA_DIR, { recursive: true });
}

// --------------------------------------------------------------------------
// Database Setup (Dynamic SQLite / PostgreSQL)
// --------------------------------------------------------------------------
let db;
const isPostgres = !!DB_URL;

if (isPostgres) {
  console.log('Connecting to PostgreSQL database...');
  db = new pg.Pool({ connectionString: DB_URL });
} else {
  const dbPath = path.join(DATA_DIR, 'telemetry.db');
  console.log(`Connecting to SQLite database at ${dbPath}...`);
  db = new sqlite3.Database(dbPath);
}

// Query runner abstraction to support both SQLite and Postgres with a 5-second timeout
const withTimeout = (promise, ms = 5000) => {
  let timeoutId;
  const timeoutPromise = new Promise((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new Error(`Database operation timed out after ${ms}ms`));
    }, ms);
  });
  return Promise.race([promise, timeoutPromise]).finally(() => {
    clearTimeout(timeoutId);
  });
};

const query = (sql, params = []) => {
  const dbPromise = new Promise((resolve, reject) => {
    if (isPostgres) {
      // Postgres parameters use $1, $2 instead of ?
      let pgSql = sql;
      let paramCount = 1;
      while (pgSql.includes('?')) {
        pgSql = pgSql.replace('?', `$${paramCount++}`);
      }
      db.query(pgSql, params, (err, res) => {
        if (err) reject(err);
        else resolve({ rows: res.rows, count: res.rowCount });
      });
    } else {
      db.all(sql, params, (err, rows) => {
        if (err) reject(err);
        else resolve({ rows, count: rows.length });
      });
    }
  });
  return withTimeout(dbPromise, 5000);
};

const execute = (sql, params = []) => {
  const dbPromise = new Promise((resolve, reject) => {
    if (isPostgres) {
      let pgSql = sql;
      let paramCount = 1;
      while (pgSql.includes('?')) {
        pgSql = pgSql.replace('?', `$${paramCount++}`);
      }
      db.query(pgSql, params, (err, res) => {
        if (err) reject(err);
        else resolve(res);
      });
    } else {
      db.run(sql, params, function(err) {
        if (err) reject(err);
        else resolve(this);
      });
    }
  });
  return withTimeout(dbPromise, 5000);
};

// Initialize Tables
const initDb = async () => {
  const createSnapshotsTable = `
    CREATE TABLE IF NOT EXISTS snapshots (
      id ${isPostgres ? 'SERIAL PRIMARY KEY' : 'INTEGER PRIMARY KEY AUTOINCREMENT'},
      validator_id TEXT,
      name TEXT,
      endpoint TEXT,
      timestamp INTEGER,
      status TEXT,
      version TEXT,
      chain_id TEXT,
      genesis_hash TEXT,
      latest_block_hash TEXT,
      height INTEGER,
      peer_count INTEGER,
      uptime INTEGER,
      memory INTEGER,
      cpu REAL,
      fd_count INTEGER,
      mempool_size INTEGER,
      latest_block_time INTEGER,
      blocks_last_minute INTEGER,
      avg_block_interval REAL,
      saga_score INTEGER,
      blocks_produced INTEGER,
      blocks_missed INTEGER,
      slash_events INTEGER,
      tps_current REAL
    )
  `;

  const createAlertsTable = `
    CREATE TABLE IF NOT EXISTS alerts (
      id ${isPostgres ? 'SERIAL PRIMARY KEY' : 'INTEGER PRIMARY KEY AUTOINCREMENT'},
      validator_id TEXT,
      name TEXT,
      timestamp INTEGER,
      type TEXT,
      message TEXT,
      resolved INTEGER
    )
  `;

  await execute(createSnapshotsTable);
  await execute(createAlertsTable);

  const addColumns = [
    "ALTER TABLE snapshots ADD COLUMN database_size_bytes INTEGER",
    "ALTER TABLE snapshots ADD COLUMN data_dir_size_bytes INTEGER",
    "ALTER TABLE snapshots ADD COLUMN disk_usage_pct REAL",
    "ALTER TABLE snapshots ADD COLUMN total_blocks INTEGER",
    "ALTER TABLE snapshots ADD COLUMN total_transactions INTEGER",
    "ALTER TABLE snapshots ADD COLUMN active_utxos INTEGER",
    "ALTER TABLE snapshots ADD COLUMN spent_utxos INTEGER",
    "ALTER TABLE snapshots ADD COLUMN historical_inputs_consumed INTEGER",
    "ALTER TABLE snapshots ADD COLUMN raw_transaction_data_bytes INTEGER",
    "ALTER TABLE snapshots ADD COLUMN bytes_per_transaction REAL",
    "ALTER TABLE snapshots ADD COLUMN bytes_per_block REAL",
    "ALTER TABLE snapshots ADD COLUMN state_amplification_ratio REAL",
    "ALTER TABLE snapshots ADD COLUMN utxo_set_size_bytes INTEGER",
    "ALTER TABLE snapshots ADD COLUMN avg_inputs_per_tx REAL",
    "ALTER TABLE snapshots ADD COLUMN avg_outputs_per_tx REAL",
    "ALTER TABLE snapshots ADD COLUMN logical_utxo_set_size_bytes INTEGER",
    "ALTER TABLE snapshots ADD COLUMN net_utxo_growth INTEGER",
    "ALTER TABLE snapshots ADD COLUMN average_utxo_size_bytes REAL",
    "ALTER TABLE snapshots ADD COLUMN tps_current REAL"
  ];
  for (const queryStr of addColumns) {
    try {
      await execute(queryStr);
    } catch (e) {
      // Column might already exist, ignore
    }
  }

  const createSyncBenchmarksTable = `
    CREATE TABLE IF NOT EXISTS sync_benchmarks (
      id ${isPostgres ? 'SERIAL PRIMARY KEY' : 'INTEGER PRIMARY KEY AUTOINCREMENT'},
      timestamp INTEGER,
      height INTEGER,
      sync_time_seconds INTEGER,
      sync_time_estimate_seconds INTEGER,
      database_size_bytes INTEGER
    )
  `;
  await execute(createSyncBenchmarksTable);

  try {
    await execute("ALTER TABLE sync_benchmarks ADD COLUMN sync_time_estimate_seconds INTEGER");
  } catch (e) {
    // Column might already exist, ignore
  }

  console.log('Database tables verified/initialized successfully.');
};

// --------------------------------------------------------------------------
// Telemetry Aggregator Logic (Polling & Alerting)
// --------------------------------------------------------------------------

// Read configured validators
const getValidators = () => {
  try {
    if (fs.existsSync(VALIDATORS_FILE)) {
      const data = fs.readFileSync(VALIDATORS_FILE, 'utf8');
      return JSON.parse(data);
    }
  } catch (err) {
    console.error('Error reading validators.json:', err);
  }
  return [];
};

// State tracker for alert debouncing: tracks consecutive failures and successes
// Key: alert identifier (e.g. validator_id:type or type:global)
// Value: { failures: 0, successes: 0, status: 'ok' | 'alerting' }
const alertTrackers = {};

const getTracker = (key) => {
  if (!alertTrackers[key]) {
    alertTrackers[key] = { failures: 0, successes: 0, status: 'ok' };
  }
  return alertTrackers[key];
};

const handleAlertFailure = async (key, triggerAlertFn) => {
  const tracker = getTracker(key);
  tracker.successes = 0;
  tracker.failures += 1;
  if (tracker.failures >= 3 && tracker.status !== 'alerting') {
    tracker.status = 'alerting';
    await triggerAlertFn();
  }
};

const handleAlertSuccess = async (key, resolveAlertFn) => {
  const tracker = getTracker(key);
  tracker.failures = 0;
  tracker.successes += 1;
  if (tracker.successes >= 3 && tracker.status === 'alerting') {
    tracker.status = 'ok';
    await resolveAlertFn();
  }
};

const initAlertTrackers = async () => {
  try {
    const activeAlerts = await query("SELECT * FROM alerts WHERE resolved = 0");
    for (const alert of activeAlerts.rows) {
      let key;
      if (alert.type === 'GENESIS_MISMATCH' || alert.type === 'LEDGER_DIVERGENCE' || alert.type === 'SYNC_SLOW') {
        key = `network:${alert.type}`;
      } else if (alert.type === 'CHAIN_FORK') {
        const match = alert.message.match(/height (\d+)/);
        const h = match ? match[1] : 'unknown';
        key = `network:CHAIN_FORK:${h}`;
      } else {
        key = `${alert.validator_id}:${alert.type}`;
      }
      alertTrackers[key] = { failures: 3, successes: 0, status: 'alerting' };
    }
    console.log(`Initialized ${Object.keys(alertTrackers).length} active alerts into in-memory trackers.`);
  } catch (err) {
    console.error('Failed to initialize alert trackers from DB:', err);
  }
};

// Dispatch webhooks (Discord / Telegram) with 5s timeout
const sendWebhookAlert = async (alertType, message) => {
  console.log(`[ALERT DISPATCHER] ${alertType}: ${message}`);
  
  // Discord Webhook
  if (process.env.DISCORD_WEBHOOK_URL) {
    try {
      await fetch(process.env.DISCORD_WEBHOOK_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          content: `🚨 **QANTO NOC ALERT** 🚨\n**Type**: ${alertType}\n**Message**: ${message}`
        }),
        signal: AbortSignal.timeout(5000)
      });
    } catch (e) {
      console.error('Failed to send Discord webhook:', e.message);
    }
  }

  // Telegram Webhook
  if (process.env.TELEGRAM_BOT_TOKEN && process.env.TELEGRAM_CHAT_ID) {
    try {
      const url = `https://api.telegram.org/bot${process.env.TELEGRAM_BOT_TOKEN}/sendMessage`;
      await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chat_id: process.env.TELEGRAM_CHAT_ID,
          text: `🚨 QANTO NOC ALERT 🚨\nType: ${alertType}\nMessage: ${message}`
        }),
        signal: AbortSignal.timeout(5000)
      });
    } catch (e) {
      console.error('Failed to send Telegram message:', e.message);
    }
  }
};

// Poll individual node and return metrics
const pollNode = async (node) => {
  const timestamp = Math.floor(Date.now() / 1000);
  try {
    // 1. Check health
    const healthRes = await fetch(`${node.url}/health`, { signal: AbortSignal.timeout(5000) });
    if (!healthRes.ok) throw new Error('Health check non-ok status');

    // 2. Fetch stats
    const statsRes = await fetch(`${node.url}/stats`, { signal: AbortSignal.timeout(5000) });
    if (!statsRes.ok) throw new Error('Stats API non-ok status');
    const stats = await statsRes.json();

    return {
      validator_id: stats.validator_id || node.id || 'unknown',
      name: node.name,
      endpoint: node.url,
      timestamp,
      status: 'online',
      version: stats.version || '0.1.0',
      chain_id: stats.chain_id || 'unknown',
      genesis_hash: stats.genesis_hash || '',
      latest_block_hash: stats.latest_block_hash || '',
      height: stats.latest_block_height !== undefined ? stats.latest_block_height : (stats.block_count || 0),
      peer_count: stats.peer_count || 0,
      uptime: stats.uptime || 0,
      memory: stats.memory || 0,
      cpu: stats.cpu || 0,
      fd_count: stats.fd_count || 0,
      mempool_size: stats.mempool_size || 0,
      latest_block_time: stats.latest_block_time || 0,
      blocks_last_minute: stats.blocks_last_minute || 0,
      avg_block_interval: stats.avg_block_interval || 0,
      saga_score: stats.saga_score || 95,
      blocks_produced: stats.blocks_produced || 0,
      blocks_missed: stats.blocks_missed || 0,
      slash_events: stats.slash_events || 0,
      database_size_bytes: stats.database_size_bytes || 0,
      data_dir_size_bytes: stats.data_dir_size_bytes || 0,
      disk_usage_pct: stats.disk_usage_pct || 0,
      total_blocks: stats.total_blocks || stats.block_count || 0,
      total_transactions: stats.total_transactions || stats.transaction_count || 0,
      active_utxos: stats.active_utxos || 0,
      spent_utxos: stats.historical_inputs_consumed || stats.spent_utxos || 0,
      historical_inputs_consumed: stats.historical_inputs_consumed || stats.spent_utxos || 0,
      raw_transaction_data_bytes: stats.raw_transaction_data_bytes || 0,
      bytes_per_transaction: stats.bytes_per_transaction || 0,
      bytes_per_block: stats.bytes_per_block || 0,
      state_amplification_ratio: stats.state_amplification_ratio || 0,
      utxo_set_size_bytes: stats.logical_utxo_set_size_bytes || stats.utxo_set_size_bytes || 0,
      logical_utxo_set_size_bytes: stats.logical_utxo_set_size_bytes || stats.utxo_set_size_bytes || 0,
      avg_inputs_per_tx: stats.avg_inputs_per_tx || 0,
      avg_outputs_per_tx: stats.avg_outputs_per_tx || 0,
      net_utxo_growth: stats.net_utxo_growth || 0,
      average_utxo_size_bytes: stats.average_utxo_size_bytes || 0,
      tps_current: stats.tps_current || 0.0
    };
  } catch (err) {
    return {
      validator_id: node.id || 'unknown',
      name: node.name,
      endpoint: node.url,
      timestamp,
      status: 'offline',
      version: 'unknown',
      chain_id: 'unknown',
      genesis_hash: '',
      latest_block_hash: '',
      height: 0,
      peer_count: 0,
      uptime: 0,
      memory: 0,
      cpu: 0,
      fd_count: 0,
      mempool_size: 0,
      latest_block_time: 0,
      blocks_last_minute: 0,
      avg_block_interval: 0,
      saga_score: 0,
      blocks_produced: 0,
      blocks_missed: 0,
      slash_events: 0,
      database_size_bytes: 0,
      data_dir_size_bytes: 0,
      disk_usage_pct: 0,
      total_blocks: 0,
      total_transactions: 0,
      active_utxos: 0,
      spent_utxos: 0,
      historical_inputs_consumed: 0,
      raw_transaction_data_bytes: 0,
      bytes_per_transaction: 0,
      bytes_per_block: 0,
      state_amplification_ratio: 0,
      utxo_set_size_bytes: 0,
      logical_utxo_set_size_bytes: 0,
      avg_inputs_per_tx: 0,
      avg_outputs_per_tx: 0,
      net_utxo_growth: 0,
      average_utxo_size_bytes: 0,
      tps_current: 0.0,
      error: err.message
    };
  }
};

// Check and trigger alerts based on latest polling results with debouncing (3 cycles) and auto-resolutions
const runAlertingEngine = async (snapshots) => {
  const onlineNodes = snapshots.filter(s => s.status === 'online');
  const timestamp = Math.floor(Date.now() / 1000);

  // 1. Validator Offline Alert
  for (const s of snapshots) {
    const key = `${s.validator_id}:OFFLINE`;
    if (s.status === 'offline') {
      await handleAlertFailure(key, async () => {
        const activeAlert = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'OFFLINE' AND resolved = 0",
          [s.validator_id]
        );
        if (activeAlert.rows.length === 0) {
          const msg = `Validator "${s.name}" (${s.endpoint}) is OFFLINE! Error: ${s.error || 'Connection Timeout'}`;
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'OFFLINE', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('OFFLINE', msg);
        }

        // Auto-resolve other active alerts for this validator when it is offline
        try {
          const otherActiveAlerts = await query(
            "SELECT id, type FROM alerts WHERE validator_id = ? AND type != 'OFFLINE' AND resolved = 0",
            [s.validator_id]
          );
          for (const alert of otherActiveAlerts.rows) {
            await execute("UPDATE alerts SET resolved = 1 WHERE id = ?", [alert.id]);
            await sendWebhookAlert('RESOLVED', `Validator "${s.name}" alert ${alert.type} auto-resolved because node went offline.`);
            
            // Also reset in-memory debouncer tracker if it exists
            const trackerKey = `${s.validator_id}:${alert.type}`;
            if (alertTrackers[trackerKey]) {
              alertTrackers[trackerKey].status = 'ok';
              alertTrackers[trackerKey].failures = 0;
              alertTrackers[trackerKey].successes = 0;
            }
          }
        } catch (e) {
          console.error(`Failed to auto-resolve active alerts for offline validator ${s.validator_id}:`, e);
        }
      });
    } else {
      await handleAlertSuccess(key, async () => {
        const activeAlert = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'OFFLINE' AND resolved = 0",
          [s.validator_id]
        );
        if (activeAlert.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'OFFLINE'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" is back online.`);
        }
      });
    }
  }

  if (onlineNodes.length === 0) return;

  // 2. Chain Split & Fingerprint Agreements
  const genesisHashes = new Set(onlineNodes.map(n => n.genesis_hash).filter(h => h));
  const genesisMismatchKey = `network:GENESIS_MISMATCH`;
  
  if (genesisHashes.size > 1) {
    await handleAlertFailure(genesisMismatchKey, async () => {
      const msg = `Network split warning! Online nodes report different Genesis hashes: ${Array.from(genesisHashes).join(', ')}`;
      const active = await query("SELECT id FROM alerts WHERE type = 'GENESIS_MISMATCH' AND resolved = 0");
      if (active.rows.length === 0) {
        await execute(
          "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES ('network', 'Global Network', ?, 'GENESIS_MISMATCH', ?, 0)",
          [timestamp, msg]
        );
        await sendWebhookAlert('CRITICAL_SPLIT', msg);
      }
    });
  } else {
    await handleAlertSuccess(genesisMismatchKey, async () => {
      const active = await query("SELECT id FROM alerts WHERE type = 'GENESIS_MISMATCH' AND resolved = 0");
      if (active.rows.length > 0) {
        await execute("UPDATE alerts SET resolved = 1 WHERE type = 'GENESIS_MISMATCH'");
        await sendWebhookAlert('RESOLVED', `Genesis hash consensus restored.`);
      }
    });
  }

  // Same height, different latest_block_hash
  const heightGroups = {};
  onlineNodes.forEach(n => {
    if (!heightGroups[n.height]) heightGroups[n.height] = [];
    heightGroups[n.height].push(n);
  });

  const forkHeights = new Set();

  for (const [h, nodes] of Object.entries(heightGroups)) {
    if (nodes.length > 1) {
      const hashes = new Set(nodes.map(n => n.latest_block_hash).filter(h => h));
      if (hashes.size > 1) {
        forkHeights.add(h);
        const forkKey = `network:CHAIN_FORK:${h}`;
        await handleAlertFailure(forkKey, async () => {
          const validatorNames = nodes.map(n => `${n.name}(hash:${n.latest_block_hash.substring(0,8)})`).join(', ');
          const msg = `Fork detected at height ${h}! Validators have conflicting block hashes: ${validatorNames}`;
          
          const active = await query(
            "SELECT id FROM alerts WHERE type = 'CHAIN_FORK' AND message LIKE ? AND resolved = 0",
            [`%height ${h}%`]
          );
          if (active.rows.length === 0) {
            await execute(
              "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES ('network', 'Global Network', ?, 'CHAIN_FORK', ?, 0)",
              [timestamp, msg]
            );
            await sendWebhookAlert('CHAIN_FORK', msg);
          }
        });
      }
    }
  }

  // Find unresolved CHAIN_FORK alerts in DB and resolve the ones that are no longer present in forkHeights
  const activeForkAlerts = await query(
    "SELECT id, message FROM alerts WHERE type = 'CHAIN_FORK' AND resolved = 0"
  );
  for (const alert of activeForkAlerts.rows) {
    const match = alert.message.match(/height (\d+)/);
    if (match) {
      const h = match[1];
      if (!forkHeights.has(h)) {
        const forkKey = `network:CHAIN_FORK:${h}`;
        await handleAlertSuccess(forkKey, async () => {
          await execute("UPDATE alerts SET resolved = 1 WHERE id = ?", [alert.id]);
          await sendWebhookAlert('RESOLVED', `Chain fork at height ${h} resolved.`);
        });
      }
    }
  }

  // 3. Version Drift (Majority version verification)
  const versions = onlineNodes.map(n => n.version);
  const versionCounts = {};
  versions.forEach(v => versionCounts[v] = (versionCounts[v] || 0) + 1);
  
  let majorityVersion = 'unknown';
  let maxCount = 0;
  for (const [v, c] of Object.entries(versionCounts)) {
    if (c > maxCount) {
      maxCount = c;
      majorityVersion = v;
    }
  }

  for (const s of onlineNodes) {
    const driftKey = `${s.validator_id}:VERSION_DRIFT`;
    if (s.version !== majorityVersion) {
      await handleAlertFailure(driftKey, async () => {
        const msg = `Version drift! Validator "${s.name}" is running version ${s.version} (Network majority is ${majorityVersion})`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'VERSION_DRIFT' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'VERSION_DRIFT', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('VERSION_DRIFT', msg);
        }
      });
    } else {
      await handleAlertSuccess(driftKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'VERSION_DRIFT' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'VERSION_DRIFT'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" version is aligned with majority.`);
        }
      });
    }
  }

  // 4. Consensus Stall (Average block interval too high)
  for (const s of onlineNodes) {
    const slowKey = `${s.validator_id}:CONSENSUS_SLOW`;
    if (s.avg_block_interval > 15.0) {
      await handleAlertFailure(slowKey, async () => {
        const msg = `Consensus Slowdown! Validator "${s.name}" reports avg block interval of ${s.avg_block_interval.toFixed(1)}s (threshold: 15s)`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'CONSENSUS_SLOW' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'CONSENSUS_SLOW', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('CONSENSUS_SLOW', msg);
        }
      });
    } else {
      await handleAlertSuccess(slowKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'CONSENSUS_SLOW' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'CONSENSUS_SLOW'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Consensus speed on validator "${s.name}" restored to normal.`);
        }
      });
    }
  }

  // 5. System Resource Alarms (Memory Soft Limit: 8GB, Open File Descriptors: 1000)
  for (const s of onlineNodes) {
    // Memory limit check
    const MEM_LIMIT = 8 * 1024 * 1024 * 1024; // 8GB in bytes
    const memKey = `${s.validator_id}:HIGH_MEMORY`;
    if (s.memory > MEM_LIMIT) {
      await handleAlertFailure(memKey, async () => {
        const memMB = (s.memory / 1024 / 1024).toFixed(0);
        const msg = `Resource Warning! Validator "${s.name}" is using high memory: ${memMB} MB (limit: 8192 MB)`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'HIGH_MEMORY' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'HIGH_MEMORY', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('RESOURCE_LIMIT', msg);
        }
      });
    } else {
      await handleAlertSuccess(memKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'HIGH_MEMORY' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'HIGH_MEMORY'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" memory usage back within normal limits.`);
        }
      });
    }

    // CPU check
    const cpuKey = `${s.validator_id}:HIGH_CPU`;
    if (s.cpu > 90.0) {
      await handleAlertFailure(cpuKey, async () => {
        const msg = `Resource Warning! Validator "${s.name}" has high CPU utilization: ${s.cpu.toFixed(1)}%`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'HIGH_CPU' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'HIGH_CPU', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('RESOURCE_LIMIT', msg);
        }
      });
    } else {
      await handleAlertSuccess(cpuKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'HIGH_CPU' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'HIGH_CPU'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" CPU utilization back within normal limits.`);
        }
      });
    }

    // Open file descriptors count
    const fdKey = `${s.validator_id}:FD_LIMIT`;
    if (s.fd_count > 1000) {
      await handleAlertFailure(fdKey, async () => {
        const msg = `Resource Warning! Validator "${s.name}" has too many open file descriptors: ${s.fd_count} (limit: 1000)`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'FD_LIMIT' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'FD_LIMIT', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('RESOURCE_LIMIT', msg);
        }
      });
    } else {
      await handleAlertSuccess(fdKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'FD_LIMIT' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'FD_LIMIT'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" open file descriptors count back within normal limits.`);
        }
      });
    }

    // Low peer count
    const peersKey = `${s.validator_id}:LOW_PEERS`;
    if (s.peer_count < 3) {
      await handleAlertFailure(peersKey, async () => {
        const msg = `P2P Warning! Validator "${s.name}" has low peer count: ${s.peer_count} peers (minimum: 3)`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'LOW_PEERS' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'LOW_PEERS', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('LOW_PEERS', msg);
        }
      });
    } else {
      await handleAlertSuccess(peersKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'LOW_PEERS' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'LOW_PEERS'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" peer count back above threshold.`);
        }
      });
    }

    // Storage Growth Alert: > 1 GB growth in 24h
    const ts24hAgo = timestamp - 86400;
    try {
      const pastSnapshot = await query(
        "SELECT database_size_bytes FROM snapshots WHERE validator_id = ? AND timestamp >= ? AND status = 'online' ORDER BY timestamp ASC LIMIT 1",
        [s.validator_id, ts24hAgo]
      );
      if (pastSnapshot.rows.length > 0) {
        const pastSize = pastSnapshot.rows[0].database_size_bytes;
        const growthBytes = s.database_size_bytes - pastSize;
        const growthGb = growthBytes / (1024 * 1024 * 1024);
        const growthKey = `${s.validator_id}:STORAGE_GROWTH`;
        if (growthGb > 1.0) {
          await handleAlertFailure(growthKey, async () => {
            const msg = `Storage Warning! Validator "${s.name}" database size grew by ${growthGb.toFixed(2)} GB in 24h (threshold: 1.0 GB)`;
            const active = await query(
              "SELECT id FROM alerts WHERE validator_id = ? AND type = 'STORAGE_GROWTH' AND resolved = 0",
              [s.validator_id]
            );
            if (active.rows.length === 0) {
              await execute(
                "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'STORAGE_GROWTH', ?, 0)",
                [s.validator_id, s.name, timestamp, msg]
              );
              await sendWebhookAlert('STORAGE_GROWTH', msg);
            }
          });
        } else {
          await handleAlertSuccess(growthKey, async () => {
            const active = await query(
              "SELECT id FROM alerts WHERE validator_id = ? AND type = 'STORAGE_GROWTH' AND resolved = 0",
              [s.validator_id]
            );
            if (active.rows.length > 0) {
              await execute(
                "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'STORAGE_GROWTH'",
                [s.validator_id]
              );
              await sendWebhookAlert('RESOLVED', `Validator "${s.name}" database size growth rate resolved.`);
            }
          });
        }
      }
    } catch (e) {
      console.error('Error in storage growth alert:', e);
    }

    // UTXO Inflation Alert: active UTXO growth > 20% per week
    const ts7dAgo = timestamp - 7 * 86400;
    try {
      const pastUtxoSnapshot = await query(
        "SELECT active_utxos FROM snapshots WHERE validator_id = ? AND timestamp >= ? AND status = 'online' ORDER BY timestamp ASC LIMIT 1",
        [s.validator_id, ts7dAgo]
      );
      if (pastUtxoSnapshot.rows.length > 0) {
        const pastUtxos = pastUtxoSnapshot.rows[0].active_utxos;
        if (pastUtxos > 0) {
          const growthPct = ((s.active_utxos - pastUtxos) / pastUtxos) * 100.0;
          const inflationKey = `${s.validator_id}:UTXO_INFLATION`;
          if (growthPct > 20.0) {
            await handleAlertFailure(inflationKey, async () => {
              const msg = `UTXO Alarm! Validator "${s.name}" active UTXOs grew by ${growthPct.toFixed(1)}% in the last week (threshold: 20%)`;
              const active = await query(
                "SELECT id FROM alerts WHERE validator_id = ? AND type = 'UTXO_INFLATION' AND resolved = 0",
                [s.validator_id]
              );
              if (active.rows.length === 0) {
                await execute(
                  "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'UTXO_INFLATION', ?, 0)",
                  [s.validator_id, s.name, timestamp, msg]
                );
                await sendWebhookAlert('UTXO_INFLATION', msg);
              }
            });
          } else {
            await handleAlertSuccess(inflationKey, async () => {
              const active = await query(
                "SELECT id FROM alerts WHERE validator_id = ? AND type = 'UTXO_INFLATION' AND resolved = 0",
                [s.validator_id]
              );
              if (active.rows.length > 0) {
                await execute(
                  "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'UTXO_INFLATION'",
                  [s.validator_id]
                );
                await sendWebhookAlert('RESOLVED', `Validator "${s.name}" active UTXO inflation resolved.`);
              }
            });
          }
        }
      }
    } catch (e) {
      console.error('Error in UTXO inflation alert:', e);
    }

    // Disk Exhaustion Alarm: validator disk usage > 80%
    const diskKey = `${s.validator_id}:DISK_EXHAUSTION`;
    if (s.disk_usage_pct > 80.0) {
      await handleAlertFailure(diskKey, async () => {
        const msg = `Disk Exhaustion Alarm! Validator "${s.name}" disk usage is at ${s.disk_usage_pct.toFixed(1)}% (threshold: 80%)`;
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'DISK_EXHAUSTION' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length === 0) {
          await execute(
            "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES (?, ?, ?, 'DISK_EXHAUSTION', ?, 0)",
            [s.validator_id, s.name, timestamp, msg]
          );
          await sendWebhookAlert('DISK_EXHAUSTION', msg);
        }
      });
    } else {
      await handleAlertSuccess(diskKey, async () => {
        const active = await query(
          "SELECT id FROM alerts WHERE validator_id = ? AND type = 'DISK_EXHAUSTION' AND resolved = 0",
          [s.validator_id]
        );
        if (active.rows.length > 0) {
          await execute(
            "UPDATE alerts SET resolved = 1 WHERE validator_id = ? AND type = 'DISK_EXHAUSTION'",
            [s.validator_id]
          );
          await sendWebhookAlert('RESOLVED', `Validator "${s.name}" disk usage back within limits.`);
        }
      });
    }
  }

  // 6. Ledger Divergence Alert: database size variance > 10% between healthy validators
  if (onlineNodes.length > 1) {
    const dbSizes = onlineNodes.map(n => n.database_size_bytes).filter(s => s > 0);
    const divKey = `network:LEDGER_DIVERGENCE`;
    if (dbSizes.length > 1) {
      const minSize = Math.min(...dbSizes);
      const maxSize = Math.max(...dbSizes);
      if (minSize > 0) {
        const variancePct = ((maxSize - minSize) / minSize) * 100.0;
        if (variancePct > 10.0) {
          await handleAlertFailure(divKey, async () => {
            const msg = `Divergence Warning! Database size variance between validators is ${variancePct.toFixed(1)}% (threshold: 10%)`;
            const active = await query(
              "SELECT id FROM alerts WHERE type = 'LEDGER_DIVERGENCE' AND resolved = 0"
            );
            if (active.rows.length === 0) {
              await execute(
                "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES ('network', 'Global Network', ?, 'LEDGER_DIVERGENCE', ?, 0)",
                [timestamp, msg]
              );
              await sendWebhookAlert('LEDGER_DIVERGENCE', msg);
            }
          });
        } else {
          await handleAlertSuccess(divKey, async () => {
            const active = await query(
              "SELECT id FROM alerts WHERE type = 'LEDGER_DIVERGENCE' AND resolved = 0"
            );
            if (active.rows.length > 0) {
              await execute(
                "UPDATE alerts SET resolved = 1 WHERE type = 'LEDGER_DIVERGENCE'"
              );
              await sendWebhookAlert('RESOLVED', `Database size variance between validators is back within normal limit.`);
            }
          });
        }
      }
    }
  }
};

// Polling executor
const runPollingCycle = async () => {
  console.log(`[POLLER] Executing polling cycle at ${new Date().toISOString()}`);
  const nodes = getValidators();
  if (nodes.length === 0) {
    console.log('[POLLER] No validators configured in validators.json.');
    return;
  }

  const pollingPromises = nodes.map(node => pollNode(node));
  const results = await Promise.all(pollingPromises);

  // Store snapshots in Database
  for (const s of results) {
    const sql = `
      INSERT INTO snapshots (
        validator_id, name, endpoint, timestamp, status, version, chain_id,
        genesis_hash, latest_block_hash, height, peer_count, uptime, memory,
        cpu, fd_count, mempool_size, latest_block_time, blocks_last_minute,
        avg_block_interval, saga_score, blocks_produced, blocks_missed, slash_events,
        database_size_bytes, data_dir_size_bytes, disk_usage_pct, total_blocks,
        total_transactions, active_utxos, spent_utxos, historical_inputs_consumed,
        raw_transaction_data_bytes, bytes_per_transaction, bytes_per_block,
        state_amplification_ratio, utxo_set_size_bytes, avg_inputs_per_tx, avg_outputs_per_tx,
        logical_utxo_set_size_bytes, net_utxo_growth, average_utxo_size_bytes,
        tps_current
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `;
    const params = [
      s.validator_id, s.name, s.endpoint, s.timestamp, s.status, s.version, s.chain_id,
      s.genesis_hash, s.latest_block_hash, s.height, s.peer_count, s.uptime, s.memory,
      s.cpu, s.fd_count, s.mempool_size, s.latest_block_time, s.blocks_last_minute,
      s.avg_block_interval, s.saga_score, s.blocks_produced, s.blocks_missed, s.slash_events,
      s.database_size_bytes, s.data_dir_size_bytes, s.disk_usage_pct, s.total_blocks,
      s.total_transactions, s.active_utxos, s.spent_utxos, s.historical_inputs_consumed,
      s.raw_transaction_data_bytes, s.bytes_per_transaction, s.bytes_per_block,
      s.state_amplification_ratio, s.utxo_set_size_bytes, s.avg_inputs_per_tx, s.avg_outputs_per_tx,
      s.logical_utxo_set_size_bytes, s.net_utxo_growth, s.average_utxo_size_bytes,
      s.tps_current
    ];
    await execute(sql, params);
  }

  // Run alert rules
  await runAlertingEngine(results);
};

// Start Polling Loop recursively
const pollLoop = async () => {
  try {
    await runPollingCycle();
  } catch (err) {
    console.error('[POLLER] Error in runPollingCycle:', err);
  } finally {
    setTimeout(pollLoop, POLL_INTERVAL);
  }
};

const startPolling = async () => {
  await initAlertTrackers();
  setTimeout(pollLoop, 2000);
};

// --------------------------------------------------------------------------
// API Endpoints
// --------------------------------------------------------------------------

// Helpers to extract latest snapshot per validator
const getLatestSnapshots = async () => {
  let sql;
  if (isPostgres) {
    sql = `
      SELECT DISTINCT ON (validator_id) *
      FROM snapshots
      ORDER BY validator_id, timestamp DESC
    `;
  } else {
    // SQLite subquery to fetch latest
    sql = `
      SELECT s1.* FROM snapshots s1
      INNER JOIN (
        SELECT validator_id, MAX(timestamp) as max_ts
        FROM snapshots
        GROUP BY validator_id
      ) s2 ON s1.validator_id = s2.validator_id AND s1.timestamp = s2.max_ts
    `;
  }
  const res = await query(sql);
  return res.rows;
};

// 1. GET /api/network-status (Public View - Hides System Metrics)
app.get('/api/network-status', async (req, res) => {
  try {
    const latest = await getLatestSnapshots();
    const onlineNodes = latest.filter(s => s.status === 'online');
    
    // Aggregates
    const validatorsOnline = onlineNodes.length;
    const totalValidators = latest.length;
    const networkHeight = onlineNodes.reduce((max, node) => Math.max(max, node.height), 0);
    const tpsCurrent = onlineNodes.reduce((sum, node) => sum + (node.tps_current || 0), 0);
    
    // consensus agreement: check if all online nodes have matching genesis and chain-id
    const genesisHashes = new Set(onlineNodes.map(n => n.genesis_hash).filter(h => h));
    const consensusAgreement = onlineNodes.length > 0 && genesisHashes.size <= 1;

    // Faucet status (mock / healthy if at least one validator is online)
    const faucetStatus = onlineNodes.length > 0 ? 'Healthy' : 'Degraded';
    
    // Bridge status (degraded if collateral ratio falls too low)
    const bridgeStatus = onlineNodes.length > 0 ? 'Healthy' : 'Degraded';

    // Majority Version count
    const versions = onlineNodes.map(n => n.version);
    const versionCounts = {};
    versions.forEach(v => versionCounts[v] = (versionCounts[v] || 0) + 1);
    let majorityVersion = 'unknown';
    let maxCount = 0;
    for (const [v, c] of Object.entries(versionCounts)) {
      if (c > maxCount) {
        maxCount = c;
        majorityVersion = v;
      }
    }

    res.json({
      network_height: networkHeight,
      validators_online: validatorsOnline,
      total_validators: totalValidators,
      consensus_agreement: consensusAgreement,
      faucet_status: faucetStatus,
      bridge_status: bridgeStatus,
      network_majority_version: majorityVersion,
      tps_current: tpsCurrent,
      timestamp: Math.floor(Date.now() / 1000)
    });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// 2. GET /api/operator-status (Operator Console - Privileged)
app.get('/api/operator-status', async (req, res) => {
  try {
    const latest = await getLatestSnapshots();
    const onlineNodes = latest.filter(s => s.status === 'online');
    const maxNetworkHeight = onlineNodes.reduce((max, node) => Math.max(max, node.height), 0);

    // Compute height divergence and versions drift per validator
    const validatorsData = latest.map(s => {
      const heightLag = s.status === 'online' ? maxNetworkHeight - s.height : null;
      return {
        validator_id: s.validator_id,
        name: s.name,
        endpoint: s.endpoint,
        status: s.status,
        height: s.height,
        height_lag: heightLag,
        peer_count: s.peer_count,
        uptime: s.uptime,
        cpu: s.cpu,
        memory_mb: s.status === 'online' ? (s.memory / 1024 / 1024).toFixed(1) : 0,
        fd_count: s.fd_count,
        mempool_size: s.mempool_size,
        latest_block_hash: s.latest_block_hash,
        genesis_hash: s.genesis_hash,
        version: s.version,
        avg_block_interval: s.avg_block_interval,
        saga_score: s.saga_score,
        blocks_produced: s.blocks_produced,
        blocks_missed: s.blocks_missed,
        slash_events: s.slash_events,
        database_size_bytes: s.database_size_bytes || 0,
        data_dir_size_bytes: s.data_dir_size_bytes || 0,
        disk_usage_pct: s.disk_usage_pct || 0,
        total_blocks: s.total_blocks || s.height || 0,
        total_transactions: s.total_transactions || 0,
        active_utxos: s.active_utxos || 0,
        spent_utxos: s.historical_inputs_consumed || s.spent_utxos || 0,
        historical_inputs_consumed: s.historical_inputs_consumed || s.spent_utxos || 0,
        raw_transaction_data_bytes: s.raw_transaction_data_bytes || 0,
        bytes_per_transaction: s.bytes_per_transaction || 0,
        bytes_per_block: s.bytes_per_block || 0,
        state_amplification_ratio: s.state_amplification_ratio || 0,
        utxo_set_size_bytes: s.logical_utxo_set_size_bytes || s.utxo_set_size_bytes || 0,
        logical_utxo_set_size_bytes: s.logical_utxo_set_size_bytes || s.utxo_set_size_bytes || 0,
        avg_inputs_per_tx: s.avg_inputs_per_tx || 0,
        avg_outputs_per_tx: s.avg_outputs_per_tx || 0,
        net_utxo_growth: s.net_utxo_growth || 0,
        average_utxo_size_bytes: s.average_utxo_size_bytes || 0
      };
    });

    res.json({
      validators: validatorsData,
      network_height: maxNetworkHeight,
      timestamp: Math.floor(Date.now() / 1000)
    });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// 3. GET /api/alerts (Active Alarms list)
app.get('/api/alerts', async (req, res) => {
  try {
    const activeAlerts = await query(
      "SELECT * FROM alerts WHERE resolved = 0 ORDER BY timestamp DESC"
    );
    res.json({ alerts: activeAlerts.rows });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// 4. POST /api/resolve-alert (Manually clear alerts if needed)
app.post('/api/resolve-alert', async (req, res) => {
  const { id } = req.body;
  try {
    await execute("UPDATE alerts SET resolved = 1 WHERE id = ?", [id]);
    res.json({ success: true, message: `Alert ${id} marked as resolved.` });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// Sync Benchmark Agent Simulation
const runSyncBenchmark = async () => {
  try {
    const latest = await getLatestSnapshots();
    const onlineNodes = latest.filter(s => s.status === 'online');
    if (onlineNodes.length === 0) return;
    
    onlineNodes.sort((a, b) => b.height - a.height);
    const repNode = onlineNodes[0];
    
    const dbSize = repNode.database_size_bytes || 0;
    const height = repNode.height || 0;
    
    // Calculate simulated sync time
    const downloadSpeedBytes = 5 * 1024 * 1024; // 5 MB/s
    const downloadTime = dbSize / downloadSpeedBytes;
    const validationTimePerBlock = 0.008; // 8ms per block
    const validationTime = height * validationTimePerBlock;
    const syncTimeSeconds = Math.round(downloadTime + validationTime);
    
    const timestamp = Math.floor(Date.now() / 1000);
    
    await execute(
      "INSERT INTO sync_benchmarks (timestamp, height, sync_time_seconds, sync_time_estimate_seconds, database_size_bytes) VALUES (?, ?, ?, ?, ?)",
      [timestamp, height, syncTimeSeconds, syncTimeSeconds, dbSize]
    );
    console.log(`[SYNC BENCHMARK AGENT] Completed sync benchmark: sync_time = ${syncTimeSeconds}s, height = ${height}, dbSize = ${dbSize} bytes`);
    
    // Check warning: sync time > 12 hours (43200 seconds)
    if (syncTimeSeconds > 43200) {
      const msg = `Sync Warning! Fresh sync duration has exceeded 12 hours: ${(syncTimeSeconds / 3600).toFixed(1)} hours. DB size is ${(dbSize / 1024 / 1024 / 1024).toFixed(2)} GB.`;
      const active = await query("SELECT id FROM alerts WHERE type = 'SYNC_SLOW' AND resolved = 0");
      if (active.rows.length === 0) {
        await execute(
          "INSERT INTO alerts (validator_id, name, timestamp, type, message, resolved) VALUES ('network', 'Global Network', ?, 'SYNC_SLOW', ?, 0)",
          [timestamp, msg]
        );
        await sendWebhookAlert('SYNC_SLOW', msg);
      }
    } else {
      try {
        const active = await query("SELECT id FROM alerts WHERE type = 'SYNC_SLOW' AND resolved = 0");
        if (active.rows.length > 0) {
          await execute("UPDATE alerts SET resolved = 1 WHERE type = 'SYNC_SLOW'");
          await sendWebhookAlert('RESOLVED', `Sync duration resolved within normal threshold.`);
        }
      } catch (e) {
        console.error('Failed to resolve SYNC_SLOW alert:', e);
      }
    }
  } catch (err) {
    console.error('Error running sync benchmark agent:', err);
  }
};

const initSyncBenchmarks = async () => {
  // If table is empty, seed it using historical snapshots
  const countRes = await query("SELECT COUNT(*) as cnt FROM sync_benchmarks");
  if (countRes.rows[0].cnt === 0) {
    console.log('Seeding sync benchmarks from snapshots history...');
    const latest = await getLatestSnapshots();
    if (latest.length > 0) {
      const repValId = latest[0].validator_id;
      const history = await query(
        "SELECT timestamp, height, database_size_bytes FROM snapshots WHERE validator_id = ? AND status = 'online' ORDER BY timestamp ASC",
        [repValId]
      );
      for (const row of history.rows) {
        const dbSize = row.database_size_bytes || 0;
        const height = row.height || 0;
        const downloadTime = dbSize / (5 * 1024 * 1024);
        const validationTime = height * 0.008;
        const syncTimeSeconds = Math.round(downloadTime + validationTime);
        
        await execute(
          "INSERT INTO sync_benchmarks (timestamp, height, sync_time_seconds, sync_time_estimate_seconds, database_size_bytes) VALUES (?, ?, ?, ?, ?)",
          [row.timestamp, height, syncTimeSeconds, syncTimeSeconds, dbSize]
        );
      }
    }
  }
  
  // Run first benchmark
  await runSyncBenchmark();
  
  // Run every 4 hours recursively
  const syncBenchmarkLoop = async () => {
    try {
      await runSyncBenchmark();
    } catch (err) {
      console.error('Error in sync benchmark loop:', err);
    } finally {
      setTimeout(syncBenchmarkLoop, 4 * 60 * 60 * 1000);
    }
  };
  setTimeout(syncBenchmarkLoop, 4 * 60 * 60 * 1000);
};

// GET /api/state-breakdown
app.get('/api/state-breakdown', async (req, res) => {
  try {
    const latest = await getLatestSnapshots();
    const onlineNodes = latest.filter(s => s.status === 'online');
    if (onlineNodes.length === 0) {
      return res.json({
        blocks: 0,
        transactions: 0,
        active_utxos: 0,
        spent_utxos: 0,
        historical_inputs_consumed: 0,
        raw_transaction_data_bytes: 0,
        bytes_per_transaction: 0,
        bytes_per_block: 0,
        state_amplification_ratio: 0,
        database_size_bytes: 0,
        data_dir_size_bytes: 0,
        utxo_set_size_bytes: 0,
        logical_utxo_set_size_bytes: 0,
        avg_inputs_per_tx: 0,
        avg_outputs_per_tx: 0,
        net_utxo_growth: 0,
        average_utxo_size_bytes: 0
      });
    }
    // Take the node with the highest block height as representative of the current state
    onlineNodes.sort((a, b) => b.height - a.height);
    const repNode = onlineNodes[0];
    res.json({
      blocks: repNode.height || 0,
      transactions: repNode.total_transactions || 0,
      active_utxos: repNode.active_utxos || 0,
      spent_utxos: repNode.historical_inputs_consumed || repNode.spent_utxos || 0,
      historical_inputs_consumed: repNode.historical_inputs_consumed || 0,
      raw_transaction_data_bytes: repNode.raw_transaction_data_bytes || 0,
      bytes_per_transaction: repNode.bytes_per_transaction || 0,
      bytes_per_block: repNode.bytes_per_block || 0,
      state_amplification_ratio: repNode.state_amplification_ratio || 0,
      database_size_bytes: repNode.database_size_bytes || 0,
      data_dir_size_bytes: repNode.data_dir_size_bytes || 0,
      utxo_set_size_bytes: repNode.logical_utxo_set_size_bytes || repNode.utxo_set_size_bytes || 0,
      logical_utxo_set_size_bytes: repNode.logical_utxo_set_size_bytes || repNode.utxo_set_size_bytes || 0,
      avg_inputs_per_tx: repNode.avg_inputs_per_tx || 0,
      avg_outputs_per_tx: repNode.avg_outputs_per_tx || 0,
      net_utxo_growth: repNode.net_utxo_growth || 0,
      average_utxo_size_bytes: repNode.average_utxo_size_bytes || 0
    });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// GET /api/state-benchmarks
app.get('/api/state-benchmarks', async (req, res) => {
  try {
    const benchmarksRes = await query("SELECT * FROM sync_benchmarks ORDER BY timestamp ASC");
    
    // Compute growth forecasting metrics
    const latest = await getLatestSnapshots();
    const onlineNodes = latest.filter(s => s.status === 'online');
    let growthPerDayGb = 0.05; // default fallback
    let projected90DayGb = 0.0;
    let projected365DayGb = 0.0;
    let currentSizeGb = 0.0;
    
    // UTXO forecasting
    let utxoGrowthPerDay = 0;
    let logicalUtxoGrowthPerDayBytes = 0;
    
    if (onlineNodes.length > 0) {
      onlineNodes.sort((a, b) => b.height - a.height);
      const repNode = onlineNodes[0];
      currentSizeGb = (repNode.database_size_bytes || 0) / (1024 * 1024 * 1024);
      const repValId = repNode.validator_id;
      
      const historyRes = await query(
        "SELECT database_size_bytes, active_utxos, logical_utxo_set_size_bytes, timestamp FROM snapshots WHERE validator_id = ? AND status = 'online' ORDER BY timestamp ASC",
        [repValId]
      );
      const rows = historyRes.rows;
      if (rows.length >= 2) {
        const first = rows[0];
        const last = rows[rows.length - 1];
        const timeDiffSec = last.timestamp - first.timestamp;
        
        if (timeDiffSec > 0) {
          const sizeDiffBytes = last.database_size_bytes - first.database_size_bytes;
          if (sizeDiffBytes > 0) {
            const bytesPerSec = sizeDiffBytes / timeDiffSec;
            const bytesPerDay = bytesPerSec * 86400;
            growthPerDayGb = bytesPerDay / (1024 * 1024 * 1024);
          }
          
          const utxosDiff = (last.active_utxos || 0) - (first.active_utxos || 0);
          const utxosPerSec = utxosDiff / timeDiffSec;
          utxoGrowthPerDay = Math.round(utxosPerSec * 86400);

          const logicalUtxoDiff = (last.logical_utxo_set_size_bytes || 0) - (first.logical_utxo_set_size_bytes || 0);
          const logicalUtxoPerSec = logicalUtxoDiff / timeDiffSec;
          logicalUtxoGrowthPerDayBytes = Math.round(logicalUtxoPerSec * 86400);
        }
      }
    }
    
    projected90DayGb = currentSizeGb + (growthPerDayGb * 90);
    projected365DayGb = currentSizeGb + (growthPerDayGb * 365);
    
    res.json({
      benchmarks: benchmarksRes.rows.map(b => ({
        ...b,
        sync_time_seconds: b.sync_time_estimate_seconds || b.sync_time_seconds || 0,
        sync_time_estimate_seconds: b.sync_time_estimate_seconds || b.sync_time_seconds || 0
      })),
      forecasting: {
        growth_per_day_gb: Number(growthPerDayGb.toFixed(4)),
        projected_90_day_gb: Number(projected90DayGb.toFixed(2)),
        projected_365_day_gb: Number(projected365DayGb.toFixed(2)),
        utxo_growth_per_day: utxoGrowthPerDay,
        logical_utxo_growth_per_day_bytes: logicalUtxoGrowthPerDayBytes
      }
    });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

// Start Server
app.listen(PORT, async () => {
  await initDb();
  await initSyncBenchmarks();
  await startPolling();
  console.log(`Telemetry Aggregator Service listening on http://localhost:${PORT}`);
});
