import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        explorer: resolve(__dirname, 'explorer.html'),
        faucet: resolve(__dirname, 'faucet.html'),
        qantoswap: resolve(__dirname, 'qantoswap.html'),
        staking: resolve(__dirname, 'staking.html'),
        bridge: resolve(__dirname, 'bridge.html'),
        analytics: resolve(__dirname, 'analytics.html'),
        whitepaper: resolve(__dirname, 'whitepaper.html'),
        airdrop: resolve(__dirname, 'airdrop.html'),
        saga: resolve(__dirname, 'saga.html'),
        tge: resolve(__dirname, 'tge.html'),
        genesis: resolve(__dirname, 'genesis.html')
      }
    }
  }
});
