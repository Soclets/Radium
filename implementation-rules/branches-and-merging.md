# Branches and merging

From D45 and `CLAUDE.md`.

1. A session commits to **its own branch** (e.g. `compiler/termination-check`), never to `main`, and pushes that
   branch.
2. A **local merge bot** on the owner's Mac notices the new commit, merges it with the latest `main` in a separate
   scratch folder, and runs the **full** test suite there.
3. It reports the result on the commit through GitHub's commit-status API (`radium/full-tests: passed` / `failed`,
   with a summary). The tick means the whole suite passed, nothing less.
4. Passed: the bot moves `main` forward and pushes it. Failed: `main` is untouched; the branch is fixed and pushed
   again.

- The bot is the only thing that merges into `main`. No pull requests, no GitHub Actions, no self-hosted runner.
- **Commit and push only when the owner says so.**
- Each session signs its commits with its own house name, `<house name>, session <session title>` (see `CLAUDE.md`).
- Takes effect with the first code; until then the design session commits and pushes when the owner says so.
