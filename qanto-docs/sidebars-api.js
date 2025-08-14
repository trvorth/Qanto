/**
 * API Reference sidebar configuration
 */

// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebarsApi = {
  apiSidebar: [
    {
      type: 'category',
      label: '📖 API Overview',
      collapsed: false,
      items: [
        'api/introduction',
        'api/authentication',
        'api/rate-limiting',
        'api/error-handling',
        'api/versioning',
      ],
    },
    {
      type: 'category',
      label: '🔗 REST API',
      collapsed: false,
      items: [
        'api/rest/overview',
        'api/rest/accounts',
        'api/rest/transactions',
        'api/rest/blocks',
        'api/rest/validators',
        'api/rest/network',
        'api/rest/governance',
      ],
    },
    {
      type: 'category',
      label: '⚡ WebSocket API',
      collapsed: false,
      items: [
        'api/websocket/overview',
        'api/websocket/connection',
        'api/websocket/subscriptions',
        'api/websocket/events',
        'api/websocket/examples',
      ],
    },
    {
      type: 'category',
      label: '🔍 GraphQL API',
      collapsed: false,
      items: [
        'api/graphql/overview',
        'api/graphql/schema',
        'api/graphql/queries',
        'api/graphql/mutations',
        'api/graphql/subscriptions',
        'api/graphql/playground',
      ],
    },
    {
      type: 'category',
      label: '🤖 SAGA AI API',
      collapsed: false,
      items: [
        'api/saga/overview',
        'api/saga/governance',
        'api/saga/analytics',
        'api/saga/predictions',
        'api/saga/credit-score',
        'api/saga/behavioral-data',
      ],
    },
    {
      type: 'category',
      label: '🛠️ Developer Tools',
      collapsed: false,
      items: [
        'api/tools/postman-collection',
        'api/tools/openapi-spec',
        'api/tools/sdk-reference',
        'api/tools/testing-endpoints',
      ],
    },
    {
      type: 'category',
      label: '📊 Data Models',
      collapsed: false,
      items: [
        'api/models/transaction',
        'api/models/block',
        'api/models/account',
        'api/models/validator',
        'api/models/governance',
      ],
    },
    {
      type: 'category',
      label: '🔧 Integration Examples',
      collapsed: false,
      items: [
        'api/examples/wallet-integration',
        'api/examples/exchange-integration',
        'api/examples/explorer-queries',
        'api/examples/monitoring-setup',
      ],
    },
  ],
};

module.exports = sidebarsApi;
