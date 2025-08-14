// @ts-check
// Note: type annotations allow type checking and IDEs autocompletion

const lightCodeTheme = require('prism-react-renderer/themes/github');
const darkCodeTheme = require('prism-react-renderer/themes/dracula');

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Qanto Documentation',
  tagline: 'The Future of Decentralized Technology - Layer-0 Blockchain with AI Governance',
  favicon: 'img/favicon.ico',

  // Set the production url of your site here
  url: 'https://docs.qanto.org',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'qantoprotocol', // Usually your GitHub org/user name.
  projectName: 'qanto-docs', // Usually your repo name.

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  // Even if you don't use internalization, you can use this field to set useful
  // metadata like html lang. For example, if your site is Chinese, you may want
  // to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: '/', // Serve docs at the root
          sidebarPath: require.resolve('./sidebars.js'),
          // Please change this to your repo.
          // Remove this to remove the "edit this page" links.
          editUrl: 'https://github.com/trvworth/qanto/tree/main/qanto-docs/',
          versions: {
            current: {
              label: 'v2.0.0 🚀',
              path: '',
            },
          },
          lastVersion: 'current',
          showLastUpdateAuthor: true,
          showLastUpdateTime: true,
        },
        blog: {
          showReadingTime: true,
          blogTitle: 'Qanto Blog',
          blogDescription: 'Latest updates and insights from the Qanto ecosystem',
          blogSidebarCount: 10,
          blogSidebarTitle: 'Recent Posts',
          feedOptions: {
            type: 'all',
            title: 'Qanto Blog',
            description: 'Latest updates from Qanto Protocol',
            copyright: `Copyright © ${new Date().getFullYear()} Qanto Protocol.`,
          },
        },
        theme: {
          customCss: require.resolve('./src/css/custom.css'),
        },
        sitemap: {
          changefreq: 'weekly',
          priority: 0.8,
        },
        gtag: {
          trackingID: 'G-XXXXXXXXXX', // Replace with actual GA4 tracking ID
          anonymizeIP: true,
        },
      }),
    ],
  ],

  plugins: [
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'api',
        path: 'api',
        routeBasePath: 'api',
        sidebarPath: require.resolve('./sidebars-api.js'),
        editUrl: 'https://github.com/trvworth/qanto/tree/main/qanto-docs/',
      },
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      // Replace with your project's social card
      image: 'img/qanto-social-card.png',
      navbar: {
        title: 'Qanto',
        logo: {
          alt: 'Qanto Logo',
          src: 'img/logo.svg',
          srcDark: 'img/logo-dark.svg',
        },
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'tutorialSidebar',
            position: 'left',
            label: 'Documentation',
          },
          {
            to: '/api',
            label: 'API Reference',
            position: 'left',
          },
          {
            to: '/blog',
            label: 'Blog',
            position: 'left',
          },
          {
            to: '/tutorials',
            label: 'Tutorials',
            position: 'left',
          },
          {
            to: '/research',
            label: 'Research Papers',
            position: 'left',
          },
          {
            type: 'docsVersionDropdown',
            position: 'right',
            dropdownItemsAfter: [{to: '/versions', label: 'All versions'}],
            dropdownActiveClassDisabled: true,
          },
          {
            href: 'https://github.com/trvworth/qanto',
            label: 'GitHub',
            position: 'right',
          },
          {
            href: 'https://discord.gg/qanto',
            label: 'Discord',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Documentation',
            items: [
              {
                label: 'Getting Started',
                to: '/introduction/what-is-qanto',
              },
              {
                label: 'User Guides',
                to: '/user-guides',
              },
              {
                label: 'Developer Docs',
                to: '/developers',
              },
              {
                label: 'API Reference',
                to: '/api',
              },
            ],
          },
          {
            title: 'Community',
            items: [
              {
                label: 'Discord',
                href: 'https://discord.gg/qanto',
              },
              {
                label: 'Telegram',
                href: 'https://t.me/qantoprotocol',
              },
              {
                label: 'Twitter',
                href: 'https://twitter.com/QantoProtocol',
              },
              {
                label: 'Reddit',
                href: 'https://reddit.com/r/qanto',
              },
            ],
          },
          {
            title: 'Resources',
            items: [
              {
                label: 'GitHub',
                href: 'https://github.com/trvworth/qanto',
              },
              {
                label: 'Whitepaper',
                href: '/research/whitepaper',
              },
              {
                label: 'Blog',
                to: '/blog',
              },
              {
                label: 'Status Page',
                href: 'https://status.qanto.org',
              },
            ],
          },
          {
            title: 'Network',
            items: [
              {
                label: 'Mainnet Explorer',
                href: 'https://explorer.qanto.org',
              },
              {
                label: 'Testnet Explorer',
                href: 'https://testnet-explorer.qanto.org',
              },
              {
                label: 'Validator Stats',
                href: 'https://validators.qanto.org',
              },
              {
                label: 'Network Stats',
                href: 'https://stats.qanto.org',
              },
            ],
          },
        ],
        copyright: `Copyright © ${new Date().getFullYear()} Qanto Protocol. Built with Docusaurus.`,
      },
      prism: {
        theme: lightCodeTheme,
        darkTheme: darkCodeTheme,
        additionalLanguages: ['rust', 'toml', 'bash', 'json', 'yaml'],
      },
      algolia: {
        // The application ID provided by Algolia
        appId: 'ALGOLIA_APP_ID', // Replace with actual Algolia App ID
        
        // Public API key: it is safe to commit it
        apiKey: 'ALGOLIA_SEARCH_API_KEY', // Replace with actual API key
        
        indexName: 'qanto-docs',
        
        // Optional: see doc section below
        contextualSearch: true,
        
        // Optional: Specify domains where the navigation should occur through window.location
        externalUrlRegex: 'external\\.com|domain\\.com',
        
        // Optional: Algolia search parameters
        searchParameters: {},
        
        // Optional: path for search page that enabled by default (`false` to disable it)
        searchPagePath: 'search',
      },
      colorMode: {
        defaultMode: 'light',
        disableSwitch: false,
        respectPrefersColorScheme: true,
      },
      announcementBar: {
        id: 'mainnet-launch',
        content:
          '🚀 <strong>Qanto Mainnet is LIVE!</strong> Join the revolution in decentralized technology. <a target="_blank" rel="noopener noreferrer" href="https://qanto.org">Get Started</a>',
        backgroundColor: '#6366f1',
        textColor: '#ffffff',
        isCloseable: false,
      },
      metadata: [
        {name: 'keywords', content: 'blockchain, cryptocurrency, DAG, post-quantum, AI governance, Layer-0, DeFi'},
        {name: 'description', content: 'Comprehensive documentation for Qanto Protocol - the revolutionary Layer-0 blockchain with AI governance and post-quantum security.'},
      ],
    }),
};

module.exports = config;
