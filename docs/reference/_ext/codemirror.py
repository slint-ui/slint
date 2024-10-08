# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import hashlib
from docutils import nodes
from docutils.parsers.rst import directives
from sphinx.util.docutils import SphinxDirective

class CodeMirrorNode(nodes.General, nodes.Element):
    pass

class CodeMirrorDirective(SphinxDirective):
    has_content = True
    required_arguments = 0  # 0 means argument is optional
    optional_arguments = 1  # Allows one optional positional argument
    option_spec = {
        'language': directives.unchanged,
        'ignore': directives.flag,
        'no-preview': directives.flag,
    }

    def run(self):
        # If there's an argument, split it by commas
        args = self.arguments[0].split(',') if self.arguments else []

        # The first argument (if present) is assumed to be the language
        language = args[0].strip() if len(args) > 0 else None

        # Handle additional arguments like "nopreview"
        additional_args = [arg.strip() for arg in args[1:] if language] if language else args

        # Check if 'nopreview' was included in the additional arguments
        nopreview = 'no-preview' in additional_args
        ignore = 'ignore' in additional_args

        content = '\n'.join(self.content)
        
        # Generate a unique ID for the editor (e.g., using a hash of the content)
        content_hash = hashlib.md5(content.encode('utf-8')).hexdigest()[:8]
        editor_id = f"codemirror-editor-{content_hash}"

        # Create a CodeMirrorNode with the ID, language, and content
        node = CodeMirrorNode()
        node['content'] = content
        node['editor_id'] = editor_id
        node['language'] = language
        node['ignore'] = ignore
        node['no-preview'] = nopreview
        return [node]

def visit_codemirror_node(self, node):
    language = node['language']
    content = node['content']
    editor_id = node['editor_id']
    ignore = node['ignore']
    nopreview = node['no-preview']
    readonly = 'true'

    # Create a container for the editor with a unique ID
    if language == 'slint':
        readonly = 'false'
    
    self.body.append(f'<div id="{editor_id}" class="codemirror-editor" data-lang="{language}" data-readonly="{readonly}" data-ignore={ignore} data-nopreview={nopreview}><div class="codemirror-content" style="display:none">{content}</div></div>')

def depart_codemirror_node(self, node):
    pass

def setup(app):
    app.add_node(CodeMirrorNode, html=(visit_codemirror_node, depart_codemirror_node))
    app.add_directive('codemirror', CodeMirrorDirective)
