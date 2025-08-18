const express = require('express');
const axios = require('axios');
const cors = require('cors');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;
const QANTO_RPC = process.env.QANTO_RPC_URL || 'http://localhost:8082';

// Middleware
app.use(cors());
app.use(express.json({ limit: '1mb' }));
app.use(express.static(path.join(__dirname, 'public')));

// Cache for reducing API calls (free-tier optimization)
const cache = new Map();
const CACHE_TTL = 30000; // 30 seconds

function getCached(key) {
    const item = cache.get(key);
    if (item && Date.now() - item.timestamp < CACHE_TTL) {
        return item.data;
    }
    cache.delete(key);
    return null;
}

function setCache(key, data) {
    // Limit cache size for memory optimization
    if (cache.size > 100) {
        const firstKey = cache.keys().next().value;
        cache.delete(firstKey);
    }
    cache.set(key, { data, timestamp: Date.now() });
}

// API Routes
app.get('/api/health', (req, res) => {
    res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

app.get('/api/status', async (req, res) => {
    try {
        const cached = getCached('status');
        if (cached) {
            return res.json(cached);
        }

        const response = await axios.get(`${QANTO_RPC}/status`, { timeout: 5000 });
        setCache('status', response.data);
        res.json(response.data);
    } catch (error) {
        console.error('Status API error:', error.message);
        res.status(500).json({ 
            error: 'Failed to fetch node status',
            message: error.message 
        });
    }
});

app.get('/api/blocks', async (req, res) => {
    try {
        const limit = Math.min(parseInt(req.query.limit) || 10, 50); // Limit for free-tier
        const cacheKey = `blocks_${limit}`;
        const cached = getCached(cacheKey);
        if (cached) {
            return res.json(cached);
        }

        const response = await axios.get(`${QANTO_RPC}/blocks?limit=${limit}`, { timeout: 5000 });
        setCache(cacheKey, response.data);
        res.json(response.data);
    } catch (error) {
        console.error('Blocks API error:', error.message);
        res.status(500).json({ 
            error: 'Failed to fetch blocks',
            message: error.message 
        });
    }
});

app.get('/api/block/:hash', async (req, res) => {
    try {
        const { hash } = req.params;
        const cached = getCached(`block_${hash}`);
        if (cached) {
            return res.json(cached);
        }

        const response = await axios.get(`${QANTO_RPC}/block/${hash}`, { timeout: 5000 });
        setCache(`block_${hash}`, response.data);
        res.json(response.data);
    } catch (error) {
        console.error('Block API error:', error.message);
        res.status(500).json({ 
            error: 'Failed to fetch block',
            message: error.message 
        });
    }
});

app.get('/api/transaction/:hash', async (req, res) => {
    try {
        const { hash } = req.params;
        const cached = getCached(`tx_${hash}`);
        if (cached) {
            return res.json(cached);
        }

        const response = await axios.get(`${QANTO_RPC}/transaction/${hash}`, { timeout: 5000 });
        setCache(`tx_${hash}`, response.data);
        res.json(response.data);
    } catch (error) {
        console.error('Transaction API error:', error.message);
        res.status(500).json({ 
            error: 'Failed to fetch transaction',
            message: error.message 
        });
    }
});

app.get('/api/stats', async (req, res) => {
    try {
        const cached = getCached('stats');
        if (cached) {
            return res.json(cached);
        }

        // Aggregate basic stats
        const [statusRes, blocksRes] = await Promise.all([
            axios.get(`${QANTO_RPC}/status`, { timeout: 5000 }),
            axios.get(`${QANTO_RPC}/blocks?limit=1`, { timeout: 5000 })
        ]);

        const stats = {
            node_status: statusRes.data,
            latest_block: blocksRes.data[0] || null,
            timestamp: new Date().toISOString()
        };

        setCache('stats', stats);
        res.json(stats);
    } catch (error) {
        console.error('Stats API error:', error.message);
        res.status(500).json({ 
            error: 'Failed to fetch stats',
            message: error.message 
        });
    }
});

// Serve main page
app.get('/', (req, res) => {
    res.sendFile(path.join(__dirname, 'public', 'index.html'));
});

// Error handling middleware
app.use((err, req, res, next) => {
    console.error('Server error:', err);
    res.status(500).json({ error: 'Internal server error' });
});

// 404 handler
app.use((req, res) => {
    res.status(404).json({ error: 'Not found' });
});

// Graceful shutdown
process.on('SIGTERM', () => {
    console.log('SIGTERM received, shutting down gracefully');
    server.close(() => {
        console.log('Server closed');
        process.exit(0);
    });
});

const server = app.listen(PORT, '0.0.0.0', () => {
    console.log(`Qanto Block Explorer (Free-Tier) running on port ${PORT}`);
    console.log(`Connecting to Qanto RPC at: ${QANTO_RPC}`);
});

// Set server timeout for free-tier optimization
server.timeout = 30000; // 30 seconds