import { op_load_slint } from "ext:core/ops";
function load_slint() {
    op_load_slint();
}

globalThis.Extension = { load_slint };
