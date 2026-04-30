# Internal procedures

## Repositories

Almost all the development of Slint is done in the https://github.com/slint-ui/slint mono-repository.

There are a few other repositories in the organization, they are either
 - Template repositories which are on their own as user are invited to use them as templates for their own projects
 - Forks of 3rd party repository which member of the slint organization have made pull requests
 - Experiments or short lived projects or example that are not part of the product
 - Private repositories used for specific customer projects
 - Website repository
 - Other repositories for code that needs to be kept private as it is part of a proprietary product

All the code of the Slint product is in the mono-repository.
Having a single mono-repository makes it easier to do changes across the whole product

Different files in the Slint repository have different licenses, and all the license are tracked with [REUSE](https://reuse.software/).
 - Most files have headers that are compatible with the [SPDX](https://spdx.org/licenses/) license identifiers
 - Otherwise the license is tracked in the REUSE.toml in the root of the repository

## Organization members

The administrators of the slint-ui Github organization are the founders of SixtyFPS GmbH.
All employees of SixtyFPS GmbH are members of the organization.
External contributors may also be invited to become members of the Github organization.

## Code Changes

We aim for a clean, readable history on `master` so that `git blame` and `git bisect` remain useful.
Commit messages should make sense on their own,
even without access to the pull request that introduced them.

The commit and PR title should be short and describe *what* changed.
Start with the area of the code it affects,
for example `compiler:`, `slintpad:`, `skia:`, `docs:`.
Don't use the `feat:`/`fix:` convention.

The commit body can be longer, expand on *what* was done, and explain *why* and *what it fixes*.
Reference related issues in the body:
use `Fixes: #123` or `Closes: #123` if the commit resolves the issue,
or `Issue: #123` / `CC: #123` to reference it without closing it.

Changes to user-visible behavior should include an update to the documentation.
Ideally, every change should come with an automated test.

### Branches

Developers can create a branch `<developer-name>/<feature-name>` in the slint-ui/slint repository.
Organization members are encouraged to use this naming convention rather than working from personal forks,
as it keeps branches discoverable and triggers CI automatically.
Outside contributors need to create a PR from a branch in their own fork,
since only organization members can push to `slint-ui/slint`.

The `master` branch and `feature/*` branches are protected.
Members of the "trusted-with-git" GitHub team can push directly to these branches.
Admins are implicitly part of this group.
For simple commits that don't need review and are unlikely to break CI,
a trusted-with-git member can push directly to `master` without creating a PR.

### Pull Requests

A PR should be reviewed before merging, unless the change is trivial.

Reviewers leave comments on the PR.
Some are nitpicks, others must be addressed before merging.
Reviewers should clearly indicate which is which.

Once approved, the author merges the PR.
GitHub's auto-merge feature can be used to merge automatically once CI passes and approvals are in.
For external contributions, the reviewer can also merge it.

Use draft PRs for work that isn't ready for review yet.
When you mark a PR as ready, let the reviewer know — GitHub doesn't notify them loudly.

### Commit Hygiene

Many PRs accumulate "fixup" or "autofix" commits during review. These are noise in the final history.

For small changes, **squash and merge** the PR.
Edit the commit message to produce a clean, self-contained summary that follows the guidelines above.
Remove any leftover fixup or autofix commit titles from the message.

For larger changes with several logically independent parts,
maintain a clean history of a few self-contained commits within the PR.
In this case, force-push to the PR branch as needed to keep the history tidy,
and use **rebase and merge** so the individual commits land on `master`.

When in doubt, squash.
A single well-written commit is better than a noisy sequence.

### Feature Branches

Feature branches are for larger efforts where partial, incremental reviews are useful.
They follow the `feature/<name>` naming convention.

#### Creating a Feature Branch

A trusted-with-git member creates the feature branch:

```sh
git push origin origin/master:feature/<name>
```

#### Working on a Feature Branch

Development on a feature branch follows the same PR and review workflow as `master`,
but PRs target the feature branch instead.
Temporary regressions are acceptable as long as they're tracked in the tracking PR (see below).

Once a PR has been merged into the feature branch, don't rewrite that history.
Don't force-push, squash, or rebase commits that were already reviewed and merged.
The commit hashes are the link back to the PR that introduced them.

#### Merging Master Into the Feature Branch

Only merge `master` into the feature branch when something from `master` is actually needed —
for example to resolve conflicts or to pick up a specific commit.
Don't merge routinely.

A trusted-with-git member performs the merge:

```sh
git switch feature/<name>
git pull origin feature/<name>
git merge origin/master
# resolve any conflicts and commit
git push origin feature/<name>
```

Resolving merge conflicts requires understanding the changes on both sides.
If you're unsure, ask for help.

#### Tracking PR

Create a **draft** PR that merges the feature branch into `master`.
This tracking PR serves as the central place for the feature:

 - Give it a descriptive title and a label.
 - Use the PR description to list remaining work items, known regressions, and open questions.
 - Keep the description up to date as the feature progresses.

Don't merge the tracking PR through GitHub's merge button.

#### Completing a Feature Branch

When the author and reviewers agree the feature is complete,
a trusted-with-git member pushes the merge commit to `master`:

```sh
git switch master
git pull origin master
git merge origin/feature/<name>
git push origin master
```

GitHub automatically closes the tracking PR once its head commit is part of `master`.
Delete the feature branch after merging.

## CI

The CI is driven by Github Actions.
The CI is triggered for any PR (including drafts) to the `master` branch, or to a branch that starts with `feature/`.
The CI is also triggered for every push to the `master` branch.
A new commit on a branch will cancel the previous CI run on that branch if it is still running.

## Nightly build

There is a nightly build that runs every night.
It will build release artifacts and upload them to the `nightly` Github release.
And will also generate the docs, wasm build of examples, and publish them on snapshots.slint.dev/master

It will also run a few extra tests (like running `cargo update` to check for broken dependencies).

## Release

New minor releases of Slint are done every couple of months.

The release process is described in <./release.md>

## Issues

Users are reporting bugs and feature requests on the [Github issue tracker](https://github.com/slint-ui/slint/issues).

The process to triage and assign label is documented in the <./triage.md> file.

## Long-Term Planning

For long-term planning, we primarily use two processes: Initiatives and Project Boards.

### Initiatives

Initiatives describe long-term goals that we as a team want to work towards.

Writing these goals down as an initiative benefits us in several ways:


- Allows us to prioritize issues based on current initiatives
- Lets us discuss priorities and bring in fresh ideas from everyone

All Slint Members can propose Initiatives in Outline.
Ideally, a proposal should address:

- **Ownership & Resources:** Who owns the initiative? Who works on it?
- **Strategic Alignment Check:** Does the initiative fit our vision?
- **Clarity & Scope:** Clearly define the initiatives scope.
- **Impact & Effort:** Is the target outcome worth it in terms of cost (work time and materials)
- **Success Criteria:** When is the initiative considered "done"?
- **Milestones:** Outline the high-level steps needed to achieve the success criteria
- **Process:** Any specific process to follow
    - Template:
        ```
        - Approved Initiative is added to the roadmap with info owner/timeline/key results
          - Priority is based on urgency, impact, and capacity.
        - Owner tracks progress towards key results. Updates shared weekly.
        ```

Initiatives are discussed by the team, reworked if necessary, and finally accepted or rejected.

Once an initiative has been accepted, the owner should migrate it to a Github issue. For this, use either the ["Tracking Issue"](https://github.com/slint-ui/slint/issues/new?template=3-tracking-issue.md) template, or assign the "roadmap" label to an existing issue. Add any corresponding sub-issues if needed.

The issue will be automatically added to the "Team Planning" Board when it receives the "roadmap" label.

### Project Boards

We encourage the use of Github Project Boards to organize tasks.

To track long-running tasks, we use a private "Team Planning" Github Project board.
When Members of the Slint organization work on long-term goals, they should make sure that they are assigned to a corresponding issue on this board.
Maintaining this board allows us to get an overview of who is working on which topics at any given time and to better plan long-term.
We check this board during our weekly meeting to see if it is still up-to-date.

Larger initiatives and tasks may also benefit from their own project boards.
We leave it up to each individual project owner to decide whether they want to organize their tasks using additional Project boards.
