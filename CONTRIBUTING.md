# Contributing

Want to help with Scribe? Nice. Keep it practical.

## Before you open a PR

- open an issue first if the change is big, risky, or changes app behavior in a non-obvious way
- if it's a small fix, just send the PR
- if you're forking it to take the project in a different direction, that's fine too

## What good contributions look like

- small focused diffs
- clear commit messages
- no drive-by refactors unless they are required for the fix
- no comment spam. prefer better names and smaller functions
- if you add a comment, explain the constraint or trade-off, not the syntax

## Dev setup

```bash
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
npm --prefix frontend install
wails dev
```

## Before you submit

```bash
wails build
go test ./...
```

If you touch release workflows or packaging, say that clearly in the PR body.

## Style

- write code like a maintainer has to live with it for a year
- prefer the smallest correct change
- avoid filler comments
- keep docs direct and honest

## Issues

Good bug reports include:

- what you tried
- what happened
- what you expected
- OS and app version
- screenshots or logs if the UI broke

## PRs

- explain the user-visible change first
- call out trade-offs and follow-up work
- keep AI-looking boilerplate out of the description

That's it. Make it easier to maintain, not harder.
