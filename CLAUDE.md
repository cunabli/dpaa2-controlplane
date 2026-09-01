# dpaa2 controlplane

This repo builds a declarative, intent-based control plane for NXP LX2160's DPAA2 Management Complex (MC), written in Rust. It reifies a topology, performance, and usage intent into the MC objects that carry it; every object or resource the MC creates, controls, configures, or observes (DPNI, DPMAC, DPBP, DPIO, DPCON, DPMCP, DPRC, …), so those objects are used correctly and efficiently and their lifecycle is fully managed, from creation to teardown.

The goal is to replace the restool CLI interface (all scripts and entry points shipped via ls-main, such as ls-addni, ls-listmac, ls-delete), and eventually restool itself, with typestate-correct Rust that makes an invalid hardware topology unrepresentable and converges the board to the stated intent idempotently, level-triggered, with no persisted state.

## Rules

1. Follow the directions provided in the prompt and stop and ask when the how to do something is not prescriptive.
2. Code quality starts with architecture, the code MUST be structured at all times to ensure encapsulation will not require heavy rewrites.
3. When using openspec, DO NOT DO all tasks in parallel and follow their dependency structure so learnings from one task can be reapplied.
4. When using openspec with beads, ABSOLUTELY DO ONE TASK AT A TIME and through completion of acceptance criteria before moving to dependent tasks.

## Idioms and tenets

1. This tool is designed through open specifications. Those specifications must always work backwards from a business outcome, which must be clearly articulated in terms of direct benefits and gaps. The fundamental definitions and agreements the tool implements are described as architectural decision records.
2. The repository is self contained in information but some tools will be expected to be present in the system: pnpm, uv, mise, git, openspec are all expected.
3. These crates express interfaces to manage hardware resources via the kernel module for fslmc so performance and correctness must be by construct. Rust is the language of choice and the code base should follow 2026 best common practices including clippy, fmt, docs, unit tests, and integration tests.
4. This repository follows sans-io/hexagonal approach to its software architecture. The core crates are low in dependency numbers and opinionated about the structure of the business problem of managing the DPAA2 management complex, and fan out to the best library to implement them or wire the tool up in a concrete ecosystem of tools.
5. This tool covers a developer experience space and backtesting through TDD and invariant definition is a goal each spec and its implementation must define and attain.

## Structure

This rust monorepo stores the following crates/:

- dpaa2-api: the domain model of network objects, the trait seams, and the pure reconcile functions to solve and dispatch on using a sans-io and hexagonal architecture approach
- dpaa2-mc: southbound adapter that drives the MC objects over the restool shim and the fsl-mc sysfs bus; a future ioctl portal drops in behind the same traits
- dpaa2-config: northbound adapter that parses the declarative topology intent into the backend-neutral model
- dpaa2-tools: customer frontend and imperative shell that drives convergence via CLI, TUI, etc.

The repository also contains:
- docs/adr/: numbered decision records that track the current agreement as to what the library and crates target, what do not target, and its parts to do so
- docs/ROADMAP.md: contains the current design and execution roadmap for all the features planned and their state is tracked in openspecs
- README.md: succinct and clear presentation of the problem, the solution and an example of use
- CHANGELOG.md: semver-style description of the sequence of changes and entirely managed with cliff automatically

## Team standards and conventions

- Code favors clarity and long-term thinking: the interface is modular and abstract and traits, generics, and macros are favored to keep business structure from details
- Specs are documented with annotations anchored to ADRs.
- Code is documented with annotations tagged to ADRs.
- Explanations are active voice and 3rd person and succinct.
- Unit and integration tests follow rust idioms and are separate.
- TDD is used to drive spec tasks and part of the later.
- Development workflow is complete when `cargo build | fmt | clippy | clippy --tests | doc` all pass
- Git commits messages are conventional and include assistance authorship, reference to spec phase, and describe why the change takes place succinctly and simple words
- Git commit content is self standing, it is amendable until sealed, and branches may be forked off to help reordering commits for independent review
- docs/{adr,ROADMAP.md} are always kept up to date after changes
- Tables that restate rules from a source of truth are linted (ADR-0014); a change that adds or edits such a table extends the lint or files its bead in the same change.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
