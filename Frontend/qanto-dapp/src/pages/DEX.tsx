import { useEffect, useMemo, useRef, useState } from 'react';
import { formatUnits, parseUnits, zeroAddress, type Address } from 'viem';
import { toast } from 'react-hot-toast';
import {
  readQantoContract,
  useAccount,
  useConnectModal,
  useWaitForTransactionReceipt,
  useWriteContract,
} from '../lib/qanto-wallet';

const ROUTER_ADDRESS = '0x9F00000000000000000000000000000000000004' as Address;
const FACTORY_ADDRESS = '0x9F00000000000000000000000000000000000003' as Address;

const ERC20_ABI = [
  {
    type: 'function',
    name: 'balanceOf',
    stateMutability: 'view',
    inputs: [{ name: 'account', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'allowance',
    stateMutability: 'view',
    inputs: [
      { name: 'owner', type: 'address' },
      { name: 'spender', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'approve',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'spender', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
] as const;

const FACTORY_ABI = [
  {
    type: 'function',
    name: 'getPair',
    stateMutability: 'view',
    inputs: [
      { name: 'tokenA', type: 'address' },
      { name: 'tokenB', type: 'address' },
    ],
    outputs: [{ name: 'pair', type: 'address' }],
  },
] as const;

const ROUTER_ABI = [
  {
    type: 'function',
    name: 'swapExactTokensForTokens',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'amountIn', type: 'uint256' },
      { name: 'amountOutMin', type: 'uint256' },
      { name: 'path', type: 'address[]' },
      { name: 'to', type: 'address' },
      { name: 'deadline', type: 'uint256' },
    ],
    outputs: [{ name: 'amounts', type: 'uint256[]' }],
  },
] as const;

const TOKENS = {
  WQNTO: {
    symbol: 'WQNTO',
    name: 'Wrapped QANTO',
    address: '0x9F00000000000000000000000000000000000001' as Address,
    decimals: 9,
  },
  QUSD: {
    symbol: 'QUSD',
    name: 'Qanto USD',
    address: '0x9F00000000000000000000000000000000000002' as Address,
    decimals: 18,
  },
} as const;

type TokenKey = keyof typeof TOKENS;
type DexAction = 'approve' | 'swap' | null;

const formatTokenAmount = (value: bigint, decimals: number) =>
  Number(formatUnits(value, decimals)).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  });

export const DEX = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();

  const [fromToken, setFromToken] = useState<TokenKey>('WQNTO');
  const [toToken, setToToken] = useState<TokenKey>('QUSD');
  const [fromAmount, setFromAmount] = useState('');
  const [pairAddress, setPairAddress] = useState<Address | null>(null);
  const [pairCheckError, setPairCheckError] = useState<string | null>(null);
  const [allowance, setAllowance] = useState<bigint>(0n);
  const [balances, setBalances] = useState<Record<TokenKey, bigint>>({
    WQNTO: 0n,
    QUSD: 0n,
  });
  const [lastAction, setLastAction] = useState<DexAction>(null);

  const { writeContract, data: txHash, isPending: isWritePending } = useWriteContract({
    mutation: {
      onError: (error: Error & { code?: number }) => {
        if (toastId.current) {
          toast.dismiss(toastId.current);
          toastId.current = null;
        }
        if (error.message?.includes('User rejected') || error.code === 4001) {
          toast.error('Transaction cancelled by user.');
        } else {
          toast.error(error.message || 'DEX transaction failed on-chain.');
        }
      },
    },
  });
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);
  const lastConfirmedHashRef = useRef<string | null>(null);
  const fromTokenConfig = TOKENS[fromToken];
  const toTokenConfig = TOKENS[toToken];
  const isPending = isWritePending || isConfirming;

  const parsedAmount = useMemo(() => {
    const trimmed = fromAmount.trim();
    if (!trimmed) {
      return null;
    }

    try {
      const amount = parseUnits(trimmed, fromTokenConfig.decimals);
      return amount > 0n ? amount : null;
    } catch {
      return null;
    }
  }, [fromAmount, fromTokenConfig.decimals]);

  const hasPair = Boolean(pairAddress && pairAddress !== zeroAddress);
  const requiresApproval = Boolean(parsedAmount && allowance < parsedAmount);

  const refreshDexState = async (walletAddress?: Address) => {
    try {
      const nextPairAddress = (await readQantoContract({
        abi: FACTORY_ABI,
        address: FACTORY_ADDRESS,
        functionName: 'getPair',
        args: [TOKENS.WQNTO.address, TOKENS.QUSD.address],
      })) as Address;
      setPairAddress(nextPairAddress);
      setPairCheckError(null);
    } catch (error) {
      setPairAddress(null);
      setPairCheckError((error as Error).message || 'Failed to read live pair address.');
    }

    if (!walletAddress) {
      setBalances({ WQNTO: 0n, QUSD: 0n });
      setAllowance(0n);
      return;
    }

    try {
      const [wqntoBalance, qusdBalance, nextAllowance] = await Promise.all([
        readQantoContract({
          abi: ERC20_ABI,
          address: TOKENS.WQNTO.address,
          functionName: 'balanceOf',
          args: [walletAddress],
        }) as Promise<bigint>,
        readQantoContract({
          abi: ERC20_ABI,
          address: TOKENS.QUSD.address,
          functionName: 'balanceOf',
          args: [walletAddress],
        }) as Promise<bigint>,
        readQantoContract({
          abi: ERC20_ABI,
          address: fromTokenConfig.address,
          functionName: 'allowance',
          args: [walletAddress, ROUTER_ADDRESS],
        }) as Promise<bigint>,
      ]);

      setBalances({
        WQNTO: wqntoBalance,
        QUSD: qusdBalance,
      });
      setAllowance(nextAllowance);
    } catch (error) {
      console.error('Failed to read DEX state', error);
    }
  };

  useEffect(() => {
    void refreshDexState(address);
  }, [address, fromToken]);

  useEffect(() => {
    if (!isConfirmed || !txHash || lastConfirmedHashRef.current === txHash) {
      return;
    }

    lastConfirmedHashRef.current = txHash;
    if (toastId.current) {
      toast.dismiss(toastId.current);
      toastId.current = null;
    }

    if (lastAction === 'approve') {
      toast.success(`${fromTokenConfig.symbol} approval confirmed.`);
    } else if (lastAction === 'swap') {
      toast.success('Router swap confirmed on QANTO Testnet.');
      setFromAmount('');
    }

    void refreshDexState(address);
  }, [address, fromTokenConfig.symbol, isConfirmed, lastAction, txHash]);

  const switchTokens = () => {
    setFromToken(toToken);
    setToToken(fromToken);
    setFromAmount('');
    setAllowance(0n);
  };

  const handleSwap = () => {
    if (!isConnected) {
      if (openConnectModal) {
        void openConnectModal();
      } else {
        toast.error('Connect wallet to perform swap.');
      }
      return;
    }

    if (!address) {
      toast.error('Connected wallet address is unavailable.');
      return;
    }

    if (!parsedAmount) {
      toast.error(`Enter a valid ${fromTokenConfig.symbol} amount.`);
      return;
    }

    if (!hasPair) {
      toast.error('No live WQNTO/QUSD pair is readable from the testnet factory.');
      return;
    }

    if (balances[fromToken] < parsedAmount) {
      toast.error(`Insufficient ${fromTokenConfig.symbol} balance for this swap.`);
      return;
    }

    const deadline = BigInt(Math.floor(Date.now() / 1000) + 20 * 60);

    if (requiresApproval) {
      setLastAction('approve');
      toastId.current = toast.loading(`Approving ${fromTokenConfig.symbol} for router spending...`);
      void writeContract({
        abi: ERC20_ABI,
        address: fromTokenConfig.address,
        functionName: 'approve',
        args: [ROUTER_ADDRESS, parsedAmount],
      });
      return;
    }

    setLastAction('swap');
    toastId.current = toast.loading('Submitting router swap to QANTO Testnet...');
    void writeContract({
      abi: ROUTER_ABI,
      address: ROUTER_ADDRESS,
      functionName: 'swapExactTokensForTokens',
      args: [parsedAmount, 0n, [fromTokenConfig.address, toTokenConfig.address], address, deadline],
    });
  };

  let swapButtonText = 'SWAP TOKENS';
  if (!isConnected) {
    swapButtonText = 'Connect Wallet';
  } else if (isPending) {
    swapButtonText = lastAction === 'approve' ? `Approving ${fromTokenConfig.symbol}...` : 'Submitting Swap...';
  } else if (!hasPair) {
    swapButtonText = 'No Live Pair';
  } else if (requiresApproval) {
    swapButtonText = `Approve ${fromTokenConfig.symbol}`;
  }

  return (
    <div className="w-full max-w-xl mx-auto px-4 py-8 relative z-10">
      <div className="backdrop-blur-md bg-[#0a0a14]/60 border border-white/10 rounded-[28px] p-6 md:p-8 shadow-[0_0_50px_rgba(6,182,212,0.1)] relative overflow-hidden">
        {/* Decorative background radial glow */}
        <div className="absolute -top-1/2 -left-1/2 w-[200%] h-[200%] bg-[radial-gradient(circle,rgba(6,182,212,0.05)_0%,transparent_60%)] animate-spin z-0 pointer-events-none" style={{ animationDuration: '25s' }} />

        <div className="relative z-10">
          <h1 className="text-3xl font-extrabold text-white text-center mb-2 font-sans tracking-tight">
            Quantum Swap DEX
          </h1>
          <p className="text-sm text-slate-400 text-center font-sans mb-8">
            Swap live testnet ERC-20 assets through the deployed QantoSwap router. No synthetic prices or demo balances remain.
          </p>

          {/* Swap Box */}
          <div className="space-y-4 mb-6">
            {/* Input Box 1 */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div className="flex justify-between text-xs text-slate-500 font-mono">
                <span>Pay</span>
                <span>
                  Balance: {isConnected ? `${formatTokenAmount(balances[fromToken], fromTokenConfig.decimals)} ${fromTokenConfig.symbol}` : `0.00 ${fromTokenConfig.symbol}`}
                </span>
              </div>
              <div className="flex justify-between items-center gap-4">
                <input
                  type="number"
                  value={fromAmount}
                  onChange={(e) => setFromAmount(e.target.value)}
                  disabled={isPending}
                  className="bg-transparent text-2xl font-bold font-mono text-white outline-none border-none w-full p-0 focus:ring-0"
                  placeholder="0.0"
                />
                <select
                  value={fromToken}
                  onChange={(e) => setFromToken(e.target.value as TokenKey)}
                  disabled={isPending}
                  className="bg-white/5 border border-white/10 text-white font-bold font-mono px-3.5 py-1.5 rounded-xl text-sm"
                >
                  {Object.entries(TOKENS).map(([key, token]) => (
                    <option key={key} value={key} className="bg-slate-950">
                      {token.symbol}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {/* Switch Button */}
            <div className="flex justify-center -my-3 relative z-20">
              <button
                onClick={switchTokens}
                disabled={isPending}
                className="w-10 h-10 rounded-xl bg-violet-600 hover:bg-violet-500 text-white flex items-center justify-center border border-white/10 transition-transform duration-200 hover:scale-110 shadow-lg shadow-purple-900/30"
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <path d="M7 16V4M7 4L3 8M7 4l4 4M17 8v12M17 20l-4-4M17 20l4-4" />
                </svg>
              </button>
            </div>

            {/* Input Box 2 */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div className="flex justify-between text-xs text-slate-500 font-mono">
                <span>Receive</span>
                <span>
                  Balance: {isConnected ? `${formatTokenAmount(balances[toToken], toTokenConfig.decimals)} ${toTokenConfig.symbol}` : `0.00 ${toTokenConfig.symbol}`}
                </span>
              </div>
              <div className="flex justify-between items-center gap-4">
                <input
                  type="text"
                  value={hasPair ? 'Determined on-chain at execution' : '0.00'}
                  disabled
                  className="bg-transparent text-lg font-bold font-mono text-slate-400 outline-none border-none w-full p-0 focus:ring-0"
                />
                <select
                  value={toToken}
                  onChange={(e) => setToToken(e.target.value as TokenKey)}
                  disabled={isPending}
                  className="bg-white/5 border border-white/10 text-white font-bold font-mono px-3.5 py-1.5 rounded-xl text-sm"
                >
                  {Object.entries(TOKENS).map(([key, token]) => (
                    <option key={key} value={key} className="bg-slate-950">
                      {token.symbol}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </div>

          {/* Details */}
          <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mb-8 text-xs font-mono text-slate-400 space-y-2">
            <div className="flex justify-between">
              <span>Router:</span>
              <span className="text-cyan-400 break-all">{ROUTER_ADDRESS}</span>
            </div>
            <div className="flex justify-between">
              <span>Factory Pair:</span>
              <span className={hasPair ? 'text-emerald-400 break-all' : 'text-rose-400'}>
                {pairAddress ?? 'unavailable'}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Allowance:</span>
              <span>{formatTokenAmount(allowance, fromTokenConfig.decimals)} {fromTokenConfig.symbol}</span>
            </div>
            <div className="flex justify-between">
              <span>Estimated Output:</span>
              <span>{hasPair ? 'Quote not exposed by current router contract' : '0.00'}</span>
            </div>
            <div className="text-[10px] text-slate-500 leading-relaxed">
              The current testnet router exposes execution but not a quote endpoint. This page submits real approvals and swaps, and only reports live pair availability or chain errors.
            </div>
            <div className="text-[10px] text-slate-500 leading-relaxed">
              WQNTO is the wrapped 9-decimal form of native QNTO. If your wallet only holds native QNTO, wrap it before swapping.
            </div>
            {pairCheckError && (
              <div className="text-[10px] text-rose-400 break-all">
                Live pair lookup failed: {pairCheckError}
              </div>
            )}
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleSwap}
              disabled={isPending || (isConnected && !hasPair)}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected
                  ? 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
                  : isPending
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : !hasPair
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white hover:shadow-[0_0_20px_rgba(6,182,212,0.6)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {swapButtonText}
            </button>

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all text-center">
                <div>Connected User: {address}</div>
                <div>Input Token: {fromTokenConfig.name} ({fromTokenConfig.symbol})</div>
                <div>Output Token: {toTokenConfig.name} ({toTokenConfig.symbol})</div>
                {txHash && <div className="text-cyan-400">Hash: {txHash}</div>}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
export default DEX;
