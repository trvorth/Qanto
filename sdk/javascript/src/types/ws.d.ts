declare module 'ws' {
  // Minimal typings to satisfy TypeScript for SDK usage
  export type Data = any;

  export default class WebSocket {
    static CONNECTING: number;
    static OPEN: number;
    static CLOSING: number;
    static CLOSED: number;

    readyState: number;

    constructor(url: string, protocols?: string | string[], options?: any);

    on(event: 'open', listener: () => void): this;
    on(event: 'message', listener: (data: Data) => void): this;
    on(event: 'close', listener: (code: number, reason: string) => void): this;
    on(event: 'error', listener: (err: Error) => void): this;

    send(data: any, options?: any, cb?: (err?: Error) => void): void;
    close(code?: number, reason?: string): void;
    terminate(): void;
    ping(data?: any, mask?: boolean, cb?: (err?: Error) => void): void;
  }
}