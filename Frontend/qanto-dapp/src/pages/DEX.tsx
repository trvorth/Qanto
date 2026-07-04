import { useState, useEffect, useRef } from 'react';
import { toast } from 'react-hot-toast';
import {
  useAccount,
  useConnectModal,
  useWaitForTransactionReceipt,
  useWriteContract,
} from '../lib/qanto-wallet';

const DEX_CONTRACT_ADDRESS = '0x9F00000000000000000000000000000000000012';

const QantoDexAbi = [
  {
    type: 'function',
    name: 'swap',
    stateMutability: 'payable',
    inputs: [
      { name: 'tokenIn', type: 'address' },
      { name: 'tokenOut', type: 'address' },
      { name: 'amountIn', type: 'uint256' },
      { name: 'minAmountOut', type: 'uint256' }
    ],
    outputs: []
  }
] as const;

export const DEX = () => {
  const { isConnected, address } = useAccount();
  const { openConnectModal } = useConnectModal();

  const [fromToken, setFromToken] = useState('ETH');
  const [toToken, setToToken] = useState('QNTO');
  const [fromAmount, setFromAmount] = useState('0.1');
  const [toAmount, setToAmount] = useState('1000');

  const { writeContract, data: txHash, isPending: isWritePending } = useWriteContract({
    mutation: {
      onError: (error: any) => {
        if (toastId.current) {
          toast.dismiss(toastId.current);
          toastId.current = null;
        }
        if (error.message?.includes('User rejected') || error.code === 4001) {
          toast.error('Transaction cancelled by user.');
        } else if (error.message?.includes('insufficient funds')) {
          toast.error('Insufficient QNTO balance for execution.');
        } else {
          toast.error('Blockchain transaction failed.');
          console.error('Wallet transaction error:', error);
        }
      }
    }
  });
  const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
    hash: txHash,
  });

  const toastId = useRef<string | null>(null);

  // Exchange rate: 1 ETH = 10,000 QNTO
  const rate = 10000;

  // Handle reciprocal calculation
  const handleFromAmountChange = (val: string) => {
    setFromAmount(val);
    if (!val || isNaN(parseFloat(val))) {
      setToAmount('');
      return;
    }
    const parsed = parseFloat(val);
    if (fromToken === 'ETH') {
      setToAmount((parsed * rate).toFixed(2));
    } else {
      setToAmount((parsed / rate).toFixed(6));
    }
  };

  const handleToAmountChange = (val: string) => {
    setToAmount(val);
    if (!val || isNaN(parseFloat(val))) {
      setFromAmount('');
      return;
    }
    const parsed = parseFloat(val);
    if (fromToken === 'ETH') {
      setFromAmount((parsed / rate).toFixed(6));
    } else {
      setFromAmount((parsed * rate).toFixed(2));
    }
  };

  const switchTokens = () => {
    setFromToken(toToken);
    setToToken(fromToken);
    setFromAmount(toAmount);
    setToAmount(fromAmount);
  };

  const handleSwap = () => {
    if (!isConnected) {
      if (openConnectModal) {
        openConnectModal();
      } else {
        toast.error('Connect wallet to perform swap.');
      }
      return;
    }

    if (!fromAmount || parseFloat(fromAmount) <= 0 || isNaN(parseFloat(fromAmount))) {
      toast.error('Enter a valid amount to swap.');
      return;
    }

    toastId.current = toast.loading('Swapping tokens...');

    // Addresses for mock tokens
    const ethSyntheticAddr = '0x0000000000000000000000000000000000000000';
    const qntoSyntheticAddr = '0x9F00000000000000000000000000000000000001';

    const tokenIn = fromToken === 'ETH' ? ethSyntheticAddr : qntoSyntheticAddr;
    const tokenOut = toToken === 'ETH' ? ethSyntheticAddr : qntoSyntheticAddr;
    
    // Convert to 18 decimals for simulation
    const amountIn = BigInt(Math.floor(parseFloat(fromAmount) * 10 ** 18));
    const minAmountOut = BigInt(Math.floor(parseFloat(toAmount) * 0.95 * 10 ** 18)); // 5% slippage

    writeContract({
      abi: QantoDexAbi,
      address: DEX_CONTRACT_ADDRESS,
      functionName: 'swap',
      args: [tokenIn, tokenOut, amountIn, minAmountOut],
      value: fromToken === 'ETH' ? amountIn : undefined
    } as any);
  };

  useEffect(() => {
    if (isConfirmed) {
      if (toastId.current) {
        toast.dismiss(toastId.current);
        toastId.current = null;
      }
      toast.success('Swap Completed successfully!');
    }
  }, [isConfirmed]);



  const isPending = isWritePending || isConfirming;
  const isSwapSuccess = isConfirmed;

  let swapButtonText = 'SWAP TOKENS';
  if (!isConnected) {
    swapButtonText = 'Connect Wallet';
  } else if (isPending) {
    swapButtonText = 'Swapping...';
  } else if (isSwapSuccess) {
    swapButtonText = 'Swap Successful ✅';
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
            Instantly swap local testnet assets with zero gas slippage.
          </p>

          {/* Swap Box */}
          <div className="space-y-4 mb-6">
            {/* Input Box 1 */}
            <div className="bg-black/40 border border-white/10 rounded-2xl p-4 flex flex-col gap-2">
              <div className="flex justify-between text-xs text-slate-500 font-mono">
                <span>Pay</span>
                <span>Balance: {isConnected ? (fromToken === 'ETH' ? '2.41 ETH' : '5,000 QNTO') : '0.00'}</span>
              </div>
              <div className="flex justify-between items-center gap-4">
                <input
                  type="number"
                  value={fromAmount}
                  onChange={(e) => handleFromAmountChange(e.target.value)}
                  disabled={isPending || isSwapSuccess}
                  className="bg-transparent text-2xl font-bold font-mono text-white outline-none border-none w-full p-0 focus:ring-0"
                  placeholder="0.0"
                />
                <span className="bg-white/5 border border-white/10 text-white font-bold font-mono px-3.5 py-1.5 rounded-xl text-sm select-none">
                  {fromToken}
                </span>
              </div>
            </div>

            {/* Switch Button */}
            <div className="flex justify-center -my-3 relative z-20">
              <button
                onClick={switchTokens}
                disabled={isPending || isSwapSuccess}
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
                <span>Receive (Estimated)</span>
                <span>Balance: {isConnected ? (toToken === 'ETH' ? '2.41 ETH' : '5,000 QNTO') : '0.00'}</span>
              </div>
              <div className="flex justify-between items-center gap-4">
                <input
                  type="number"
                  value={toAmount}
                  onChange={(e) => handleToAmountChange(e.target.value)}
                  disabled={isPending || isSwapSuccess}
                  className="bg-transparent text-2xl font-bold font-mono text-white outline-none border-none w-full p-0 focus:ring-0"
                  placeholder="0.0"
                />
                <span className="bg-white/5 border border-white/10 text-white font-bold font-mono px-3.5 py-1.5 rounded-xl text-sm select-none">
                  {toToken}
                </span>
              </div>
            </div>
          </div>

          {/* Details */}
          <div className="bg-white/[0.02] border border-white/5 rounded-xl p-4 mb-8 text-xs font-mono text-slate-400 space-y-2">
            <div className="flex justify-between">
              <span>Rate:</span>
              <span>1 ETH = {rate.toLocaleString()} QNTO</span>
            </div>
            <div className="flex justify-between text-sm mt-4">
              <span className="text-slate-400">Network Fee</span>
              <span className="text-emerald-400 font-bold drop-shadow-[0_0_8px_rgba(52,211,153,0.8)] animate-pulse">Sponsored by SAGA 🧠</span>
            </div>
            <div className="flex justify-between">
              <span>Price Slippage:</span>
              <span>&lt; 0.05%</span>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <button
              onClick={handleSwap}
              disabled={isPending || (isConnected && isSwapSuccess)}
              className={`w-full py-4 px-6 rounded-xl font-bold font-sans text-sm md:text-base border transition-all duration-300 ${
                !isConnected
                  ? 'bg-cyan-500 hover:bg-cyan-400 text-white shadow-[0_0_20px_rgba(6,182,212,0.4)] border-transparent hover:scale-[1.02]'
                  : isPending
                  ? 'bg-slate-800 border-transparent text-slate-500 cursor-not-allowed opacity-70'
                  : isSwapSuccess
                  ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400 cursor-default'
                  : 'bg-cyan-500 hover:bg-cyan-400 text-white hover:shadow-[0_0_20px_rgba(6,182,212,0.6)] border-transparent hover:scale-[1.02]'
              }`}
            >
              {swapButtonText}
            </button>

            {isConnected && (
              <div className="flex flex-col gap-2 mt-4 text-[10px] font-mono text-slate-500 break-all text-center">
                <div>Connected User: {address}</div>
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
