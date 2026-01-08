import defaultClient, { VERSION, DEFAULT_ENDPOINTS, NETWORKS, QantoClient } from '../index';

describe('index exports', () => {
  test('VERSION and DEFAULT_ENDPOINTS are exported and have expected values', () => {
    expect(VERSION).toBe('1.0.0');
    expect(DEFAULT_ENDPOINTS[NETWORKS.LOCAL].http).toBe('p2p://discovery');
  });

  test('default export is QantoClient class', () => {
    expect(defaultClient).toBeDefined();
    // The default export should be the QantoClient constructor
    expect(defaultClient).toBe(QantoClient);
  });
});