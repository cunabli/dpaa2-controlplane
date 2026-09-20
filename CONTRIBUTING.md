# Contribution guidelines

First off, thank you for considering contributing to dpaa2-controlplane.

If your contribution is not straightforward, please first discuss the change you
wish to make by creating a new issue before making the change.

## Reporting issues

Before reporting an issue on the
[issue tracker](https://github.com/cunabli/dpaa2-controlplane/issues),
please check that it has not already been reported by searching for some related
keywords.

## Pull requests

Try to do one pull request per change.

### Updating the changelog

Update the changes you have made in
[CHANGELOG](https://github.com/cunabli/dpaa2-controlplane/blob/main/CHANGELOG.md)
file under the **Unreleased** section.

Add the changes of your pull request to one of the following subsections,
depending on the types of changes defined by
[Keep a changelog](https://keepachangelog.com/en/1.0.0/):

- `Added` for new features.
- `Changed` for changes in existing functionality.
- `Deprecated` for soon-to-be removed features.
- `Removed` for now removed features.
- `Fixed` for any bug fixes.
- `Security` in case of vulnerabilities.

If the required subsection does not exist yet under **Unreleased**, create it!

## Commit hooks

The tracked hooks in `.githooks/` enforce commit mechanics and two repo rules.
Point git at them once per clone, and register the ITF trace clean filter so
volatile `#meta` timestamps stay out of `git status`/`git add`:

```shell
git config core.hooksPath .githooks
git config filter.itfmask.clean "node scripts/hooks/mask-itf.mjs"
```

Both settings, like `core.hooksPath`, live in the clone's local config and are
not checked in — every clone and CI runner registers them from this block.

The `commit-msg` hook checks each commit message:

- Title is `<area>: <summary>` with a lowercase area prefix, at most 72 chars.
- A blank line separates the title from the body.
- Body lines stay within 72 chars (URLs and trailers exempt).
- The body is at most 15 substantive lines — detail belongs in the ADR/spec/bead.
- Exactly one `Change: <slug>` and one `BeadId: dpaa2-controlplane-<id>` trailer.
- The referenced bead is already closed (close-then-commit).
- `.beads/issues.jsonl` is staged when the close changed it.
- One writer per crate per task: a commit touches at most one `crates/<name>`.

The `pre-commit` hook runs fast staged-source lints; among them:

- CHANGELOG.md is cliff-owned: staging an edit to it fails, since git-cliff generates it at release time.
- A public-repo leak scan rejects added lines carrying a real IP or MAC address or a board-vendor brand name.

The beads hooks still run; `.githooks/` delegates to them. Use
`git commit --no-verify` to bypass the checks.

## Developing

### Set up

This is no different than other Rust projects.

```shell
git clone https://github.com/cunabli/dpaa2-controlplane
cd dpaa2-controlplane
cargo test
```

### Useful Commands

- Build and run release version:

  ```shell
  cargo build --release && cargo run --release
  ```

- Run Clippy:

  ```shell
  cargo clippy --all-targets --all-features --workspace
  ```

- Run all tests:

  ```shell
  cargo test --all-features --workspace
  ```

- Check to see if there are code formatting issues

  ```shell
  cargo fmt --all -- --check
  ```

- Format the code in the project

  ```shell
  cargo fmt --all
  ```
