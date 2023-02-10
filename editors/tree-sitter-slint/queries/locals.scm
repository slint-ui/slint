; Copyright © SixtyFPS GmbH <info@slint-ui.com>
; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

; Functions and callbacks
(typed_identifier name: ((_) @definition.parameter))

; definitions
(callback name: ((_) @definition.method) (#set! "definition.method.scope" "parent"))
(function name: ((_) @definition.method) (#set! "definition.method.scope" "parent"))
(property name: ((_) @definition.var) (#set! "definition.var.scope" "parent"))
(component id: ((_) @definition.var) (#set! "definition.var.scope" "parent"))
(global_definition name: ((user_type_identifier) @definition.type) (#set! "definition.method.scope" "parent"))
(struct_definition name: ((user_type_identifier) @definition.type) (#set! "definition.type.scope" "global"))
(struct_definition (type_anon_struct name: ((_) @definition.var)))

(block) @scope
