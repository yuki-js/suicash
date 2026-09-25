# ETHGlobal Tokyo 2026 judging criteria

This reference captures the judging criteria published for ETHGlobal Tokyo 2026 and provides a practical self-assessment worksheet.

**Live source:** <https://ethglobal.com/events/tokyo2026/info/details>

> ETHGlobal publishes the five categories below but does not publish numerical weights or a formal scoring scale. The 1–5 worksheet in this file is a preparation aid, not ETHGlobal's official scoring system. Re-check the live page before submitting.

## Official criteria

### 1. Technicality

Judges consider the complexity of the problem and the sophistication of the solution.

Strong evidence usually includes:

- A deliberate architecture rather than a collection of disconnected integrations
- Meaningful smart-contract, blockchain, or protocol design
- Correct handling of authorization, funds, identity, state, and failure cases
- Tests, simulations, or reproducible technical proof
- A clear explanation of important engineering trade-offs

Technicality is not the same as code volume. A small, well-designed implementation can be stronger than a large collection of unverified features.

Ask yourself:

- Why is this problem difficult?
- Why is blockchain, web3, or an onchain component necessary?
- What is the most technically interesting decision in the project?
- What security or correctness risks did you identify?
- Can a judge verify the claim from the repository or demo?

### 2. Originality

Judges consider whether the project introduces a new idea or solves an existing problem in a creative, differentiated way.

Strong evidence usually includes:

- A clear insight rather than a copied template
- A distinct user, context, or mechanism
- An unexpected connection between technologies
- A problem that the team understands personally or technically

Avoid describing the project as only "an AI app," "an NFT marketplace," or "a dashboard." Explain the specific insight that makes this version different.

Ask yourself:

- What would a generic implementation look like?
- What does this project do that a generic implementation would not?
- Why does the user care?
- Is the novelty in the product, the mechanism, or the user experience?
- Could the idea work beyond crypto users?

### 3. Practicality

Judges consider whether the project is complete, functional, and usable by its target audience today.

Strong evidence usually includes:

- A working end-to-end core flow
- Realistic inputs, users, or test data
- A stable testnet, local fork, or production-like deployment
- Clear setup instructions
- Graceful error and edge-case handling
- An honest explanation of current limitations

A narrow, reliable MVP is usually more practical than a large product with many broken paths. Do not claim that a mock, screenshot, or hard-coded response is a live integration.

Ask yourself:

- Can a judge reproduce the main flow from a clean checkout?
- Is every button in the demo connected to real behavior?
- Does the project handle loading, rejection, expiry, and insufficient funds?
- Does the team know the current limitations?
- Would a real target user understand what to do next?

### 4. Usability — UI/UX/DX

Judges consider how intuitive the product is for its users and how easy it is for developers to work with.

Strong evidence usually includes:

- A clear first-run experience
- Obvious calls to action
- Useful wallet, network, and transaction feedback
- Readable errors with a recovery path
- Accessible and consistent visual design
- A good README, environment-variable documentation, and test commands
- A quick path for a partner or judge to verify the integration

Usability includes both the product experience and the developer experience. A technically correct project can still score poorly if nobody can figure out how to run it.

Ask yourself:

- Is the main action obvious within the first screen?
- Does the user know whether an operation is pending, succeeded, or failed?
- Are secrets and sensitive data hidden?
- Does the README point to the exact integration files?
- Can a new developer run the project quickly?

### 5. WOW Factor

Judges consider whether the project leaves a lasting impression or contains something especially compelling.

Strong evidence usually includes:

- A surprising or delightful interaction
- A particularly elegant technical solution
- A strong real-world use case
- A memorable demo moment
- Careful product polish and a coherent story

WOW Factor does not require flashy visual effects. A small idea that is executed exceptionally well can have more impact than a large project with noisy features.

Ask yourself:

- What is the one moment a judge will remember?
- Why should this project stand out from other submissions?
- Is the presentation coherent from problem to demo to impact?
- Would you be impressed if you saw it without knowing the technology?
- What is the strongest visual or emotional proof of the idea?

## What judges evaluate

The published guidance says judges look at:

- Creativity
- Functionality
- Technical difficulty
- Quality of the final product
- The five categories above
- The work completed during the hackathon

Finalist judging is structured as a four-minute presentation followed by three minutes of Q&A. Partner judges may also evaluate the project against the partner's specific qualification requirements. Booth attendance does not replace the submission materials.

## Practical self-assessment worksheet

Use a 1–5 scale only to identify weak areas. This is **not** an official ETHGlobal score.

| Criterion | 1: Weak | 3: Working | 5: Strong | Evidence / next fix |
| --- | --- | --- | --- | --- |
| Technicality | Mostly UI or a superficial integration | Core flow works with a reasonable architecture | Deliberate design, proof, tests, and clear trade-offs | |
| Originality | Generic copy of a common template | Distinct use case but familiar mechanism | Memorable insight or genuinely new mechanism | |
| Practicality | Prototype with major broken paths | Core flow works | Reliable, reproducible, and useful today | |
| Usability | Judge cannot find or understand the flow | Understandable with help | Polished, intuitive, and easy to run | |
| WOW Factor | No memorable differentiator | Solid execution | Clear standout moment and strong story | |

Do not average the scores and present the result as an official prediction. Fix the lowest category first if time is limited, then improve the category that most affects the chosen prize.

## Evidence to collect before submission

Create a checklist for each selected prize:

- [ ] One-sentence problem statement
- [ ] Target user and concrete use case
- [ ] Architecture diagram or short explanation
- [ ] Live testnet, local fork, or deployment link
- [ ] Happy-path demo
- [ ] Failure, rejection, or edge-case demo
- [ ] Tests, logs, or technical proof
- [ ] Exact files implementing the prize integration
- [ ] README setup instructions
- [ ] Required feedback artifact or form
- [ ] AI-use disclosure and pre-existing-work notes
- [ ] Final 2–4 minute demo video, if chosen

## Questions to prepare for Q&A

Prepare concise answers to:

- What inspired the project?
- What problem does it solve, and for whom?
- Why is a blockchain or onchain design appropriate?
- What did you build specifically during the hackathon?
- What was the hardest technical problem?
- How did you test the product?
- What happens when verification or payment fails?
- Which files prove the partner integration?
- What are the limitations, and what would you build next?

## Final judging audit

Before the submission deadline, verify:

1. The repository is public and runnable.
2. The README makes the value proposition understandable in under a minute.
3. The demo shows the real core flow, not only slides or mock data.
4. The technical claims are supported by code, transactions, tests, or logs.
5. A failure or rejection path is visible.
6. The presentation has a single memorable story.
7. Partner-prize requirements are satisfied and linked from the README.
8. No credentials, private keys, or sensitive information are exposed.

Re-check the live rules and deadline at <https://ethglobal.com/events/tokyo2026/info/details> and submit through <https://ethglobal.com/events/tokyo2026/home>.
