# Branches and merging

From D45, D89, D90 and `CLAUDE.md`.

**The design session controls `main`** (D89): it commits straight to `main`, the oracle included, when the owner
says so. Every other session follows the flow below.

1. A session commits to **its own branch** (e.g. `compiler/termination-check`), never to `main`, and pushes that
   branch.
2. A **local merge bot** on the owner's Mac notices the new commit, merges it with the latest `main` in a separate
   scratch folder, and runs the **full** test suite there.
3. It reports the result on the commit through GitHub's commit-status API (`radium/full-tests: passed` / `failed`,
   with a summary). The tick means the whole suite passed, nothing less.
4. Passed: the bot moves `main` forward and pushes it. Failed: `main` is untouched; the branch is fixed and pushed
   again.

- The bot is the only thing that merges other sessions' branches into `main`. No pull requests, no GitHub Actions, no self-hosted runner.
- **Commit and push only when the owner says so.**
- Each session signs its commits with its own house name, `<house name>, session <session title>` (see `CLAUDE.md`).
- The bot takes effect with the first code from a session other than the design session.

## Commit messages (D90)

```
kind.folder.narrowing: sentence (who decided; D<n>)

Optional body: what was done and why, wrapped.

Trace: D<n>, …
Co-Authored-By: <house name>, session <session title> <noreply@anthropic.com>
```

- **Kinds:** `decision`, `design`, `feature`, `fix`, `test`, `house`.
- **Second word:** the folder (`oracle`, `curie`, `geiger`, `std`, …), or the topic for `decision`, `design` and
  `house` (`decision.ownership`, `house.readme`, `house.brand`).
- **Third word** optional; never a fourth.
- **`Trace:`** whenever code carries out decisions.
