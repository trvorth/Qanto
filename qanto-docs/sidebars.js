/**
 * Creating a sidebar enables you to:
 - create an ordered group of docs
 - render a sidebar for each doc of that group
 - provide next/previous navigation

 The sidebars can be generated from the filesystem, or explicitly defined here.

 Create as many sidebars as you want.
 */

// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  // Main documentation sidebar
  tutorialSidebar: [
    {
      type: 'category',
      label: '🚀 Getting Started',
      collapsed: false,
      items: [
        'introduction/what-is-qanto',
        'introduction/key-features',
        'introduction/architecture-overview',
        'introduction/quick-start',
      ],
    },
    {
      type: 'category',
      label: '👤 User Guides',
      collapsed: false,
      items: [
        'user-guides/index',
        'user-guides/create-wallet',
        'user-guides/send-receive-tokens',
        'user-guides/staking-guide',
        'user-guides/join-testnet',
        'user-guides/security-best-practices',
      ],
    },
    {
      type: 'category',
      label: '🔧 Node Operations',
      collapsed: false,
      items: [
        'node-operators/index',
        'node-operators/system-requirements',
        'node-operators/setup-validator',
        'node-operators/configuration',
        'node-operators/monitoring',
        'node-operators/troubleshooting',
      ],
    },
    {
      type: 'category',
      label: '👨‍💻 Developer Documentation',
      collapsed: false,
      items: [
        'developers/index',
        'developers/quickstart',
        'developers/sdk-overview',
        {
          type: 'category',
          label: 'APIs',
          items: [
            'developers/api/rest-api',
            'developers/api/websocket-api',
            'developers/api/graphql-api',
          ],
        },
        {
          type: 'category',
          label: 'Smart Contracts',
          items: [
            'developers/smart-contracts/getting-started',
            'developers/smart-contracts/deployment',
            'developers/smart-contracts/examples',
          ],
        },
        {
          type: 'category',
          label: 'Integration Guides',
          items: [
            'developers/integrations/exchanges',
            'developers/integrations/wallets',
            'developers/integrations/payment-processors',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: '🤖 SAGA AI System',
      collapsed: false,
      items: [
        'saga/index',
        'saga/overview',
        'saga/governance-model',
        'saga/behavioral-analysis',
        'saga/predictive-modeling',
        'saga/credit-scoring',
        'saga/api-reference',
      ],
    },
    {
      type: 'category',
      label: '🔒 Security',
      collapsed: false,
      items: [
        'security/index',
        'security/post-quantum-crypto',
        'security/threat-model',
        'security/audit-reports',
        'security/responsible-disclosure',
      ],
    },
    {
      type: 'category',
      label: '💰 Economics',
      collapsed: false,
      items: [
        'economics/index',
        'economics/tokenomics',
        'economics/hame-model',
        'economics/fee-structure',
        'economics/inflation-control',
        'economics/validator-rewards',
      ],
    },
    {
      type: 'category',
      label: '🌐 Network',
      collapsed: false,
      items: [
        'network/index',
        'network/mainnet',
        'network/testnet',
        'network/consensus-mechanism',
        'network/dag-structure',
        'network/performance',
      ],
    },
    {
      type: 'category',
      label: '🔗 Ecosystem',
      collapsed: false,
      items: [
        'ecosystem/index',
        'ecosystem/wallets',
        'ecosystem/explorers',
        'ecosystem/exchanges',
        'ecosystem/grants-program',
        'ecosystem/partnerships',
      ],
    },
    {
      type: 'category',
      label: '❓ Support',
      collapsed: false,
      items: [
        'support/faq',
        'support/troubleshooting',
        'support/community',
        'support/contact',
      ],
    },
  ],

  // Tutorials sidebar
  tutorialsSidebar: [
    {
      type: 'category',
      label: '🎓 Beginner Tutorials',
      items: [
        'tutorials/beginner/wallet-setup',
        'tutorials/beginner/first-transaction',
        'tutorials/beginner/understanding-fees',
        'tutorials/beginner/backup-restore',
      ],
    },
    {
      type: 'category',
      label: '🚀 Advanced Tutorials',
      items: [
        'tutorials/advanced/running-validator',
        'tutorials/advanced/custom-rpc-setup',
        'tutorials/advanced/performance-tuning',
        'tutorials/advanced/multi-sig-setup',
      ],
    },
    {
      type: 'category',
      label: '👨‍💻 Developer Tutorials',
      items: [
        'tutorials/developer/first-dapp',
        'tutorials/developer/integration-example',
        'tutorials/developer/testing-guide',
        'tutorials/developer/deployment-automation',
      ],
    },
    {
      type: 'category',
      label: '🎬 Video Tutorials',
      items: [
        'tutorials/videos/getting-started',
        'tutorials/videos/wallet-management',
        'tutorials/videos/validator-setup',
        'tutorials/videos/developer-quickstart',
      ],
    },
  ],

  // Research papers sidebar
  researchSidebar: [
    {
      type: 'category',
      label: '📄 Core Papers',
      items: [
        'research/whitepaper',
        'research/technical-specification',
        'research/consensus-algorithm',
        'research/dag-architecture',
      ],
    },
    {
      type: 'category',
      label: '🤖 AI & Governance',
      items: [
        'research/saga-ai-system',
        'research/behavioral-analysis',
        'research/predictive-governance',
        'research/autonomous-parameters',
      ],
    },
    {
      type: 'category',
      label: '🔒 Security Research',
      items: [
        'research/post-quantum-security',
        'research/threat-analysis',
        'research/cryptographic-primitives',
        'research/security-proofs',
      ],
    },
    {
      type: 'category',
      label: '💰 Economic Models',
      items: [
        'research/hame-economics',
        'research/tokenomic-analysis',
        'research/game-theory',
        'research/market-dynamics',
      ],
    },
  ],
};

module.exports = sidebars;
