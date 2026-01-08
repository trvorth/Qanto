import { QantoWebSocket } from '../websocket';
import { WebSocketError } from '../types';

describe('QantoWebSocket error handling', () => {
  test('emits error for server error message', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);

    await new Promise<void>((resolve) => {
      ws.on('error', (err: Error) => {
        expect(err).toBeInstanceOf(WebSocketError);
        expect(err.message).toContain('Unknown WebSocket error');
        resolve();
      });

      (ws as any).handleMessage({ type: 'error', data: { message: undefined }, timestamp: Date.now() });
    });
  });

  test('warns on unknown message type and still emits message event', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => {});

    const messagePayload = { type: 'unknown_type', data: { foo: 'bar' }, timestamp: Date.now() };
    const onMessage = jest.fn();
    ws.on('message', onMessage);
    (ws as any).handleMessage(messagePayload);

    expect(warnSpy).toHaveBeenCalled();
    expect(onMessage).toHaveBeenCalledWith(messagePayload);

    warnSpy.mockRestore();
  });

  test('send throws when not connected', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    expect(() => (ws as any).send({ type: 'subscribe', subscription_type: 'blocks' })).toThrow(WebSocketError);
  });
});