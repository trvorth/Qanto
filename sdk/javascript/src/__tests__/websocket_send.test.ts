import { QantoWebSocket } from '../websocket';
import WS from 'ws';
import { WebSocketError } from '../types';

describe('QantoWebSocket send behavior', () => {
  test('sends serialized data when connected', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    const fakeSocket: any = { readyState: WS.OPEN, send: jest.fn() };
    anyWs.ws = fakeSocket;

    const payload = { type: 'subscribe', subscription_type: 'blocks' };
    (ws as any).send(payload);
    expect(fakeSocket.send).toHaveBeenCalledTimes(1);
    const arg = (fakeSocket.send as jest.Mock).mock.calls[0][0];
    // Ensure payload is serialized
    expect(typeof arg).toBe('string');
    expect(JSON.parse(arg)).toEqual(payload);
  });

  test('throws WebSocketError when underlying send throws', () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const anyWs = ws as any;

    const fakeSocket: any = { readyState: WS.OPEN, send: jest.fn(() => { throw new Error('boom'); }) };
    anyWs.ws = fakeSocket;

    expect(() => (ws as any).send({ foo: 'bar' })).toThrow(WebSocketError);
    try {
      (ws as any).send({ foo: 'bar' });
    } catch (e) {
      const err = e as Error;
      expect(err.message).toContain('Failed to send message:');
    }
  });
});