declare module 'lucide-react' {
  import type { SVGProps } from 'react';

  export type LucideIcon = (props: SVGProps<SVGSVGElement>) => JSX.Element;

  export const Menu: LucideIcon;
  export const X: LucideIcon;
  export const Lock: LucideIcon;
  export const Layers: LucideIcon;
  export const Network: LucideIcon;
  export const Brain: LucideIcon;
  export const EyeOff: LucideIcon;
  export const Zap: LucideIcon;
}
