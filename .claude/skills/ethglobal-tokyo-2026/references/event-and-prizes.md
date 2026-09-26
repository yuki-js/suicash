# ETHGlobal Tokyo 2026 event and prize reference

Snapshot captured **2026-09-25**. ETHGlobal pages are dynamic. Verify the live page before making a submission or prize-eligibility decision:

- Event info: <https://ethglobal.com/events/tokyo2026/info>
- Start guide: <https://ethglobal.com/events/tokyo2026/info/start>
- Details and rules: <https://ethglobal.com/events/tokyo2026/info/details>
- Prize board: <https://ethglobal.com/events/tokyo2026/prizes>
- Resources: <https://ethglobal.com/events/tokyo2026/info/resources>
- Dashboard: <https://ethglobal.com/events/tokyo2026/home>

## Submission and judging snapshot

- Deadline: **Sunday, September 27, 2026 at 09:00 JST**; late submissions are rejected.
- Teams: up to five people; each accepted participant stakes individually.
- Submission fields include title, description, and repository link.
- Select up to **three Partner Prizes** in the final submission step.
- A partner with multiple tracks may count as one Partner Prize.
- Optional video: 2–4 minutes, at least 720p. Do not use mobile-phone recording, text-to-speech/AI voiceover, or footage sped up to fit the limit.
- Finalist judging: 4-minute presentation plus 3-minute Q&A.
- Judging categories: Technicality, Originality, Practicality, Usability (UI/UX/DX), WOW Factor.

## Track rules

### Classic — From Scratch

Project-specific code, design, and assets must begin after the hackathon starts. Public libraries, starter kits, and boilerplates are allowed, but reuse should be documented. Projects built before the event may not qualify for partner prizes or Finalist judging under the published guidance.

### Continuity — Extend Open Source / Ship a Feature

Existing code may be extended when permitted by the selected track. Document the baseline, the new feature, and the hackathon commits. Partner eligibility can differ, so read the individual prize entry.

## Published prize snapshot

The prize board showed seven partner organizations. The amounts and qualification details below are a navigation aid, not a promise of eligibility.

### World — $15,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/world>

- **Best Use of IDKit — $5,000:** use IDKit and a supported World ID credential for a meaningful trust moment. Verify server-side or onchain as appropriate. Demonstrate a successful verification and an alternative path such as cancellation, rejection, or unavailable credential. Include integration feedback.
- **Best Use of World ID for Agents — $5,000:** use the event's World ID for Agents development environment. Show request, human completion, validated result, and protected action, plus a denied/expired/cancelled path. Keep secrets server-side.
- **Continuity variants:** the page lists Continuity-only IDKit and World ID for Agents categories, each shown at $2,500 in the snapshot.

Key docs: <https://docs.world.org/world-id/idkit/integrate>, <https://docs.world.org/world-id/credentials/1>, <https://sandbox.auth.world.org/docs>.

### 1inch — $7,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/1inch>

- **Build an Aqua App — $5,000:** create a custom Aqua app implementing a sophisticated DeFi position. Official Aqua/SwapVM contracts are required. Demonstrate onchain token transfers, with local forks allowed, and show tests or a UI. SwapVM use is favored.
- **Continuity variant — $2,000:** same core requirements for a Continuity project.

Key resources: <https://github.com/1inch/swap-vm>, <https://github.com/1inch/aqua>, <https://github.com/1inch/sdks/tree/master/typescript/aqua>.

### ENS — $10,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/ens>

- **Best Use of ENSv2 — $6,000:** build centrally on ENSv2 on Sepolia. Explore hierarchical registries, wildcard resolution, subnames, Enhanced Access Control, Permissioned Resolvers, aliasing, or agent namespaces. The demo must be functional, open source, and linked from the showcase.
- **Continuity integration — $4,000:** integrate ENSv2 on Sepolia into an existing project's testnet deployment. Explain the concrete UX or capability improvement and show a functional, open-source demo.

Key docs: <https://docs.ens.domains/ensv2/overview/>, <https://docs.ens.domains/ensv2/permissioned-registry/>, <https://docs.ens.domains/ensv2/permissioned-resolver/>, <https://docs.ens.domains/ensv2/enhanced-access-control/>.

### Uniswap Foundation — $10,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/uniswap-foundation>

- **Best Uniswap Stack Contribution — $6,000:** integrate the Uniswap API, v2/v3/v4, CCA, hooks, or another official stack component.
- **Continuity variant — $4,000:** add a new contribution to an existing project/repository.

Published requirements include a public GitHub repository, a `FEEDBACK.md`, and completion of the [Uniswap Developer Feedback Form](https://developers.uniswap.org/hackathon-feedback). The README should point to the relevant contracts and code so the integration can be verified.

### Sui — $5,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/sui>

- **DeFi & Payments — $5,000:** build programmable payment or financial systems that move, manage, or transform money. Examples include payment flows, wallets, vaults, capital allocators, automation, and financial abstractions.

Resources: <https://docs.sui.io/>, <https://github.com/MystenLabs/sui-stack-hello-world>.

### Curvegrid — $3,000 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/curvegrid>

Three $1,000 categories were listed:

- **Best RWA Tokenization Project:** tokenized treasuries, invoice financing, real-estate shares, supply-chain assets, stablecoins, or programmable asset controls.
- **Best Digital Asset Dashboard:** RWA analytics, treasury management, DeFi analytics, or cross-chain views.
- **Best AI Agent Project:** treasury, payment, monitoring, portfolio, policy-aware transaction, or agent-to-agent payment agents.

MultiBaas is optional. The published README requirements ask for a one-sentence summary, team/social handles, setup and test instructions, and relevant integration feedback.

Docs: <https://docs.curvegrid.com/>.

### Intercepta — $2,500 total

Prize page: <https://ethglobal.com/events/tokyo2026/prizes/intercepta>

- **Safe Agent-to-Agent Payments with x402 — $2,000:** make a real Intercepta API call inside an agent payment flow before a payment is signed or accepted. The verdict must decide whether to pay, refuse, cap, or request human review. Show one successful payment and one blocked/held payment with the reason visible. A live API response is required; hard-coded or mocked responses do not qualify for this prize.
- **Continuity feature — $500:** add the screening step to an existing agent, x402 endpoint, or payment service and show the flow before and after the check.

Request a free event sandbox key through the prize page or partner instructions. Do not commit the key.

## Useful official resources

The event resources page includes Ethereum.org developer guides, Solidity documentation, Remix, Scaffold ETH, CryptoZombies, Alchemy University, Chainlink material, EIPs, L2Beat, and ecosystem resources. Prefer a small set of tools that the team can actually deploy and explain.

- Ethereum developers: <https://ethereum.org/en/developers/>
- Solidity documentation: <https://docs.soliditylang.org/>
- Remix: <https://remix.ethereum.org/>
- Scaffold ETH: <https://scaffoldeth.io/>
- SpeedRunEthereum: <https://speedrunethereum.com/>
- L2Beat: <https://l2beat.com/>
- Ethereum Bug Bounty Program: <https://ethereum.org/en/bug-bounty/>
- Discord: <https://ethglobal.com/discord>

## Evidence checklist for every prize

Keep a short record in the repository:

```text
Prize:
Official requirement:
Relevant files:
Network and contract/API versions:
Happy-path evidence:
Failure/rejection-path evidence:
Tests or logs:
Feedback artifact/form:
README links:
Open question (if any):
```

The strongest submission is one in which a judge can reproduce the integration from the README, inspect the code, and see the required behavior in the live demo.
