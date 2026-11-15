# Slint Grammars

This folder holds the source of the Slint syntax highlighting grammars:

- `words.json` lists the keywords, types, and other words that the grammars share.
- `slint.tmLanguage.template.json` is the TextMate grammar.
- `highlightjs-slint.template.js` is the highlight.js grammar.

In both templates, each `@NAME@` stands for the `name` list of `words.json`, joined with `|`.

Edit `words.json` to add a keyword or a type.
The Monaco grammar of SlintPad imports it directly.
The tests check that the Sublime grammar in `editors/sublime` lists the same words.

Run `pnpm build` to write the grammars to `dist/`, and `pnpm test` to check them against the grammar tests in `editors/vscode/tests/grammar/`.
The VS Code extension, the documentation, and the Figma inspector build the TextMate grammar themselves.
For other editors and tools, a nightly job builds it into the [slint-tmLanguage](https://github.com/slint-ui/slint-tmLanguage) repository.

## highlight.js

We publish the highlight.js grammar as `@slint-ui/highlightjs-slint`.
In a browser, load it after highlight.js, and it registers the `slint` language:

```html
<script src="https://cdn.jsdelivr.net/npm/@slint-ui/highlightjs-slint/dist/highlightjs-slint.js"></script>
```

With a bundler or in Node.js, register it yourself:

```js
import hljs from "highlight.js/lib/core";
import slint from "@slint-ui/highlightjs-slint";

hljs.registerLanguage("slint", slint);
```
