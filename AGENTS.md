# Repository execution contract

- Read the target issue and every applicable `AGENTS.md` before changing code. Issue-specific requirements override repository rules, and applicable nested rules override this file.
- Build only the requested issue. Do not add future options, speculative flexibility, unrelated cleanup, or one-implementation indirection.
- Follow surrounding code, existing formatters, language idioms, module boundaries, and generated-file rules.
- Keep maintained source readable. Use clear names, direct control flow, cohesive functions, explicit side effects, and useful error context.
- Add abstractions only for at least two real callers. Do not extract one-use helpers only to shorten code.
- Explain non-obvious compatibility constraints and ordering. Do not restate code or leave commented-out code.
- Validate at boundaries. Do not suppress errors or add test-only fallback behavior.
- Add focused tests for observable behavior and meaningful values. Do not weaken, skip, delete, or duplicate tests merely to pass.
- Use repository-native headless validation. Run the target issue's focused local gates and let CI run the authoritative full suite.
- Do not add a dependency without approval. Stop and report if one is required.
- Never launch or interact with the application. Do not use GUI or browser automation, simulated input, AppleScript, persistent services, watchers, installers, or platform-integration builds locally. Updating browser-test source is allowed; CI runs it.
- One issue equals one branch and one pull request. Keep requested changes together and use the issue's exact pull-request description.
- Report verified facts, blockers, uncovered requirements, and required human verification. Keep routine command output in CI or the pull request.

Follow `CONTRIBUTING.md` for project-specific `./scripts/dev-env` commands, architecture boundaries, generated TypeScript contract rules, and release, testing, ADR, and release-note references.
