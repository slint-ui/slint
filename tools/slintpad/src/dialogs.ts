// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

export function modal_dialog(
    extra_class: string,
    content:
        | HTMLElement[]
        | ((_is_ready: (_ready: boolean) => void) => HTMLElement[]),
    trigger_text = "OK",
    trigger_action = () => {
        /**/
    },
    close_action = () => {
        /**/
    },
) {
    const dialog = document.createElement("dialog");
    dialog.classList.add("dialog");
    dialog.classList.add("modal");
    dialog.classList.add(extra_class);

    const titlebar = document.createElement("div");
    titlebar.classList.add("titlebar");

    const close_i = document.createElement("i");
    close_i.classList.add("close_button");
    close_i.classList.add("fa");
    close_i.classList.add("fa-times");
    close_i.onclick = () => dialog.close();

    titlebar.appendChild(close_i);

    const content_div = document.createElement("div");
    content_div.classList.add("dialog_content");

    const ok_button = document.createElement("button");

    let content_elements: HTMLElement[] = [];
    if (typeof content === "function") {
        content_elements = content((r) => {
            ok_button.disabled = !r;
        });
    } else {
        content_elements = content;
    }

    for (const c of content_elements) {
        content_div.appendChild(c);
    }

    const button_div = document.createElement("div");
    button_div.classList.add("button_row");

    ok_button.innerText = trigger_text;
    ok_button.onclick = () => {
        trigger_action();
        dialog.close();
    };

    button_div.appendChild(ok_button);

    dialog.appendChild(titlebar);
    dialog.appendChild(content_div);
    dialog.appendChild(button_div);

    document.body.appendChild(dialog);

    dialog.onclose = close_action;
    dialog.showModal();
}

export function report_export_error_dialog(error: string) {
    alert(error);
}

export function report_export_url_dialog(...urls: string[]) {
    const p_message = document.createElement("p");
    p_message.innerText = "Share this URL:";

    const elements: HTMLElement[] = [p_message];

    for (const url of urls) {
        const url_line_div = document.createElement("div");
        url_line_div.classList.add("url");

        const p_url = document.createElement("p");
        p_url.className = "url_text";
        p_url.innerHTML =
            '<a href="' + url + '" target="_blank">' + url + "</a>";

        const copy_button = document.createElement("button");
        copy_button.classList.add("button");
        copy_button.classList.add("copy_url");
        copy_button.onclick = () => navigator.clipboard.writeText(url);

        const copy_i = document.createElement("i");
        copy_i.classList.add("fa");
        copy_i.classList.add("fa-copy");

        copy_button.appendChild(copy_i);

        url_line_div.appendChild(p_url);
        url_line_div.appendChild(copy_button);

        elements.push(url_line_div);
    }

    modal_dialog("report_export_url", elements);
}

export async function export_gist_dialog(
    exporter: (_description: string, _is_public: boolean) => void,
) {
    const description = document.createElement("textarea");
    description.cols = 80;
    description.rows = 5;
    description.autofocus = true;
    description.placeholder = "Description";

    const is_public_div = document.createElement("div");

    const is_public = document.createElement("input");
    is_public.type = "checkbox";
    is_public.id = "is_public";
    is_public.checked = true;

    const is_public_label = document.createElement("label");
    is_public_label.innerText = "Create public Gist";
    is_public_label.htmlFor = "is_public";

    is_public_div.appendChild(is_public);
    is_public_div.appendChild(is_public_label);

    modal_dialog(
        "gist_export_dialog",
        [description, is_public_div],
        "Export",
        () => exporter(description.value, is_public.checked),
    );
}
