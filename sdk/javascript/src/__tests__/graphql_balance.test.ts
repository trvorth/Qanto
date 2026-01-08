import { QantoGraphQL } from '../graphql';

describe('QantoGraphQL.getBalance', () => {
  test('returns typed balance fields', async () => {
    const gql = new QantoGraphQL('http://example/graphql');
    // Mock the internal GraphQL client request
    (gql as any).client.request = jest.fn().mockResolvedValue({
      balance: { confirmed: 42, unconfirmed: 8, total: 50 }
    });

    const res = await gql.getBalance('QANTO_ADDR_TEST');
    expect(res).toEqual({ confirmed: 42, unconfirmed: 8, total: 50 });
  });
});