import { BindingTextProvider, DefinitionPosition, PropertiesView } from "../../../tools/online_editor/src/shared/properties";

let node = PropertiesView.createNode();
let view = new PropertiesView(node);
document.body.appendChild(node);

const vscode = acquireVsCodeApi();
view.property_clicked = (uri, p) => {
    vscode.postMessage({ command: 'property_clicked', uri: uri, property: p });
};
view.change_property = (uri, p, new_value, old_value) => {
    vscode.postMessage({ command: 'change_property', uri: uri, property: p, old_value: old_value, new_value: new_value });
};

class TextProvider implements BindingTextProvider {
    #code: string[];
    constructor(code: string) {
        this.#code = code.split('\n');
    }

    binding_text(location: DefinitionPosition): string {
        let l = location.expression_range.start.line;
        const line_utf8 = new TextEncoder().encode(this.#code[l]);
        let l2 = location.expression_range.end.line;
        if (l == l2) {
            return new TextDecoder().decode(
                line_utf8.slice(location.expression_range.start.character, location.expression_range.end.character));
        }
        let result = new TextDecoder().decode(
            line_utf8.slice(location.expression_range.start.character));
        l++;
        while (l < l2) {
            result += "\n" + this.#code[l];
            l++;
        }
        const end_utf8 = new TextEncoder().encode(this.#code[l]);
        return result + new TextDecoder().decode(
            end_utf8.slice(0, location.expression_range.end.character));
    }
}


window.addEventListener('message', async event => {
    if (event.data.command === "set_properties") {
        view.set_properties(new TextProvider(event.data.code), event.data.properties);
    }
});