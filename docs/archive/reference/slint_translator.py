# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""A custom Sphinx HTML Translator for adding CodeMirror to Slint Docs
"""

from docutils import nodes
from sphinx.writers.html5 import HTML5Translator

class SlintHTML5Translator(HTML5Translator):
    def visit_literal_block(self, node):
        """
        Override the visit_literal_block method from HTML5Translator to add CodeMirror Editor.
        """
        if node.rawsource != node.astext():
            # most probably a parsed-literal block -- don't highlight
            return super().visit_literal_block(node)

        # Extract the raw code from the node
        content = node.rawsource

        # Extract the language
        lang = node.get('language', 'default')
        args = [arg.strip() for arg in lang.split(',')]
        data_args = []
        # Check if the first argument is 'slint'
        if args and args[0] == 'slint':
            data_args.append('data-readonly="false"') 
            if 'ignore' in args:
                data_args.append('data-ignore="true"')
            if 'no-preview' in args:
                data_args.append('data-nopreview="true"')
        else:
            data_args.append('data-ignore="true"')  # Set ignore if not slint

        data_args_string = ' '.join(data_args)

        # Assign an id
        docname_with_hyphens = self.builder.current_docname.replace('/', '-')
        editor_id = f"editor-{docname_with_hyphens}-{node.line or 0}"

        # Insert the custom HTML for the code block
        self.body.append(
            f'<div id="{editor_id}" class="codemirror-editor" '
            f'data-lang="{lang}" {data_args_string}>'
            f'<div class="codemirror-content" style="display:none">{self.encode(content)}</div>'
            '</div>'
        )

        raise nodes.SkipNode
