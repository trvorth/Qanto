import { useAccount, useBalance } from 'wagmi';
import { useState, useEffect, useCallback } from 'react';
import { request, gql } from 'graphql-request';

const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';
const RPC_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/rpc';

export function useQantoBalance() {
  const { address, isConnected } = useAccount();
  const [balanceData, setBalanceData] = useState({
    confirmed: 0,
    pending: 0,
    live: 0,
    isLoading: false,
    isError: false,
  });

  // Wagmi useBalance hook
  const { data: wagmiBalance, refetch: refetchWagmi } = useBalance({
    address,
    query: {
      enabled: isConnected && !!address,
    }
  });

  const fetchFallbackBalance = useCallback(async () => {
    if (!address) return;
    setBalanceData(prev => ({ ...prev, isLoading: true }));

    // 1. Try GraphQL Fallback
    try {
      const query = gql`
        query GetBalance($address: String!) {
          balance(address: $address) {
            amount
          }
        }
      `;
      const gqlRes = await request<{ balance: { amount: number } }>(GRAPHQL_ENDPOINT, query, { address });
      const amount = gqlRes.balance?.amount || 0;
      setBalanceData({
        confirmed: amount,
        pending: 0,
        live: amount,
        isLoading: false,
        isError: false,
      });
      return;
    } catch (gqlErr) {
      console.warn('GraphQL Balance fallback failed, trying RPC:', gqlErr);
    }

    // 2. Try RPC Fallback
    try {
      const response = await fetch(RPC_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0',
          method: 'eth_getBalance',
          params: [address, 'latest'],
          id: 1,
        }),
      });
      const data = await response.json();
      if (data.result) {
        // Balance comes in Wei (18 decimals), convert to QNTO representation
        const balanceWei = BigInt(data.result);
        const amount = Number(balanceWei) / 10 ** 18;
        setBalanceData({
          confirmed: amount,
          pending: 0,
          live: amount,
          isLoading: false,
          isError: false,
        });
        return;
      }
    } catch (rpcErr) {
      console.error('RPC Balance fallback failed:', rpcErr);
    }

    setBalanceData(prev => ({ ...prev, isLoading: false, isError: true }));
  }, [address]);

  // Refresh balance
  const refresh = useCallback(async () => {
    if (refetchWagmi) {
      const { data } = await refetchWagmi();
      if (data) {
        const val = Number(data.value) / 10 ** 18; // Convert to standard balance representation
        setBalanceData({
          confirmed: val,
          pending: 0,
          live: val,
          isLoading: false,
          isError: false,
        });
        return;
      }
    }
    await fetchFallbackBalance();
  }, [refetchWagmi, fetchFallbackBalance]);

  useEffect(() => {
    if (isConnected && address) {
      if (wagmiBalance) {
        const val = Number(wagmiBalance.value) / 10 ** 18;
        setBalanceData({
          confirmed: val,
          pending: 0,
          live: val,
          isLoading: false,
          isError: false,
        });
      } else {
        fetchFallbackBalance();
      }
    } else {
      setBalanceData({
        confirmed: 0,
        pending: 0,
        live: 0,
        isLoading: false,
        isError: false,
      });
    }
  }, [isConnected, address, wagmiBalance, fetchFallbackBalance]);

  return {
    ...balanceData,
    refresh,
  };
}
