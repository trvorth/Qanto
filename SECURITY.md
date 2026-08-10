# Security Policy

## Overview

Qanto is committed to ensuring the long-term security of its heterogeneous, post-quantum Layer-0 protocol. As the project is currently in Phase 1 - Foundation (In Progress), no official releases have been published, and thus no versions are currently supported with security updates. This policy outlines our approach to security during development and provides guidance for reporting vulnerabilities as we progress toward staged public testnet launches.

## Supported Versions

As Qanto is in pre-release development, no versions are currently supported with security updates. Once the initial testnet is launched, we will publish a versioning scheme and designate supported versions in this section. Security updates will be provided for all active testnet releases, with a deprecation process outlined prior to end-of-life for any version.

## Reporting a Vulnerability

### Reporting Process
Qanto welcomes responsible disclosure of security vulnerabilities to strengthen the protocol. To report a vulnerability, please follow these steps:

1. **Contact the Security Team Privately**: Submit a [private vulnerability report](https://github.com/trvorth/qanto/security/advisories/new). Do not open a public issue. If GitHub reports that private vulnerability reporting is unavailable, contact the repository owners through the project channels without including exploit details and request a private channel.
2. **Provide Details**: Ensure your report includes sufficient technical detail to allow our team to assess the issue.
3. **Confidentiality**: Do not disclose the vulnerability publicly until it has been addressed. We will treat all reports with strict confidentiality.

### Response Timeline
- **Acknowledgment target**: We aim to confirm receipt within 48 hours.
- **Assessment target**: We aim to provide an initial severity assessment within 7 calendar days.
- **Updates**: For accepted reports, we aim to provide an update at least every 7 calendar days until remediation or coordinated disclosure.
- **Resolution**: Once validated, vulnerabilities will be patched in the next development milestone, with a timeline communicated to the reporter.

### Outcome Expectations
- **Accepted Vulnerabilities**: If a report is accepted, we will prioritize a fix, integrate it into the codebase, and schedule deployment for the next testnet release. Reporters may be acknowledged (with consent) in the release notes.
- **Declined Reports**: If a vulnerability is deemed out of scope, non-exploitable, or invalid, we will provide a detailed explanation within 14 days of assessment.
- **Coordination**: For critical issues (e.g., those affecting post-quantum cryptographic integrity), we may request additional collaboration with the reporter and third-party auditors.

### Rewards Program
No formal bug bounty or guaranteed monetary reward is active during Phase 1.

## Security Development Practices

### Current Status
As a pre-release project, Qanto is actively developing its security infrastructure. Cryptographic, consensus, bridge, and operational claims remain subject to the launch gates documented under `Docs/audits/`; the presence of an implementation is not a claim of independent certification.

### Future Audits
An independent third-party audit and remediation verification are required before mainnet. Internal engineering reviews are stored under `Docs/audits/` and do not substitute for that independent review.

### Community Involvement
We encourage the community to contribute to security through code reviews and vulnerability reports. Please refer to our Contribution Guidelines (CONTRIBUTING.md) for participation details. All contributors are expected to adhere to our Code of Conduct.

## Legal and Ethical Considerations

We ask researchers to act in good faith, avoid privacy violations and service disruption, and allow reasonable time for coordinated remediation. The repository license does not itself define a vulnerability-disclosure safe harbor.

## Contact

For non-sensitive security questions, use [Qanto GitHub Discussions](https://github.com/trvorth/qanto/discussions). Vulnerability details must use the private reporting process above.
