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
    required_arguments = 0  # We require one argument: the programming language
    option_spec = {
        'language': directives.unchanged,
    }

    def run(self):
        language = self.arguments[0]
        content = '\n'.join(self.content)
        
        # Generate a unique ID for the editor (e.g., using a hash of the content)
        content_hash = hashlib.md5(content.encode('utf-8')).hexdigest()[:8]
        editor_id = f"codemirror-editor-{content_hash}"

        # Create a CodeMirrorNode with the ID, language, and content
        node = CodeMirrorNode()
        node['content'] = content
        node['editor_id'] = editor_id
        return [node]

def visit_codemirror_node(self, node):
    language = node['language']
    content = node['content']
    editor_id = node['editor_id']

    # Create a container for the editor with a unique ID
    self.body.append(f'<div id="{editor_id}" class="codemirror-editor" data-lang="{language}" style="height: 300px;"><div class="codemirror-content" style="display:none">{content}"</div></div>')

def depart_codemirror_node(self, node):
    pass

def setup(app):
    app.add_node(CodeMirrorNode, html=(visit_codemirror_node, depart_codemirror_node))
    app.add_directive('codemirror', CodeMirrorDirective)
