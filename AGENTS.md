# Repository execution contract

- Follow instructions in this order: the latest explicit `HUMAN → EXECUTOR`, the current `ORCHESTRATOR → EXECUTOR`, the target issue and unresolved review feedback, nested `AGENTS.md` files, this file, then surrounding code. A specific higher-priority instruction overrides a general lower-priority prohibition only for the named action.
- Read the target issue and every applicable `AGENTS.md` before changing code.
- Build only the requested issue. Do not add future options, speculative flexibility, unrelated cleanup, or one-implementation indirection.
- Follow surrounding code, existing formatters, language idioms, module boundaries, and generated-file rules.
- Keep maintained source readable. Use clear names, direct control flow, cohesive functions, explicit side effects, and useful error context.
- Add abstractions only for at least two real callers. Do not extract one-use helpers only to shorten code.
- Explain non-obvious compatibility constraints and ordering. Do not restate code or leave commented-out code.
- Validate at boundaries. Do not suppress errors or add test-only fallback behavior.
- Add focused tests for observable behavior and meaningful values. Do not weaken, skip, delete, or duplicate tests merely to pass.
- Use repository-native headless validation. Run the target issue's focused local gates and let CI run the authoritative full suite.
- Do not add a dependency without approval. Stop and report if one is required.
- Building packages, installing, and launching the application are prohibited by default. A current human or orchestrator handoff may explicitly authorize building, packaging, or installing the named validated commit.
- Installation authorization does not authorize application launch, interface control, GUI automation, security bypass, or unrelated builds. Do not use simulated input, AppleScript, persistent services, or watchers. Updating browser-test source is allowed; CI runs it.
- One issue equals one branch and one pull request. Every pull-request description contains exactly one `Fixes #N`. Keep requested changes on that same branch and pull request.
- Report verified facts, blockers, uncovered requirements, blocking human verification, and deferred low-risk visual verification. Keep routine command output in CI or the pull request.

Follow `CONTRIBUTING.md` for project-specific `./scripts/dev-env` commands, architecture boundaries, generated TypeScript contract rules, and release, testing, ADR, and release-note references.

## Executor communication

- Begin routine status with `EXECUTOR → HUMAN`.
- Begin blocking questions with `EXECUTOR → HUMAN — ACTION REQUIRED`.
- Return completion and blocker handoffs as one fenced block whose first line is `EXECUTOR → ORCHESTRATOR`.
- Include repository, issue and pull-request numbers, branch, latest commit, CI state, unresolved feedback, uncovered requirements, blocking human verification, deferred visual items, installation state, the installed commit or version when applicable, installation blockers, queue state, and the blocker when blocked.
- Exclude issue repetition, implementation summaries, routine command output, repository history, and optional suggestions from completion handoffs.
