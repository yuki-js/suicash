---
name: sui-move
description: Production-oriented workflow for designing, implementing, testing, reviewing, and deploying Move on Sui software with an LLM. Use when writing or reviewing Sui Move contracts, PTBs, toolchain/dependency setup, or Sui deployment workflows.
---

# Building Sui Move Software with LLMs in 2026

**Scope:** A production-oriented workflow for using an LLM to design, implement, test, review, and deploy software built on Move on Sui.[1] This reflects the Sui documentation and toolchain available on **25 September 2026**.[11] The Sui ecosystem changes quickly, so pin versions and re-check the official documentation for each project.[14]

## Executive recommendation

Use an LLM as a **tool-using software engineer inside a deterministic development loop**, not as an autonomous authority that writes a contract and publishes it.[unverified] This is a recommendation, not a claim about LLM capabilities.[unverified]

The strongest 2026 setup is a proposed synthesis grounded in official tooling and workflow guidance.[1][4][5]

1. A current, network-matched Sui CLI and Move toolchain.[17]
2. Mysten Labs' Sui agent skills as the baseline context.[2]
3. Sui documentation retrieval (the Sui Docs MCP server and/or the local `sui prompt` command).[6][10]
4. Move Analyzer/LSP diagnostics for semantic feedback.[8]
5. A test-first workflow: build, lint, unit tests, scenario tests, localnet, and dry runs.[5][7][11]
6. Formal verification for high-value invariants, followed by an independent human/security review.[9][12]
7. CI gates and human-controlled Testnet/Mainnet deployment.[11][12]

The LLM should propose code and explanations.[unverified] The compiler, tests, prover, simulation, and reviewers should decide whether the result is acceptable.[unverified]

## 1. Give the agent a project contract, not just a feature request

Before asking for code, create a repository-level agent instruction file such as `AGENTS.md` or `CLAUDE.md`.[unverified] It should state:

- the exact network, Sui CLI version, Move edition, and dependency policy;
- the commands for build, format, lint, test, coverage, localnet, and dry runs;
- the package/module layout and which public API is frozen;
- the object-ownership model for every state object;
- the authorization model and capability relationships;
- the invariants that must never be broken;
- the event schema and off-chain consumers;
- non-goals, especially "do not publish, sign, or modify deployment keys";
- the rule that unknown APIs and missing information must be retrieved or reported, never guessed.

Keep the context small and task-specific.[2] Mysten Labs' current skills repository recommends a three-skill starter set (`sui-overview`, `sui-move`, and `sui-move-project`) for newcomers and warns that installing every skill can create unnecessary context overhead and trigger collisions.[2] Load additional skills only when the task requires them.

For a task involving a package whose source is not in the repository, do not infer its interface from its name.[4] Retrieve the module interface, resolve the dependency graph, and identify the exact on-chain package ID.[4] A January 2026 third-party benchmark found that the largest reliability difference between LLMs came from progressive interface discovery rather than syntax recall; models that guessed missing interfaces failed badly.[15] Treat that as a useful warning, not as a general model ranking.

## 2. Separate the LLM's roles

Use separate prompts or sessions for at least four roles:[unverified]

### Planner

Produce the following design artifacts:[unverified]

- user-visible behavior;
- state and ownership diagram;
- module boundaries;
- public/entry/private function split;
- invariants and abort conditions;
- dependency and upgrade assumptions;
- test matrix and threat model.

The planner should label unknowns explicitly and should not "fill gaps" with plausible Move APIs.[unverified]

### Implementer

Make one small, reviewable change at a time.[unverified] Keep implementation, tests, and specification in separate files or clearly separated sections.[unverified] Ask the implementer to show the exact files changed and the commands it intends to run.[unverified]

### Verifier

Run deterministic tools and inspect their output.[unverified] The verifier should not claim that code works merely because it looks correct.[unverified] It should report failing commands, warnings, and unverified assumptions.[unverified]

### Independent reviewer

Review the design and diff from an attacker's perspective: authorization, object-ID confusion, capability misuse, rounding, replay, shared-object contention, upgrade authority, event omissions, and dependency changes.[unverified] A second model can help, but it does not replace a human security review.[unverified]

## 3. Design the object and API model before generating implementation

Sui's object model is a core design decision, not an implementation detail.[3] Decide whether each piece of state should be address-owned, shared, wrapped, or immutable.[20] Avoid making state shared merely because a contract needs global access: mutable shared-object access can serialize transactions, while immutable references can preserve parallelism. The object-model skill and official documentation emphasize these ownership and versioning consequences.[20]

For every public function, decide explicitly:[unverified]

- Is it `public` for composability or `entry` for a transaction endpoint?
- Does it return an object or transfer it internally?
- Does it mutate shared state?
- Which capability authorizes it?
- Which object ID proves that the capability and target object belong together?

Official Move guidance recommends keeping core functions composable and returning values instead of silently transferring them, while using a separate `entry` wrapper when a convenient transaction endpoint is needed.[19] It also recommends explicit capability parameters instead of using `tx_context::sender()` as the only authorization mechanism.[4]

Make the public API deliberately.[18] Public signatures and published struct layouts are compatibility commitments.[23] Internal helpers should generally use the narrowest visibility that still works, such as `public(package)`, so upgrades do not accidentally become breaking changes.[18]

## 4. Make the compiler/test loop mandatory

A plausible code block is not a deliverable.[unverified] Every implementation step should end with real tool output.[unverified] A useful minimum loop is:[unverified]

```bash
sui move format
sui move lint
sui move build
sui move test
sui move test --coverage
```

Use `sui move test --trace` when a failure needs execution-level diagnosis.[7] `sui move lint` is available in the current CLI and provides generic Move and Sui-specific object-model checks.[14][19]

Test at several levels:[unverified]

- **Unit tests:** pure calculations, validation, error codes, and resource cleanup.
- **Negative tests:** exact `expected_failure` behavior for authorization and invalid input.
- **Scenario tests:** multiple senders, multiple transactions, shared objects, transfers, and initialization flows.
- **Application tests:** TypeScript SDK transaction construction, PTB composition, object queries, and event handling.
- **Network tests:** localnet first, then Testnet, then a production dry run against representative state.

The official testing documentation recommends `test_scenario` for multi-transaction flows and shows that `expected_failure` should match the actual abort code and origin when necessary.[5] The official testing skill also recommends `assert_eq!` for equality checks, `tx_context::dummy()` for simple context-only tests, and `std::unit_test::destroy` for cleanup.[21]

Do not let the LLM report success from code inspection.[unverified] Require it to paste or summarize the actual command result, including test counts, failures, warnings, and coverage.[unverified] For security-critical changes, add independent property-based or fuzz testing where appropriate; testing and formal verification answer different questions.[unverified]

## 5. Treat security invariants as first-class artifacts

Before implementation, write a short threat model and an invariant list.[4] At minimum, review:[unverified]

- capability requirements and capability-to-object ID checks;
- revocation and rotation of `AdminCap`, `TreasuryCap`, `MetadataCap`, `DenyCapV2`, and `UpgradeCap`;
- authorization of every privileged shared-object function;
- amount, zero, overflow, and rounding edge cases;
- stale oracles and replayable off-chain data;
- randomness restrictions and PTB composition;
- emergency pause/rate-limit authority;
- event emission for privileged actions;
- exact package and dependency IDs, including whether dependencies remain upgradeable.

The Sui security guidance is explicit that language-level resource safety and checked arithmetic are foundations, not substitutes for business-logic review, invariant testing, access-control review, or economic analysis.[4]

For DeFi or other high-value contracts, create formal specifications for properties such as:[unverified]

- total balance preservation;
- share price monotonicity;
- no unauthorized withdrawal;
- no supply/accounting divergence;
- correct rounding direction;
- immutability of a field after initialization.

The open-source Sui Prover can verify selected Move specifications and produce counterexamples; it is useful for expressing and checking properties that ordinary tests may miss.[9][10] Formal verification does not replace economic analysis, dependency review, or a human audit. Keep formal verification results and audit reports as separate release evidence.[9][12]

## 6. Pin the toolchain and verify every dependency

### Prefer proven libraries over LLM-written primitives

The LLM should not reimplement security-critical primitives that already have reputable, audited implementations.[7][29] This is especially important for arithmetic and fixed-point math.[30][32] Access control and timelocks are also covered by reviewed packages.[33] The official Sui tooling page lists OpenZeppelin Contracts for Sui as a library of audited contracts.[7] OpenZeppelin announced the Sui math and access-control release in March 2026.[34] OpenZeppelin's package documentation provides MVR installation and API guidance for its packages.[30][31] The audit index shows package-specific reviews across releases.[37] However, the repository's current README warns that the library is no longer under active maintenance, so pin a known release, inspect the relevant audit, and verify the exact package rather than assuming that the name alone means "safe".[29]

Use a library when the required capability is covered by a reputable, public, audited package that matches the target Move/Sui version and the design's assumptions.[7][36]

- the library's owner is reputable and its code and audit reports are public;
- the package is released for the target Sui/Move version;
- the exact package ID and dependency revision are pinned;
- the API matches the design being built;
- the library's assumptions are compatible with your own invariants;
- the dependency remains immutable, or its upgrade authority is separately governed.

Do not use a library merely because it is popular or described as audited.[7][29] A dependency can introduce a new attack surface, incompatible assumptions, gas costs, and upgrade risk.[36]

Before accepting a library, require the agent to record the repository, release, package IDs, audit scope, upgrade status, API, limitations, and integration test results.[29][36][37]

- repository and release/version;
- package ID on each target network;
- audit organization, date, scope, and unresolved findings;
- whether the package is upgradeable;
- the exact modules and public functions used;
- known limitations, gas cost, and API migration notes;
- how the package behaves under failed or partial transactions;
- the test results for the integration.

For a new project, a sensible starting catalog is the Sui framework plus task-specific audited packages, selected by capability rather than by adding every available integration.[7][35]

- **Sui framework (`sui`, `std`)** for the platform primitives, objects, coins, transfers, and transaction context;
- **OpenZeppelin Contracts for Sui** for reviewed access-control and math building blocks where its pinned release fits;
- **DeepBook V3** when the protocol actually needs its order-book or liquidity primitives;
- **Sui Kiosk** for NFT marketplace and transfer-policy flows;
- **zkLogin, Enoki, and Seal** for authentication, sponsored transactions, and threshold encryption;
- **Walrus** for decentralized blob storage rather than storing large data in Move objects;
- **a protocol-specific audited package** for lending, swapping, pricing, or indexing only when that exact protocol is part of the requirements.

The catalog above is a starting point, not a blanket recommendation.[35] Do not add DeepBook, Kiosk, Walrus, or a lending protocol merely to make the repository look complete.[7][35]

Do not let the LLM choose "latest" versions or invent package names.[unverified]

- Use `suiup` to install and switch network-specific Sui toolchains.[17][22]
- Pin the Sui CLI and the on-chain framework version used for a release.[17]
- Commit `Move.toml`, `Move.lock`, and the toolchain's publication state file (`Published.toml`) where applicable.[3][18]
- Never hand-edit `Move.lock`; regenerate it with the intended toolchain when dependency state is wrong.[18][28]
- Use the current Move 2024 manifest format and avoid copying legacy `[addresses]` or framework-dependency examples.[18]
- Pin third-party dependencies and record their package IDs and upgrade policies.[4]
- Use `sui client verify-source` during release verification, with the release toolchain required by the publication metadata.[14]
- For multi-environment packages, build and test with an explicit target environment.[18]

For full-stack applications, use the current Sui data interfaces deliberately: the official documentation describes gRPC as the typed, low-latency backend/streaming option and GraphQL as the flexible query option, with archival storage needed for historical data.[26] Do not build new production dependencies on an interface that is already being retired; the 2026 documentation reports a JSON-RPC shutdown on Sui Foundation Mainnet full nodes and a planned code decommission later in 2026.[26][14]

## 7. Treat PTBs and generated bindings as first-class code

Move contracts are not the whole application.[unverified] The LLM may generate TypeScript SDK code, PTBs, indexers, and frontend calls as well.[unverified]

For every generated PTB:[unverified]

1. Resolve object IDs and versions from current state.[13]
2. Check argument types against the actual module interface.[13][24]
3. Chain results with explicit result references.[13][24]
4. Consume or destroy every non-`drop` value.[13][24]
5. Simulate with `--dry-run` or the SDK equivalent.[11][13]
6. Check the execution status, not only the transaction digest.[13]
7. Have the wallet or an offline signer show the intended effects to the human.[11][12]

PTB command semantics are stricter than a sequence of ordinary function calls: failures are atomic, and generated values must be consumed.[13][24] The official PTB skill documents the input/command model, result chaining, atomicity, and common SDK/CLI mistakes.[24]

Prefer generated bindings from the reviewed Move source or exact on-chain package, then review their package IDs and types.[unverified] Never trust a human-readable package name or a stale frontend constant as proof of identity.[unverified]

## 8. Use the 2026 agent tooling in layers

### Baseline: official Mysten Labs skills

Start with the official skills repository and its `sui-move`, `sui-move-project`, `object-model`, `move-unit-testing`, `sui-build-test`, `sui-publish`, and `move-security` guidance as needed.[1][2] These are a strong baseline because they are maintained alongside Sui's documentation ecosystem.[unverified]

### Documentation retrieval

Use the Sui Docs MCP server to search and fetch current documentation.[6] It is intended to provide recent material rather than relying on an agent's stale training data.[6] Connect it to the coding agent as a tool, and make the agent cite or record the exact document it used for API decisions.[unverified]

For a lightweight, self-contained option, the Sui CLI now contains `sui prompt`, which embeds categorized Move/Sui skills and can print a category, a skill bundle, or a reference file.[10][10] Treat this as a complementary, evolving tool; verify important behavior against the live official docs.

### Semantic diagnostics

Use Move Analyzer/LSP for completions, hover types, definitions, references, and diagnostics.[8] Keep the analyzer and Sui CLI versions aligned when possible.[unverified]

### Formal verification

Use Sui Prover or a commercial alternative for high-value invariants.[9][16] A verifier-backed specification is an artifact that can be rerun in CI, not merely a prose claim.[unverified]

### Community orchestration layers

`contract-hero/sui-pilot` combines bundled documentation, an LSP bridge, a prover wrapper, and an end-to-end browser-testing workflow.[13] `first-mover-tw/sui-dev-agents` offers a broader agent/command/hooks lifecycle toolkit.[14] These are community-maintained; pin their versions, inspect their code, and do not let their bundled examples override current official Sui documentation.[unverified]

## 9. Use a strict task prompt

A useful prompt contract is:[unverified]

```text
You are working in a Sui Move repository.

Before editing:
1. Read the repository instructions, Move.toml, Move.lock, relevant modules,
   tests, and publication state.
2. Retrieve current official Sui documentation for every framework API you use.
3. State the object-ownership model, authorization model, invariants, and
   unresolved assumptions.
4. If a required fact is missing, stop and report it; do not invent an API.

While editing:
5. Make the smallest reviewable change.
6. Preserve public API compatibility and pinned dependencies.
7. Add positive, negative, edge-case, and scenario tests as appropriate.
8. Do not edit deployment keys or publish anything.

Before reporting completion:
9. Run format, lint, build, tests, and relevant simulations.
10. Show the exact commands and real results.
11. List remaining warnings, assumptions, and unverified behavior.
12. Never claim that a test or deployment succeeded without tool output.
```

For non-trivial work, split the prompt into planning, implementation, test design, and independent review passes.[unverified] A single request for "a complete DeFi protocol" encourages hidden assumptions and makes review difficult.[unverified]

## 10. Evaluate the agent on your own repository

Do not choose a model from a leaderboard or a single coding demo.[unverified] Build a small evaluation set from real tasks in the target repository, for example:[unverified]

- add a Move function with a new abort condition;
- model an owned/shared object choice;
- add a multi-sender scenario test;
- integrate a third-party package;
- repair a compiler or dependency error;
- review an intentionally vulnerable implementation;
- generate a PTB from an exact module interface;
- write a formal specification for one invariant.

Measure:[unverified]

- build success and first-pass success;
- test discovery and failure diagnosis;
- number of incorrect API assumptions;
- security findings and false positives;
- whether the agent respects repository constraints;
- recovery after compiler/test feedback;
- time, tokens, and monetary cost;
- human review effort.

The 2026 third-party type-inhabitation report is useful evidence that missing-information handling can dominate raw syntax ability, but it tested a narrow PTB construction task and should not be treated as a general security or software-engineering benchmark.[15]

## Recommended default operating procedure

For a new Sui Move project, the recommended procedure is:[unverified]

1. Pin a current Sui toolchain with `suiup`; install `move-analyzer` when available.[17][7]
2. Create the project with the Move 2024 format and commit `Move.lock`.[18]
3. Install the official starter skills, then add only task-specific skills.[2]
4. Add `AGENTS.md`/`CLAUDE.md` with commands, invariants, and deployment prohibitions.[unverified]
5. Connect the Sui Docs MCP server; optionally use `sui prompt` for embedded guidance.[6][10]
6. Write the object/API/threat model before implementation.[3][4][20]
7. Have the LLM implement one small change with tests.[unverified]
8. Run format, lint, build, tests, coverage, localnet scenarios, and dry runs.[5][7][11]
9. Run a separate security review and formal verification for critical properties.[4][9][25]
10. Pin the exact release toolchain, verify source, and require human approval for Testnet/Mainnet publication.[11][12][27]
11. Transfer privileged capabilities to multisig or hardware custody; never give an LLM the private key.[4][12]
12. Monitor events, transaction failures, gas, package upgrades, and dependency changes after launch.[12]

## The short version

The 2026 best practice is **retrieval + small changes + compiler feedback + adversarial tests + formal invariants + human deployment gates**, with proven libraries preferred over LLM-written security primitives.[unverified] This is a synthesis and recommendation, not a universal empirical result.[unverified]

## Sources

[1] https://docs.sui.io/skills [2] https://github.com/MystenLabs/skills [3] https://docs.sui.io/develop/write-move/move-best-practices [4] https://docs.sui.io/develop/security/best-practices [5] https://docs.sui.io/develop/testing-debugging/testing [6] https://docs.sui.io/getting-started/sui-mcp-server [7] https://docs.sui.io/getting-started/tooling [8] https://docs.sui.io/references/ide/move [9] https://github.com/asymptotic-code/sui-prover [10] https://raw.githubusercontent.com/MystenLabs/sui/main/crates/sui-prompt/README.md [11] https://docs.sui.io/develop/publish-upgrade-packages/deploy-github-actions [12] https://docs.sui.io/develop/production-readiness [13] https://docs.sui.io/references/cli/ptb [14] https://docs.sui.io/references/release-notes [15] https://paragraph.com/@evandekim/llm-benchmark-for-move-smart-contract-type-inhabitation-evaluation [16] https://docs.certora.com/en/latest/docs/move/usage.html [17] https://docs.sui.io/getting-started/onboarding/sui-install [18] https://github.com/MystenLabs/skills/blob/main/sui-move-project/SKILL.md [19] https://github.com/MystenLabs/skills/blob/main/composable-move-functions/SKILL.md [20] https://github.com/MystenLabs/skills/blob/main/object-model/SKILL.md [21] https://github.com/MystenLabs/skills/blob/main/move-unit-testing/SKILL.md [22] https://github.com/MystenLabs/skills/blob/main/sui-build-test/SKILL.md [23] https://github.com/MystenLabs/skills/blob/main/sui-publish/SKILL.md [24] https://github.com/MystenLabs/skills/blob/main/ptbs/SKILL.md [25] https://github.com/MystenLabs/skills/blob/main/move-security/SKILL.md [26] https://docs.sui.io/develop/accessing-data/json-rpc-migration [27] https://docs.sui.io/develop/manage-packages/source-verification [28] https://docs.sui.io/references/cli/move [29] https://github.com/OpenZeppelin/contracts-sui [30] https://docs.openzeppelin.com/contracts-sui [31] https://docs.openzeppelin.com/contracts-sui/1.x [32] https://docs.openzeppelin.com/contracts-sui/1.x/math [33] https://docs.openzeppelin.com/contracts-sui/1.x/access [34] https://www.openzeppelin.com/news/introducing-openzeppelin-contracts-for-sui [35] https://docs.sui.io/references/awesome-sui [36] https://docs.sui.io/develop/manage-packages/move-package-management [37] https://raw.githubusercontent.com/OpenZeppelin/contracts-sui/main/audits/README.md
