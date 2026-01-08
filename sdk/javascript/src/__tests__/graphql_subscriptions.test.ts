import { QantoGraphQL } from '../graphql';

describe('QantoGraphQL subscriptions', () => {
  let graphql: QantoGraphQL;
  let warnSpy: jest.SpyInstance;

  beforeEach(() => {
    graphql = new QantoGraphQL('http://localhost/graphql');
    warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    warnSpy.mockRestore();
  });

  test('subscribeToBlocks registers subscription and returns id', async () => {
    const id = await graphql.subscribeToBlocks();
    expect(typeof id).toBe('string');
    expect(graphql.getActiveSubscriptions()).toContain(id);
    expect(warnSpy).toHaveBeenCalled();
    await graphql.unsubscribe(id);
    expect(graphql.getActiveSubscriptions()).not.toContain(id);
  });

  test('subscribeToTransactions registers subscription and returns id', async () => {
    const id = await graphql.subscribeToTransactions();
    expect(typeof id).toBe('string');
    expect(graphql.getActiveSubscriptions()).toContain(id);
    expect(warnSpy).toHaveBeenCalled();
    await graphql.unsubscribe(id);
    expect(graphql.getActiveSubscriptions()).not.toContain(id);
  });
});