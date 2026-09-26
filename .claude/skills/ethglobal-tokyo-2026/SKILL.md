---
name: ethglobal-tokyo-2026
description: Build, evaluate, and submit projects for ETHGlobal Tokyo 2026, including partner-prize research, Classic and Continuity tracks, implementation, demo planning, and final submission checks.
---

# ETHGlobal Tokyo 2026

Use this skill when the user is participating in, planning for, or submitting to ETHGlobal Tokyo 2026. It is an event-specific operating guide, not a substitute for the live ETHGlobal pages or partner documentation.

## Operating principles

1. Treat the official ETHGlobal pages and each partner's documentation as the source of truth.
2. Re-check live pages before making a time-sensitive claim about deadlines, prize rules, eligibility, APIs, or judging. The snapshot in `references/event-and-prizes.md` is a starting point, not a substitute for verification.
3. Distinguish **Classic — From Scratch** from **Continuity — Extend Open Source / Ship a Feature**. Never assume which track applies.
4. Optimize for a working, explainable product that can be demonstrated live, not a large pile of unfinished features.
5. Preserve an honest Git history and clearly separate pre-existing work from work completed during the event.
6. Be concise and action-oriented during the hackathon. Ask only questions that block the next implementation decision.

## First response: establish context

Before advising or editing, determine or ask for:

- project idea and target user/problem;
- Classic or Continuity track, if known;
- target prize(s), if any;
- current repository state and stack;
- target network/testnet and available time;
- whether the user needs architecture, implementation, testing, demo, or submission help.

Then return a short working brief:

```text
Goal:
Track:
Target prize(s):
Minimum demo path:
Current state:
Next three actions:
Main risks:
```

For a repository task, inspect the repository and preserve unrelated files. Do not overwrite user work or make a submission, deploy, purchase, or financial transaction without explicit permission.

## Event snapshot

The following was published for ETHGlobal Tokyo 2026 and must be verified before relying on it:

- **Submission deadline:** Sunday, September 27, 2026 at 09:00 JST. Late submissions are not accepted.
- **Submission location:** ETHGlobal Hacker Dashboard: <https://ethglobal.com/events/tokyo2026/home>
- **Team size:** Up to five people. Each participant must be accepted and stake individually; the published information says the stake is normally returned about three weeks after the event when a project is submitted.
- **Finalist presentation:** Four-minute demo followed by three minutes of judge Q&A, if selected for finalist judging.
- **Demo video:** Optional but strongly encouraged; upload requirements are 2–4 minutes and at least 720p. Avoid mobile-phone recording, AI/text-to-speech voiceovers, speeding up footage, music with explanatory text, long introductions, and unnecessary waiting.
- **Judging:** Technicality, Originality, Practicality, Usability (UI/UX/DX), and WOW Factor. See `references/judging-criteria.md` for the detailed rubric and self-assessment worksheet.

The authoritative event pages are:

- Info: <https://ethglobal.com/events/tokyo2026/info>
- Getting started: <https://ethglobal.com/events/tokyo2026/info/start>
- Rules, submission, judging, and video guidance: <https://ethglobal.com/events/tokyo2026/info/details>
- Resources: <https://ethglobal.com/events/tokyo2026/info/resources>
- Prizes: <https://ethglobal.com/events/tokyo2026/prizes>
- Dashboard: <https://ethglobal.com/events/tokyo2026/home>

## Track compliance

### Classic — From Scratch

- Start project-specific implementation, design, and assets after the hackathon officially starts.
- Public libraries, starter kits, and boilerplates are allowed, but document them.
- Do not present pre-existing project-specific code or assets as work completed during the event.
- The published guidance says projects built before the event may not qualify for partner prizes or the Finalist category; verify the current rules before applying.

### Continuity — Extend Open Source / Ship a Feature

- Building on an existing repository may be allowed under the selected track.
- Clearly document what existed before the hackathon and what new feature or functionality was completed during it.
- Keep new commits and a focused diff so judges can verify the contribution.
- Partner-prize eligibility varies; check the exact prize page and ask the partner/mentor when uncertain.

## Prize-selection workflow

1. Read the live prize page and its linked qualification requirements.
2. Select no more than three partner prizes in the submission form. A partner with multiple tracks may count as one Partner Prize even when several tracks are eligible.
3. Prefer prizes whose technology solves the product's core problem; avoid cosmetic integrations that cannot be demonstrated.
4. Build a requirement matrix before coding:

| Requirement | Evidence to collect | Status |
| --- | --- | --- |
| Required SDK/protocol/contract | Link, version, transaction or test | TODO |
| Required network | Chain ID, deploy address, testnet fork | TODO |
| Required user flow | Success path and rejection/error path | TODO |
| Required documentation | README location, setup steps, feedback | TODO |
| Submission artifact | Repo, live demo, video, form response | TODO |

5. For every selected prize, record the exact files or user actions that prove integration. Point judges to them in the README.
6. Never promise a prize, infer eligibility, or fabricate an integration. If a requirement cannot be verified, say so and choose a smaller demonstrable scope.

A concise snapshot of the published prize categories and official links is in `references/event-and-prizes.md`. Re-fetch that page before recommending a prize.

## Build strategy

Use a thin vertical slice:

1. Write the user story and the single action that proves the idea.
2. Define the trust, money, identity, or coordination boundary the product handles.
3. Choose the smallest architecture that can complete that action end to end.
4. Prefer a local fork or testnet for the happy path, then add one realistic failure/rejection path.
5. Add tests for contract logic, authorization, input validation, and the most important UI states.
6. Record setup, environment variables, network, deploy addresses, and demo steps in the README.
7. Commit coherent, incremental changes; do not squash the entire event into one final commit.

### AI-use transparency

ETHGlobal's published guidance permits AI assistance when the team is transparent and meaningfully involved. Maintain an `AI_USAGE.md` or equivalent section that records:

- tools and approximate dates of use;
- tasks assisted (planning, code generation, debugging, research, assets);
- important files or areas affected;
- human decisions, testing, and validation performed;
- any prompts, specs, or planning artifacts that should be included.

Do not present generated output as an unreviewed finished product. Preserve the team's reasoning and verification.

## Security and reliability guardrails

- Keep private keys, API keys, seed phrases, and auth tokens out of source control and client bundles.
- Validate identity, payment, and API responses on a trusted backend or otherwise at the authorization boundary; do not treat an unvalidated client response as proof or permission.
- Use allowlists, spending limits, expiry, cancellation, and human approval where an agent or payment flow can move value.
- Make the denied, expired, cancelled, malformed, or insufficient-funds path visible in the product.
- Prefer testnets, forks, faucets, and sandbox keys for the demo unless the user explicitly requests otherwise.
- Check arithmetic, slippage, token identity, chain ID, contract addresses, and revert paths before calling a DeFi flow safe.
- Do not expose a partner secret in a demo video or screenshot.

## Demo and submission workflow

### Before recording

- Freeze a stable deployment or local fork and record the exact command used to run it.
- Prepare two or three seeded test accounts/states if the flow depends on identity, ownership, or history.
- Remove dead ends, long RPC waits, wallet setup noise, and debug panels from the recorded path.
- Prepare a fallback recording or screenshots, but do not present mocks as live integration evidence.
- Write a one-sentence project summary and no more than four bullets per slide.

### Demo video outline (2–4 minutes)

1. **Problem and user (about 20 seconds):** state the problem, intended user, and why the web3/AI component matters.
2. **Core flow (about 60–90 seconds):** connect or use the test environment, perform the real action, and show the resulting state/transaction.
3. **Technical proof (about 30–45 seconds):** show the relevant contract/API/integration, test, or verification path.
4. **Impact and honesty (about 20–30 seconds):** explain who benefits, limitations, and what was built during the event.
5. **Closing (about 10–20 seconds):** name the repository, deployed demo, and integration feedback.

Record in landscape at 720p or higher, use a real human voice, and do not speed up the video to meet the limit.

### Final checklist

- [ ] Correct Classic or Continuity track is selected.
- [ ] Team members and repository contributors are accurate.
- [ ] Repository is public and has meaningful incremental history.
- [ ] README explains the product, architecture, setup, tests, and exact integration points.
- [ ] Live/testnet/fork demo works from a clean checkout.
- [ ] Required partner integrations are visible in the user flow, not just mentioned.
- [ ] Success and failure/rejection paths are demonstrated.
- [ ] Required feedback file/form, such as Uniswap's `FEEDBACK.md` and feedback form, is complete when applicable.
- [ ] AI usage and pre-existing work are disclosed accurately.
- [ ] No secrets or sensitive credentials are committed.
- [ ] Demo video is optional/within 2–4 minutes/at least 720p and has no prohibited recording or voiceover issue.
- [ ] Up to three Partner Prizes are selected deliberately and the final repository link is correct.
- [ ] Submission is completed before Sunday, September 27, 2026 at 09:00 JST after re-checking the live deadline.

## Response style during the hackathon

When the user asks for help, lead with the smallest useful action:

- **Idea:** give a scoped vertical slice and a falsifiable demo path.
- **Architecture:** state the trust boundary, chain, contracts, API calls, and failure handling.
- **Implementation:** edit the smallest coherent slice, then run the relevant tests/build.
- **Debugging:** reproduce first, isolate the failing boundary, and avoid broad rewrites.
- **Prize advice:** cite the live requirement and identify the evidence needed; do not merely list technologies.
- **Submission:** audit the repository, README, demo, prize form, and deadline as separate gates.

When a requirement or deadline cannot be verified from the live source, explicitly mark it **VERIFY** and link the relevant page instead of guessing.
