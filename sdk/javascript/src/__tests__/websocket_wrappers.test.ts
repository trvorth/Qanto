import { QantoWebSocket } from '../websocket';

describe('QantoWebSocket wrapper subscription methods', () => {
  afterEach(() => {
    jest.restoreAllMocks();
  });

  test('subscribeToNetworkHealth, subscribeToAnalytics, subscribeToSecurityAlerts delegate to subscribe', async () => {
    const ws = new QantoWebSocket('ws://example', {} as any);
    const subscribeSpy = jest.spyOn(ws as any, 'subscribe').mockResolvedValue(undefined);

    await ws.subscribeToNetworkHealth();
    await ws.subscribeToAnalytics();
    await ws.subscribeToSecurityAlerts();

    const topics = subscribeSpy.mock.calls.map(c => c[0]);
    expect(topics).toEqual(expect.arrayContaining(['network_health', 'analytics', 'security_alerts']));
  });
});