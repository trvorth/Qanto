import { useEnsName, useEnsAvatar } from 'wagmi';
import { mainnet } from 'wagmi/chains';

interface AddressDisplayProps {
  address: string;
}

export function AddressDisplay({ address }: AddressDisplayProps) {
  // Ensure the address is a valid 0x string
  const cleanAddress = address?.startsWith('0x') ? (address as `0x${string}`) : undefined;

  const { data: ensName } = useEnsName({
    address: cleanAddress,
    chainId: mainnet.id,
  });

  const { data: ensAvatar } = useEnsAvatar({
    name: ensName || undefined,
    chainId: mainnet.id,
  });

  const truncateAddress = (addr: string) => {
    if (!addr) return '';
    if (addr.length <= 13) return addr;
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  };

  return (
    <div className="inline-flex items-center gap-2 font-mono bg-white/5 border border-white/10 rounded-full px-2.5 py-1 text-slate-300 hover:text-cyan-400 hover:border-cyan-500/30 hover:bg-cyan-500/5 transition-all duration-300 cursor-pointer select-none">
      {ensAvatar ? (
        <img
          src={ensAvatar}
          alt={ensName || address}
          className="w-5 h-5 rounded-full border border-cyan-500/30 object-cover"
        />
      ) : (
        <div className="w-5 h-5 rounded-full bg-gradient-to-tr from-cyan-500/40 to-purple-500/40 border border-white/10 flex items-center justify-center text-[8px] font-bold text-white uppercase">
          {ensName ? ensName.slice(0, 2) : address?.slice(2, 4) || '??'}
        </div>
      )}
      <span className="text-xs font-semibold">
        {ensName || truncateAddress(address)}
      </span>
    </div>
  );
}
