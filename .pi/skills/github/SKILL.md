---
name: github
description: Interact with GitHub using the gh CLI. Issues, pull requests, releases, and actions. Uses the rust-ui repository (vincent-uden/rust-ui) by default.
---

# GitHub Skill

Commands for interacting with GitHub. The default repository is `vincent-uden/rust-ui`.

## Authentication

Check authentication status:
```bash
gh auth status
```

Log in to GitHub:
```bash
gh auth login
```

## Repository

View repository information:
```bash
gh repo view vincent-uden/rust-ui
```

View remote branches:
```bash
git remote -v
```

## Issues

List issues (open by default):
```bash
gh issue list
```

List all issues including closed:
```bash
gh issue list --state all
```

View a specific issue:
```bash
gh issue view <issue-number>
```

Create a new issue:
```bash
gh issue create --title "Issue title" --body "Issue description"
```

Create issue with labels:
```bash
gh issue create --title "Bug" --body "Description" --label bug
```

Close an issue:
```bash
gh issue close <issue-number>
```

Reopen an issue:
```bash
gh issue reopen <issue-number>
```

List available labels:
```bash
gh label list
```

## Pull Requests

List pull requests (open by default):
```bash
gh pr list
```

List all PRs including closed:
```bash
gh pr list --state all
```

View a specific PR:
```bash
gh pr view <pr-number>
```

View PR diff/review:
```bash
gh pr view <pr-number> --json title,body,files,additions,deletions,changedFiles
```

Create a new PR from current branch:
```bash
gh pr create --title "PR title" --body "PR description"
```

Create PR with review request:
```bash
gh pr create --title "Feature" --body "Description" --reviewer username
```

Checkout a PR locally:
```bash
gh pr checkout <pr-number>
```

Close a PR:
```bash
gh pr close <pr-number>
```

Merge a PR (will prompt for merge method):
```bash
gh pr merge <pr-number>
```

Merge with specific method:
```bash
gh pr merge <pr-number> --squash
gh pr merge <pr-number> --rebase
gh pr merge <pr-number> --admin --merge
```

View PR checks status:
```bash
gh pr checks <pr-number>
```

View PR reviews:
```bash
gh pr view <pr-number> --json reviews
```

Add review (approve, request changes, comment):
```bash
gh pr review <pr-number> --approve
gh pr review <pr-number> --request-changes --body "Feedback"
gh pr review <pr-number> --comment --body "Review comment"
```

## Releases

List releases:
```bash
gh release list
```

View latest release:
```bash
gh release view
```

View specific release:
```bash
gh release view <tag>
```

Create a release:
```bash
gh release create <tag> --title "Release Title" --notes "Release notes"
```

Upload assets to a release:
```bash
gh release upload <tag> <path-to-asset>
```

Delete a release:
```bash
gh release delete <tag>
```

Delete a release tag:
```bash
gh release delete <tag> --yes
```

## Actions

List workflows:
```bash
gh run list
```

List recent workflow runs:
```bash
gh run list --limit 10
```

View workflow run status:
```bash
gh run view <run-id>
```

View run logs:
```bash
gh run view <run-id> --log
```

Rerun a workflow:
```bash
gh run rerun <run-id>
```

Cancel a running workflow:
```bash
gh run cancel <run-id>
```

View workflow runs for a PR:
```bash
gh run list --branch <branch-name>
```

## Searching

Search issues/PRs:
```bash
gh search issues "rust-ui" --owner vincent-uden
```

Search code:
```bash
gh search code "function_name" --owner vincent-uden
```

## Gists

List gists:
```bash
gh gist list
```

View gist:
```bash
gh gist view <gist-id>
```

Create gist:
```bash
gh gist create <file>
```

Create secret gist:
```bash
gh gist create --public false <file>
```

## Useful Aliases

You can create gh aliases in your shell config:

```bash
# Add to .bashrc or .zshrc
alias gi='gh issue'
alias gp='gh pr'
alias gr='gh run'
```

## Common Options

- `-R owner/repo` - Specify repository (defaults to current repo)
- `--json` - Output in JSON format for scripting
- `--limit` - Limit number of results
- `--state` - Filter by state (open, closed, all)
- `--jq` - Filter JSON output with jq syntax
