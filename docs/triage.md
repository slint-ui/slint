<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
## Introduction

This document will outline the process of triaging GitHub issues and provide an explanation of GitHub labels.

## GitHub Labels

Labels that start with `a:` are area labels.
Area labels help categorise issues based on their scope. Each issue should ideally have at least one area label.
The description of the area label ends with a code that indicates the maintainer or backup maintainer of that area.
For example, if a label description ends with `(mX,bY)`, it means person X is the maintainer, and person Y is the backup maintainer for that area.

Apart from area labels, there can be other labels used to provide additional context to issues.

- **Bug**: Indicates that the issue is a bug or a software defect.
- **Enhancement**: Indicates that the issue suggests an improvement or new feature.
- **Good First Issue**: Indicates that the issue is suitable for newcomers to contribute to the project.
   The issue should have a good description and ideally a comment to describe how to implement it.
- **Needs More Info**: Indicates that the issue lacks sufficient information to be able to act on it right now and requires additional details from the reporter.
  The maintainer for the area must remove the tag when more info is provided, or close it if it is too old.

## GitHub Assignee

Here are some guidelines for GitHub assignees:
- Assign issues to individuals who are actively working on them or going to actively work on them in the near future.
- Avoid assigning too many issues to a single person to prevent overload.
- Assign an issue to yourself if you plan to work on it to inform others that the issue is being addressed.
- Assign an issue to someone else if you expect a quick action or if someone else is better suited to handle it.
- Unassign issues that you are not actively working on to allow others to pick them up.

## Issue Description

A clear and concise issue description is crucial for effective communication.
- Edit the title and description of the issue to ensure clarity and conciseness.
- If modifications have been made to the original issue, indicate it by adding "Edited:" to the description.
- Consider adding "acceptance criterial" so that it's easy to understand what needs to be accomplished in order to close the issue as done.

## Triage Process

1. The first person who identifies an issue can assign labels to it. Make sure to include at least one area label.
2. Individuals responsible for the specific area should regularly check issues associated with their labels.
   - Thank the reporter for reporting the issue.
   - Assess the severity and other relevant factors.
   - Try to reproduce the issue and request additional information if necessary.
3. Ensure that most bugs are fixed before the release, unless they are low priority.
4. Respond to issues reasonably fast. Ideally, within one business day.

Remember, effective communication and prompt action are essential for successful issue triaging.

## Filters

Use the following scripts to generate the filters.

Filter for all the issues not assigned to an area:

```sh
curl -H 'Accept: application/vnd.github.v3+json' "https://api.github.com/repos/slint-ui/slint/labels?per_page=100&page=1" | jq -r '.[].name'  | grep "^a:" | sed 's/^\(.*\)$/-label:\\\"\1\\\"/' | xargs echo
```

Filter of all issues for which X is a maintainer  (replace the X in `"mX"` with the right letter name, or `bX` for the backup)

```sh
curl -H 'Accept: application/vnd.github.v3+json' "https://api.github.com/repos/slint-ui/slint/labels?per_page=100&page=1" | jq -r '.[] | select(.description | contains("mX")) | .name' | awk '{printf "\"%s\",", $0}' | sed 's/^\(.*\),$/label:\1\n/'
```
