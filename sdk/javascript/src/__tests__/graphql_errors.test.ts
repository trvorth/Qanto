import { QantoGraphQL } from '../graphql';
import { GraphQLError } from '../types';

describe('QantoGraphQL error handling', () => {
  test('getBalance throws GraphQLError on request failure', async () => {
    const gql = new QantoGraphQL('http://example/graphql');
    (gql as any).client.request = jest.fn().mockRejectedValue(new Error('network down'));

    await expect(gql.getBalance('QANTO_ADDR_TEST')).rejects.toBeInstanceOf(GraphQLError);
    await expect(gql.getBalance('QANTO_ADDR_TEST')).rejects.toThrow('Failed to get balance: network down');
  });

  test('query throws GraphQLError on request failure', async () => {
    const gql = new QantoGraphQL('http://example/graphql');
    (gql as any).client.request = jest.fn().mockRejectedValue(new Error('bad query'));

    await expect(gql.query('{ test }')).rejects.toBeInstanceOf(GraphQLError);
    await expect(gql.query('{ test }')).rejects.toThrow('GraphQL query failed: bad query');
  });
});