<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->

# tree-sitter support for SLint

> Tree-sitter is a parser generator tool and an incremental parsing library. It
> can build a concrete syntax tree for a source file and efficiently update the
> syntax tree as the source file is edited.

                                                (taken from tree-sitter page)

Use with vim/helix/... other editors.

## Inject into Rust

This tree-sitter configuration can be injected into rust, so that the `slint!`
macro gets highlighted.

In `neovim` with the `nvim-treesitter` plugin this is done with the

`:TSEditQueryUserAfter injections rust` to create/edit the rust injection
configuration. Copy and paste this into the new file:

```tree-sitter
;; Inject the slint language into the `slint!` macro:
(macro_invocation
  macro: [
    (
      (scoped_identifier
        path: (_) @_macro_path
        name: (_) @_macro_name
      )
    )
    ((identifier) @_macro_name @macro_path)
  ]
  ((token_tree) @injection.content
  (#eq? @_macro_name "slint")
  (#eq? @_macro_path "slint")
  (#offset! @injection.content 0 1 0 -1)
  (#set! injection.language "slint")
  (#set! injection.combined)
  (#set! injection.include-children)
  )
)
```

Please send PRs when you find out how to do the same with other editors.
