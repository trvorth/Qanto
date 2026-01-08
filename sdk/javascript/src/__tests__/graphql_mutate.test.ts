import { QantoGraphQL } from '../graphql';
import { GraphQLError } from '../types';

describe('QantoGraphQL mutate and submitTransaction', () => {
  test('mutate throws GraphQLError on request failure', async () => {
    const graphql = new QantoGraphQL('http://localhost/graphql');
    const anyGql = graphql as any;
    anyGql.client.request = jest.fn().mockRejectedValue(new Error('bad mutation'));
    await expect(graphql.mutate('mutation { foo }')).rejects.toBeInstanceOf(GraphQLError);
  });

  test('mutate returns result on success', async () => {
    const graphql = new QantoGraphQL('http://localhost/graphql');
    const anyGql = graphql as any;
    anyGql.client.request = jest.fn().mockResolvedValue({ ok: true });
    const res = await graphql.mutate('mutation { bar }');
    expect(res).toEqual({ ok: true });
  });

  test('submitTransaction returns typed fields on success', async () => {
    const graphql = new QantoGraphQL('http://localhost/graphql');
    const anyGql = graphql as any;
    anyGql.client.request = jest.fn().mockResolvedValue({ submitTransaction: { hash: 'h', success: true } });
    const res = await graphql.submitTransaction('deadbeef');
    expect(res).toEqual({ hash: 'h', success: true });
  });
});