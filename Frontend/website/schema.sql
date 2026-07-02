CREATE TABLE IF NOT EXISTS waitlist_subscribers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  email TEXT NOT NULL COLLATE NOCASE UNIQUE,
  subscribed_at TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'coming-soon',
  referrer TEXT,
  user_agent TEXT,
  ip_country TEXT,
  ip_hash TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (length(email) BETWEEN 3 AND 320)
);

CREATE INDEX IF NOT EXISTS idx_waitlist_subscribers_subscribed_at
  ON waitlist_subscribers (subscribed_at DESC);

CREATE INDEX IF NOT EXISTS idx_waitlist_subscribers_source
  ON waitlist_subscribers (source);
