import { QantoClient } from '../client';
import { DEFAULT_ENDPOINTS, NETWORKS } from '../types';

describe('QantoClient configuration and headers', () => {
  test('getConfig resolves defaults for local network', () => {
    const client = new QantoClient({ network: 'local' });
    const cfg = client.getConfig();
    expect(cfg.network).toBe('local');
    expect(cfg.httpEndpoint).toBe(DEFAULT_ENDPOINTS[NETWORKS.LOCAL].http);
    expect(cfg.websocketEndpoint).toBe(DEFAULT_ENDPOINTS[NETWORKS.LOCAL].websocket);
    expect(cfg.graphqlEndpoint).toBe(DEFAULT_ENDPOINTS[NETWORKS.LOCAL].graphql);
    expect(cfg.userAgent).toMatch('@qanto/sdk');
  });

  test('updateConfig updates axios defaults and Authorization header toggling', () => {
    const client = new QantoClient({ network: 'local' });
    const anyClient = client as any;

    client.updateConfig({ httpEndpoint: 'https://example.com', timeout: 500, apiKey: 'abc' });
    expect(anyClient.httpClient.defaults.baseURL).toBe('https://example.com');
    expect(anyClient.httpClient.defaults.timeout).toBe(500);
    expect(anyClient.httpClient.defaults.headers['Authorization']).toBe('Bearer abc');

    // Clear API key should remove header
    client.updateConfig({ apiKey: '' });
    expect(anyClient.httpClient.defaults.headers['Authorization']).toBeUndefined();
  });
});