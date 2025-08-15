# Qanto Documentation Portal

[![Deploy Status](https://github.com/trvworth/qanto/workflows/Deploy%20Qanto%20Documentation/badge.svg)](https://github.com/trvworth/qanto/actions/workflows/deploy-docs.yml)
[![Website](https://img.shields.io/website?url=https%3A//docs.qanto.org)](https://docs.qanto.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Node Version](https://img.shields.io/badge/node-%3E%3D18.0-brightgreen)](https://nodejs.org/)

The comprehensive documentation portal for Qanto Protocol - the revolutionary Layer-0 blockchain with AI governance and post-quantum security.

🌐 **Live Site**: [docs.qanto.org](https://docs.qanto.org)

## 📚 What's Included

### Core Documentation
- **Getting Started** - Introduction to Qanto and quick setup guides
- **User Guides** - Wallet setup, transactions, staking, and security
- **Node Operations** - Running validators and network participation
- **Developer Docs** - SDKs, APIs, smart contracts, and integration guides
- **SAGA AI System** - AI governance, behavioral analysis, and predictive modeling
- **Security** - Post-quantum cryptography and threat modeling
- **Economics** - Tokenomics, HAME model, and validator rewards
- **Network** - Consensus mechanism, DAG architecture, and performance
- **Ecosystem** - Wallets, exchanges, grants program, and partnerships

### Interactive Features
- **API Reference** - OpenAPI/Swagger specs with live testing
- **Tutorial Series** - Step-by-step guides with downloadable resources
- **Video Content** - Embedded YouTube tutorials and learning paths
- **Research Papers** - Whitepapers and academic publications
- **Search** - Algolia DocSearch integration for instant queries

### Technical Infrastructure
- **Multi-language Support** - English, Chinese, Japanese, Korean, Spanish, German
- **Responsive Design** - Mobile-first approach with progressive enhancement
- **Performance Optimized** - SSG with CDN delivery and aggressive caching
- **SEO Optimized** - Structured data, meta tags, and sitemap generation
- **Accessibility** - WCAG 2.1 AA compliance with screen reader support

## 🚀 Getting Started

### Prerequisites

- **Node.js** 18.0 or higher
- **npm** 9.0 or higher
- **Git** for version control

### Installation

1. **Clone the repository**:
   ```bash
   git clone https://github.com/trvworth/qanto.git
   cd qanto/qanto-docs
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Start development server**:
   ```bash
   npm start
   ```

4. **Open in browser**:
   - Local: `http://localhost:3000`
   - Network: `http://[your-ip]:3000`

The site will automatically reload when you make changes.

### Development Commands

```bash
# Start development server
npm start

# Build for production
npm run build

# Serve production build locally
npm run serve

# Run linting
npm run lint

# Format code
npm run format

# Type checking
npm run typecheck

# Run tests
npm run test
```

## 🏗️ Project Structure

```
qanto-docs/
├── docs/                    # Documentation content
│   ├── introduction/        # Getting started guides
│   ├── user-guides/         # End-user documentation
│   ├── developers/          # Developer resources
│   ├── node-operators/      # Validator guides
│   ├── saga/               # SAGA AI documentation
│   ├── security/           # Security and cryptography
│   ├── economics/          # Economic model details
│   ├── network/            # Network architecture
│   ├── ecosystem/          # Ecosystem overview
│   ├── tutorials/          # Step-by-step tutorials
│   ├── research/           # Research papers
│   └── support/            # FAQ and troubleshooting
├── api/                    # API documentation
│   ├── rest/              # REST API endpoints
│   ├── websocket/         # WebSocket API
│   ├── graphql/           # GraphQL API
│   ├── saga/              # SAGA AI API
│   └── tools/             # Developer tools
├── blog/                  # Blog posts and updates
├── src/                   # Custom components and pages
│   ├── components/        # React components
│   ├── css/              # Global styles
│   └── pages/            # Custom pages
├── static/               # Static assets
│   ├── img/              # Images and logos
│   ├── pdfs/             # Research papers and guides
│   └── resources/        # Tutorial resources
├── docusaurus.config.js  # Main configuration
├── sidebars.js          # Documentation sidebars
├── sidebars-api.js      # API reference sidebars
└── package.json         # Dependencies and scripts
```

## ✨ Features

### Documentation Features
- **Versioned Documentation** - Support for multiple versions with automated migration
- **Multi-language Support** - Full i18n with community translations
- **Interactive Code Examples** - Live code blocks with syntax highlighting
- **Mermaid Diagrams** - Architecture and flow diagrams
- **Math Expressions** - LaTeX support for technical formulas
- **Admonitions** - Callouts, warnings, and tips
- **Tabs and Code Groups** - Organized code examples
- **Breadcrumbs** - Clear navigation hierarchy
- **Table of Contents** - Auto-generated from headings
- **Last Updated** - Git-based modification dates
- **Edit This Page** - Direct GitHub editing links

### API Documentation
- **OpenAPI Integration** - Live API testing and exploration
- **Interactive Swagger UI** - Try endpoints directly from docs
- **Code Generation** - Auto-generated client libraries
- **Authentication Examples** - Working code samples
- **Rate Limiting Info** - Clear usage guidelines
- **Error Handling** - Comprehensive error documentation

### Developer Experience
- **Hot Reloading** - Instant preview of changes
- **Broken Link Detection** - Catch issues during build
- **HTML Validation** - Ensure markup quality
- **Performance Monitoring** - Lighthouse CI integration
- **Accessibility Testing** - Automated a11y audits
- **SEO Analysis** - Meta tag and structure validation

## 🌍 Internationalization

The documentation supports multiple languages:

| Language | Code | Status | Translator |
|----------|------|---------|------------|
| English | `en` | ✅ Complete | Core team |
| Chinese (Simplified) | `zh-Hans` | ✅ Complete | Community |
| Japanese | `ja` | 🚧 In Progress | Community |
| Korean | `ko` | 📋 Planned | Community |
| Spanish | `es` | 📋 Planned | Community |
| German | `de` | 📋 Planned | Community |

### Contributing Translations

1. **Join our translation team** on [Crowdin](https://crowdin.com/project/qanto-docs)
2. **Review existing translations** and suggest improvements
3. **Add new language support** by creating locale files
4. **Test translations** in the development environment

## 🎨 Customization

### Theme Customization

The documentation uses a custom theme based on Docusaurus with Qanto branding:

- **Colors**: Custom CSS variables for Qanto purple/blue gradient
- **Typography**: Inter font family for modern readability
- **Components**: Custom React components for enhanced functionality
- **Layout**: Responsive grid system with mobile-first approach

### Adding Content

#### New Documentation Page

1. Create a new `.md` file in the appropriate `docs/` subdirectory
2. Add frontmatter with metadata:
   ```yaml
   ---
   id: unique-page-id
   title: Page Title
   sidebar_label: Short Label
   description: Page description for SEO
   tags: [tag1, tag2, tag3]
   ---
   ```
3. Write your content using Markdown and MDX
4. Update the sidebar configuration in `sidebars.js`

#### New API Documentation

1. Create the endpoint documentation in the `api/` directory
2. Follow the OpenAPI specification format
3. Include examples and error responses
4. Update `sidebars-api.js` for navigation

#### Blog Posts

1. Add a new file to the `blog/` directory with date prefix:
   ```
   2025-01-15-new-feature-announcement.md
   ```
2. Include author information and tags
3. Use featured images for social sharing

## 🚀 Deployment

### Automated Deployment

The documentation is automatically deployed using GitHub Actions:

- **Staging**: Every pull request gets a preview deployment
- **Production**: Main branch pushes deploy to `docs.qanto.org`
- **Performance**: Lighthouse audits run on every production deployment
- **Security**: Automated security scans and SSL certificate monitoring

### Manual Deployment

For emergency deployments or testing:

```bash
# Build the site
npm run build

# Deploy to NameCheap hosting
# Build artifacts are ready for manual upload to NameCheap hosting panel
# or automated deployment via NameCheap's hosting APIs

## Infrastructure

The documentation is hosted on NameCheap with the following setup:

- **Static Hosting**: NameCheap's static site hosting service
- **CDN**: NameCheap's integrated CDN for global distribution
- **Route 53**: DNS management with health checks
- **Certificate Manager**: SSL/TLS certificates
- **Lambda**: Search indexing and form processing

## 📊 Analytics and Monitoring

### Performance Monitoring
- **Core Web Vitals** tracking with Google Analytics
- **Lighthouse CI** for performance regression detection
- **Real User Monitoring** with CloudWatch synthetics
- **Error Tracking** with centralized logging

### User Analytics
- **Google Analytics 4** for user behavior insights
- **Hotjar** for user experience optimization
- **Search Analytics** via Algolia dashboard
- **A/B Testing** for content optimization

## 🤝 Contributing

We welcome contributions to improve the documentation! Here's how to get involved:

### Types of Contributions

- **Content Updates** - Fix typos, improve explanations, add examples
- **New Documentation** - Create guides for new features
- **Translations** - Help translate content to new languages
- **Bug Reports** - Report broken links, rendering issues, or errors
- **Feature Requests** - Suggest new functionality or improvements

### Contribution Process

1. **Fork the repository** and create a feature branch
2. **Make your changes** following our style guide
3. **Test locally** to ensure everything works
4. **Submit a pull request** with a clear description
5. **Address feedback** from maintainers
6. **Celebrate** when your contribution is merged! 🎉

### Style Guide

- **Use clear, concise language** suitable for international audiences
- **Include code examples** for technical concepts
- **Add screenshots** for UI-based instructions
- **Follow existing patterns** for consistency
- **Test all links** and ensure accuracy
- **Optimize images** for web delivery

### Getting Help

- **Discord**: [discord.gg/qanto-docs](https://discord.gg/qanto-docs)
- **GitHub Discussions**: [Repository Discussions](https://github.com/trvworth/qanto/discussions)
- **Email**: [docs@qanto.org](mailto:docs@qanto.org)

## 📄 License

This documentation is licensed under the [MIT License](LICENSE).

The Qanto Protocol itself is licensed under [Apache 2.0](../LICENSE).

## 🙏 Acknowledgments

- **Docusaurus Team** for the amazing documentation platform
- **Community Contributors** for translations and content improvements
- **Design System** inspired by modern documentation practices
- **Academic Partners** for research paper contributions

---

**Made with ❤️ by the Qanto Protocol Team**

Last updated: January 2025
