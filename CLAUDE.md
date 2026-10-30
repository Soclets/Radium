# Radium

Design repository for **Radium**, a programming language for final products that cannot have any mistakes. Start
with `DECISIONS.md` (every decision, numbered), `TERMS.md` (one name per concept) and `design-rules/` (the general
rules every new choice is checked against).

## Ground rules

- **Design authority is the owner's.** Claude sessions propose and recommend; the owner decides syntax, naming,
  semantics and architecture. Never fill in a default for an open decision.
- **One question at a time, in plain chat text.** Never use the AskUserQuestion tool or other popups: they don't
  always appear.
- **Record each decision in `DECISIONS.md` as it is made**, noting whether the owner designed it or said yes to a
  proposal. Numbers are permanent (code cites them with `trace D<n>`, D18). New concepts go into `TERMS.md`. When a
  decision creates or changes a general rule, update its file in `design-rules/` too.
- **Commit and push only when the owner says so.**

## Sessions

Each Claude session has its own house name, so commits show which session did the work rather than the model's
name (owner, 2026-10-29: otherwise "other Claudes would become the same name as you").

- **Design session** — house name "radium design claude", desktop session "Radium language design", working in
  `/Users/ts/Radium Design`. Commits end with:
  `Co-Authored-By: radium design claude, session Radium language design <noreply@anthropic.com>`

A new session gets its own name, in the same shape (`<house name>, session <session title>`), added here before its
first commit.
