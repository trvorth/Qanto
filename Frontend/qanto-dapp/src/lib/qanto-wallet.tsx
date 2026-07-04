import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createPublicClient,
  createWalletClient,
  custom,
  defineChain,
  encodeFunctionData,
  http,
  type Address,
  type Hash,
  type Hex,
} from 'viem';

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

const queryClient = new QueryClient();
const publicClient = createPublicClient({
  chain: qantoChain,
  transport: http(qantoChain.rpcUrls.default.http[0]),
});

interface EthereumProvider {
  request(args: { method: string; params?: unknown[] | object }): Promise<unknown>;
  on(event: string, listener: (...args: unknown[]) => void): void;
  removeListener(event: string, listener: (...args: unknown[]) => void): void;
}

declare global {
  interface Window {
    ethereum?: EthereumProvider;
  }
}

interface WalletContextValue {
  address?: Address;
  isConnected: boolean;
  isConnecting: boolean;
  connect: () => Promise<Address>;
  disconnect: () => void;
  getProvider: () => EthereumProvider;
  getWalletClient: () => Promise<ReturnType<typeof createWalletClient>>;
}

interface Web3ProviderProps {
  children: ReactNode;
}

interface MutationCallbacks {
  mutation?: {
    onError?: (error: Error) => void;
  };
}

interface SendTransactionRequest {
  to: Address;
  value?: bigint;
  data?: Hex;
}

interface WriteContractRequest {
  abi: readonly unknown[];
  address: Address;
  functionName: string;
  args?: readonly unknown[];
  value?: bigint;
}

interface BalanceHookOptions {
  address?: Address;
  query?: {
    enabled?: boolean;
  };
}

interface ConnectButtonProps {
  label?: string;
  showBalance?: boolean;
  chainStatus?: 'icon' | 'name' | 'none';
}

const WalletContext = createContext<WalletContextValue | null>(null);

function normalizeError(error: unknown): Error {
  if (error instanceof Error) {
    return error;
  }
  return new Error(typeof error === 'string' ? error : 'Unknown wallet error');
}

function formatAddress(address?: string): string {
  if (!address) {
    return 'Connect Wallet';
  }
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

function getInjectedProvider(): EthereumProvider {
  if (typeof window === 'undefined' || !window.ethereum) {
    throw new Error('No injected EVM wallet found in this browser.');
  }
  return window.ethereum;
}

async function ensureQantoChain(provider: EthereumProvider): Promise<void> {
  const chainIdHex = `0x${qantoChain.id.toString(16)}`;

  try {
    await provider.request({
      method: 'wallet_switchEthereumChain',
      params: [{ chainId: chainIdHex }],
    });
  } catch (error) {
    const switchError = error as { code?: number; message?: string };
    if (switchError.code !== 4902) {
      throw normalizeError(error);
    }

    await provider.request({
      method: 'wallet_addEthereumChain',
      params: [
        {
          chainId: chainIdHex,
          chainName: qantoChain.name,
          nativeCurrency: qantoChain.nativeCurrency,
          rpcUrls: qantoChain.rpcUrls.default.http,
          blockExplorerUrls: [qantoChain.blockExplorers.default.url],
        },
      ],
    });
  }
}

export function Web3Provider({ children }: Web3ProviderProps) {
  const [address, setAddress] = useState<Address | undefined>();
  const [isConnecting, setIsConnecting] = useState(false);

  const getProvider = useCallback(() => getInjectedProvider(), []);

  const getWalletClient = useCallback(async () => {
    const provider = getProvider();
    await ensureQantoChain(provider);
    return createWalletClient({
      chain: qantoChain,
      transport: custom(provider),
    });
  }, [getProvider]);

  const syncAccounts = useCallback(async () => {
    try {
      const provider = getProvider();
      const accounts = (await provider.request({
        method: 'eth_accounts',
      })) as string[];

      if (accounts.length > 0) {
        setAddress(accounts[0] as Address);
      } else {
        setAddress(undefined);
      }
    } catch {
      setAddress(undefined);
    }
  }, [getProvider]);

  useEffect(() => {
    void syncAccounts();

    if (typeof window === 'undefined' || !window.ethereum) {
      return;
    }

    const provider = window.ethereum;

    const handleAccountsChanged = (accounts: unknown) => {
      const nextAccounts = Array.isArray(accounts) ? accounts : [];
      setAddress(nextAccounts[0] as Address | undefined);
    };

    const handleChainChanged = () => {
      void syncAccounts();
    };

    provider.on('accountsChanged', handleAccountsChanged);
    provider.on('chainChanged', handleChainChanged);

    return () => {
      provider.removeListener('accountsChanged', handleAccountsChanged);
      provider.removeListener('chainChanged', handleChainChanged);
    };
  }, [syncAccounts]);

  const connect = useCallback(async () => {
    setIsConnecting(true);

    try {
      const provider = getProvider();
      await ensureQantoChain(provider);

      const accounts = (await provider.request({
        method: 'eth_requestAccounts',
      })) as string[];

      const nextAddress = accounts[0] as Address | undefined;
      if (!nextAddress) {
        throw new Error('The wallet did not return an account.');
      }

      setAddress(nextAddress);
      return nextAddress;
    } catch (error) {
      throw normalizeError(error);
    } finally {
      setIsConnecting(false);
    }
  }, [getProvider]);

  const disconnect = useCallback(() => {
    setAddress(undefined);
  }, []);

  const contextValue = useMemo<WalletContextValue>(
    () => ({
      address,
      isConnected: Boolean(address),
      isConnecting,
      connect,
      disconnect,
      getProvider,
      getWalletClient,
    }),
    [address, connect, disconnect, getProvider, getWalletClient, isConnecting],
  );

  return (
    <WalletContext.Provider value={contextValue}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </WalletContext.Provider>
  );
}

function useWalletContext(): WalletContextValue {
  const context = useContext(WalletContext);
  if (!context) {
    throw new Error('Wallet hooks must be used inside Web3Provider.');
  }
  return context;
}

export function useWalletSession() {
  const { address, isConnected, isConnecting, connect, disconnect } = useWalletContext();
  return { address, isConnected, isConnecting, connect, disconnect };
}

export function useAccount() {
  const { address, isConnected } = useWalletContext();
  return { address, isConnected };
}

export function useConnectModal() {
  const { connect, isConnecting } = useWalletContext();
  return {
    openConnectModal: connect,
    connectModalOpen: isConnecting,
  };
}

export function ConnectButton({ label }: ConnectButtonProps) {
  const { address, isConnected, isConnecting, connect, disconnect } = useWalletSession();

  const handleClick = async () => {
    if (isConnected) {
      disconnect();
      return;
    }

    await connect();
  };

  return (
    <button
      type="button"
      onClick={() => {
        void handleClick();
      }}
      className="rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-4 py-2 text-sm font-semibold text-cyan-300 transition-all hover:border-cyan-400 hover:bg-cyan-500 hover:text-white"
    >
      {isConnecting ? 'Connecting...' : isConnected ? formatAddress(address) : label ?? 'Connect Wallet'}
    </button>
  );
}

export function useSendTransaction(options?: MutationCallbacks) {
  const { address, connect, getWalletClient } = useWalletContext();
  const [data, setData] = useState<Hash | undefined>();
  const [error, setError] = useState<Error | null>(null);
  const [isPending, setIsPending] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);

  const sendTransaction = useCallback(
    async (request: SendTransactionRequest) => {
      setIsPending(true);
      setError(null);
      setIsSuccess(false);

      try {
        const account = address ?? (await connect());
        const walletClient = await getWalletClient();
        const hash = await walletClient.sendTransaction({
          account,
          chain: qantoChain,
          to: request.to,
          value: request.value ?? 0n,
          data: request.data,
        });

        setData(hash);
        setIsSuccess(true);
        return hash;
      } catch (nextError) {
        const normalized = normalizeError(nextError);
        setError(normalized);
        options?.mutation?.onError?.(normalized);
        throw normalized;
      } finally {
        setIsPending(false);
      }
    },
    [address, connect, getWalletClient, options],
  );

  return { sendTransaction, data, error, isPending, isSuccess };
}

export function useWriteContract(options?: MutationCallbacks) {
  const sendTransactionState = useSendTransaction(options);

  const writeContract = useCallback(
    async (request: WriteContractRequest) => {
      const data = encodeFunctionData({
        abi: request.abi,
        functionName: request.functionName,
        args: request.args,
      });

      return sendTransactionState.sendTransaction({
        to: request.address,
        data,
        value: request.value,
      });
    },
    [sendTransactionState],
  );

  return {
    writeContract,
    data: sendTransactionState.data,
    error: sendTransactionState.error,
    isPending: sendTransactionState.isPending,
    isSuccess: sendTransactionState.isSuccess,
  };
}

export function useWaitForTransactionReceipt({ hash }: { hash?: Hash }) {
  const [isLoading, setIsLoading] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const lastHashRef = useRef<Hash | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;

    if (!hash) {
      setIsLoading(false);
      setIsSuccess(false);
      setError(null);
      lastHashRef.current = undefined;
      return () => {
        cancelled = true;
      };
    }

    if (lastHashRef.current === hash && (isLoading || isSuccess)) {
      return () => {
        cancelled = true;
      };
    }

    lastHashRef.current = hash;
    setIsLoading(true);
    setIsSuccess(false);
    setError(null);

    void publicClient
      .waitForTransactionReceipt({ hash })
      .then(() => {
        if (cancelled) {
          return;
        }
        setIsSuccess(true);
      })
      .catch((nextError) => {
        if (cancelled) {
          return;
        }
        setError(normalizeError(nextError));
      })
      .finally(() => {
        if (cancelled) {
          return;
        }
        setIsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [hash, isLoading, isSuccess]);

  return { isLoading, isSuccess, error };
}

export function useBalance({ address, query }: BalanceHookOptions) {
  const enabled = query?.enabled ?? Boolean(address);
  const [data, setData] = useState<{ value: bigint } | undefined>();
  const [error, setError] = useState<Error | null>(null);

  const refetch = useCallback(async () => {
    if (!enabled || !address) {
      setData(undefined);
      return { data: undefined };
    }

    try {
      const value = await publicClient.getBalance({ address });
      const nextData = { value };
      setData(nextData);
      setError(null);
      return { data: nextData };
    } catch (nextError) {
      const normalized = normalizeError(nextError);
      setError(normalized);
      return { data: undefined, error: normalized };
    }
  }, [address, enabled]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { data, error, refetch };
}
