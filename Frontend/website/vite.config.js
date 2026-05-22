import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        explorer: resolve(__dirname, 'explorer.html'),
        faucet: resolve(__dirname, 'faucet.html'),
        qantoswap: resolve(__dirname, 'qantoswap.html')
      }
    }
  }
});
