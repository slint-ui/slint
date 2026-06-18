# Writing Style Guide

This guide applies to everything we write:

- code comments and documentation comments (internal and public API)
- repository documentation and the documentation website
- blog posts and posts on social media

Some guidance is universal; the rest depends on the audience.
The sections below are organized that way: apply the universal principles everywhere, then the section that matches what you're writing.

## Universal Principles

Whatever you write, aim to be:

- **Concise** — people skim, so cut words that don't earn their place.
- **Clear** — simple terms and short sentences reach the widest audience.
- **Direct** — give the instruction or fact straight, without hedging.

Concretely:

1. Offer direct advice.
   - Avoid: "Please install XYZ ..."
   - Use: "Install XYZ"
   - Rationale: The reader is here for instructions, there's no need to beat around the bush.
2. Write actionable.
   - Avoid: "Element XYZ makes it possible to set the background color."
   - Use: "Use element XYZ to set the background color."
   - Rationale: Shorter, straight to the point.
3. Don't shout.
   - Avoid: "Try out XYZ!"
   - Use: "Try out XYZ."
   - Rationale: Use exclamation points sparingly and save them for when they really count; we already have the reader's attention.
4. In Markdown and doc comments, put each sentence on its own line, or break after a comma when a line gets long.
   - Rationale: Just like a newline after `;` in code, this keeps diffs readable and avoids reflowing a whole paragraph for one edit.
5. Use American English spelling.
   - Rationale: Slint's API uses American spelling (such as `color`), so the rest of our writing matches.

## Code Comments

For comments in source code — both internal implementation notes and public API documentation comments:

1. Describe what the code *is* and why, in the present tense.
   - Rationale: The comment should make sense to whoever reads the code next; what changed belongs in the commit message, not the source.
2. For public items, document the interface, not the implementation.
   - A comment above a property, function, or type says what it does and why for the caller, not how it works inside.
   - Rationale: Callers shouldn't have to read the implementation, and implementation details in the comment go stale as the code evolves.

## Documentation, Blog, and Social

For the documentation website, blog posts, and social media we also aim to sound like a small, human company rather than a corporation:

1. Use contractions.
   - Avoid: "We are proud to announce ..."
   - Use: "We're proud to announce ..."
   - Rationale: Makes for a conversational, human tone.
2. Use Title Case for headings.
3. Use active voice for things *we* did.
   - Avoid: "The foo widget got revamped."
   - Use: "We revamped the foo widget."
   - Rationale: We're announcing the result of our work, not watching it from the audience.
4. Write from the user's perspective — emphasize the outcome they gain, not the product change.
   - Avoid: "Slint adds feature X."
   - Use: "Achieve Y with the new X feature in Slint."
   - Rationale: Users care how a feature helps them reach a goal, not just that it exists.

### Docs

- Make sure links resolve — don't point at blank or moved pages.

### Tab Order

In the documentation website, order the items of a `<Tabs>` block consistently.

For `syncKey="dev-language"`:

- Rust
- C++
- NodeJS
- Python

For `syncKey="dev-platform"`:

- Windows
- macOS
- Linux
- Android
- iOS
