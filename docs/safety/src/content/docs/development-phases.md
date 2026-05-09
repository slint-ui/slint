---
title: Development Phases
description: Development phases for Slint SC.

---

# Development Phases (ISO 26262-4)

# Phase 1: Ticket Approval

A ticket, also known as a Github Issue, should describe the work necessary to be done. The ticket should also contain the motivation for the change, and a proposed solution.

The ticket should detail the impact of the change on the safety of the system.

Once the ticket has been scoped and approved, Code Development may begin.

# Phase 2: Code Development

During this phase, the developer will implement the changes described in the ticket.

(TODO: add naming convention of git branches here?)

After work has reached the point where it should be reviewed by others, the developer will create a Pull Request (PR).

After each PR is created, or after each new commit is pushed to that branch, the CI/CD pipeline will run, and the developer can see the results of the Regression Tests.

# Phase 3: Code Review

The master branch is protected, only admins are able to push directly to the `master` branch.

Any Pull Request (PR) on Slint SC must be approved by a reviewer before being merged.

The full set of Regression Tests run by CI must pass on the merge commit between the PR and the base branch. The merge commit must be the one being fast-forwarded on the base branch. Testing a merge commit and then merging into the base branch with a different merge commit (for example if other changes were merged in the meantime) does not count.

Reviewers can leave comments on the PR. Some comments are just nitpicks but some other comments should be addressed before merging the PR. Reviewers should make an effort to clearly indicate what needs to be addressed to obtain approval.

Once approved, the author of the PR can merge the PR if he has the rights to do so.
For external contributions, the reviewer must merge the PR.

# Phase 4: Testing

We have a set of tests that are executed as part of this phase.
These tests can be run locally using the "cargo test" command.
They are described in more detail here: [Tests](/qualification-plan/test-cases/).





