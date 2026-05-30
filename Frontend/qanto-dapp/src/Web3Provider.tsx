import React from 'react';
import '@rainbow-me/rainbowkit/styles.css';
import { getDefaultConfig, RainbowKitProvider, darkTheme } from '@rainbow-me/rainbowkit';
import { WagmiProvider } from 'wagmi';
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { defineChain } from 'viem';

export const qantoChain = defineChain({
  id: 21313,
  name: 'QANTO Testnet',
  nativeCurrency: { name: 'QANTO', symbol: 'QNTO', decimals: 9 },
  rpcUrls: {
    default: { http: ['https://trvorth-qanto-testnet.hf.space/rpc'] },
  },
  blockExplorers: {
    default: { name: 'Qanto Explorer', url: 'https://qanto.org/explorer' },
  },
});

const config = getDefaultConfig({
  appName: 'Qanto dApp',
  projectId: 'a0f8bfd8b8ea5f32a76f2f2679cc7a00', // Injected dummy WalletConnect ID
  chains: [qantoChain],
});

const queryClient = new QueryClient();

interface Web3ProviderProps {
  children: React.ReactNode;
}

export function Web3Provider({ children }: Web3ProviderProps) {
  return (
    <WagmiProvider config={config}>
      <QueryClientProvider client={queryClient}>
        <RainbowKitProvider theme={darkTheme({
          accentColor: '#8b5cf6', // Quantum Purple accent
          accentColorForeground: 'white',
          borderRadius: 'large',
          overlayBlur: 'small',
        })}>
          {children}
        </RainbowKitProvider>
      </QueryClientProvider>
    </WagmiProvider>
  );
}
