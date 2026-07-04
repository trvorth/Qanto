import { useState, useEffect, useCallback } from 'react';
import { formatUnits } from 'viem';
import { request, gql } from 'graphql-request';
import { qantoChain, useAccount, useBalance } from '../lib/qanto-wallet';

const GRAPHQL_ENDPOINT = 'https://trvorth-qanto-testnet.hf.space/graphql';
const RPC_ENDPOINT = qantoChain.rpcUrls.default.http[0];
const QANTO_DECIMALS = qantoChain.nativeCurrency.decimals;

export function useQantoBalance() {
  const { address, isConnected } = useAccount();
  const [balanceData, setBalanceData] = useState({
    confirmed: 0,
    pending: 0,
    live: 0,
    isLoading: false,
    isError: false,
  });

  // Primary injected-wallet balance query.
  const { data: walletBalance, refetch: refetchWalletBalance } = useBalance({
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
        const balanceBaseUnits = BigInt(data.result);
        const amount = Number(formatUnits(balanceBaseUnits, QANTO_DECIMALS));
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
    if (refetchWalletBalance) {
      const { data } = await refetchWalletBalance();
      if (data) {
        const val = Number(formatUnits(data.value, QANTO_DECIMALS));
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
  }, [refetchWalletBalance, fetchFallbackBalance]);

  useEffect(() => {
    if (isConnected && address) {
      if (walletBalance) {
        const val = Number(formatUnits(walletBalance.value, QANTO_DECIMALS));
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
  }, [isConnected, address, walletBalance, fetchFallbackBalance]);

  return {
    ...balanceData,
    refresh,
  };
}
