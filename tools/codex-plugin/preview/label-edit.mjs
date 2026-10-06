function tokens(source) {
  return [...source.matchAll(/"(?:\\.|[^"\\])*"|\/\/[^\n]*|\/\*[\s\S]*?\*\/|[A-Za-z_][A-Za-z_0-9-]*|[0-9]+(?:\.[0-9]+)?(?:px)?|[^\s]/g)]
    .filter(match => !match[0].startsWith("//") && !match[0].startsWith("/*"))
    .map(match => ({ text: match[0], start: match.index, end: match.index + match[0].length }));
}

export function findButtonLabel(source) {
  const list = tokens(source);
  const stack = [];
  const candidates = [];
  let windowCount = 0;
  for (let i = 0; i < list.length; i++) {
    if (list[i].text === "{") {
      const owner = list[i - 1]?.text;
      if (owner === "Window") windowCount++;
      if (owner === "Button" && list[i - 2]?.text !== "inherits") {
        candidates.push({ index: i, direct: stack.length === 1 && stack[0] === "Window" });
      }
      stack.push(owner);
    } else if (list[i].text === "}") stack.pop();
  }
  if (windowCount !== 1 || candidates.length !== 1 || !candidates[0].direct) return null;
  const { index } = candidates[0];
  const properties = new Map();
  let depth = 1;
  for (let i = index + 1; i < list.length && depth > 0; i++) {
    if (list[i].text === "{") depth++;
    else if (list[i].text === "}") depth--;
    else if (depth === 1 && list[i + 1]?.text === ":") {
      let end = i + 2;
      while (end < list.length && ![";", "{" , "}"].includes(list[end].text)) end++;
      if (list[end]?.text === ";") {
        if (properties.has(list[i].text)) return null;
        properties.set(list[i].text, {
          start: list[i + 2].start, end: list[end].start,
          value: source.slice(list[i + 2].start, list[end].start).trim(),
        });
        i = end;
      }
    }
  }
  const label = properties.get("label");
  if (!label || !/^"(?:\\.|[^"\\])*"$/.test(label.value)) return null;
  let text;
  try { text = JSON.parse(label.value); } catch { return null; }
  if (text.includes("\n") || text.includes("\r")) return null;
  const length = name => {
    const value = properties.get(name)?.value;
    return value && /^[0-9]+(?:\.[0-9]+)?px$/.test(value) ? Number(value.slice(0, -2)) : null;
  };
  const width = length("width");
  const height = length("height");
  const x = length("x");
  const y = length("y");
  const centered = (name, dimension) => tokens(properties.get(name)?.value ?? "").map(token => token.text).join("") === `(parent.${dimension}-self.${dimension})/2`;
  if (!width || !height || (properties.has("x") && x === null && !centered("x", "width")) ||
      (properties.has("y") && y === null && !centered("y", "height"))) return null;
  if (["rotation-angle", "scale", "translate-x", "translate-y"].some(name => properties.has(name))) return null;
  return {
    ...label, end: label.start + label.value.length, label: text, width, height, x, y,
    offset: list[index - 1].start,
    background: properties.get("background-color")?.value ?? "#dc2626",
    foreground: properties.get("label-color")?.value ?? "#ffffff",
    properties: Object.fromEntries(properties),
    insertAt: list[index].end,
    indent: (source.slice(0, list[index - 1].start).match(/(?:^|\n)([ \t]*)[^\n]*$/)?.[1] ?? "") + "    ",
  };
}

const namedColors = { black: "#000000", white: "#ffffff", red: "#ff0000", green: "#008000", blue: "#0000ff", yellow: "#ffff00", gray: "#808080", silver: "#c0c0c0", purple: "#800080", orange: "#ffa500", pink: "#ffc0cb", lime: "#00ff00", cyan: "#00ffff", magenta: "#ff00ff", navy: "#000080", teal: "#008080", maroon: "#800000", olive: "#808000" };

export function opaqueColor(value) {
  if (namedColors[value]) return namedColors[value];
  if (/^#[0-9a-f]{3}$/i.test(value)) return "#" + [...value.slice(1)].map(char => char + char).join("").toLowerCase();
  if (/^#[0-9a-f]{6}$/i.test(value)) return value.toLowerCase();
  if (/^#[0-9a-f]{6}ff$/i.test(value)) return value.slice(0, 7).toLowerCase();
  if (/^#[0-9a-f]{3}f$/i.test(value)) return opaqueColor(value.slice(0, 4));
  return null;
}

export function findButtonColor(source, property) {
  if (!["background-color", "label-color"].includes(property)) return null;
  const button = findButtonLabel(source);
  if (!button) return null;
  if (property === "background-color" && button.properties.background) return null;
  const binding = button.properties[property];
  const value = binding?.value ?? (property === "background-color" ? "#dc2626" : "#ffffff");
  const color = opaqueColor(value);
  if (!color) return null;
  return { ...binding, end: binding ? binding.start + binding.value.length : undefined, property, value, color, offset: button.offset, insertAt: button.insertAt, indent: button.indent };
}

export function replaceButtonColor(source, target, color) {
  const current = findButtonColor(source, target.property);
  if (!current || current.offset !== target.offset || current.start !== target.start || current.value !== target.value) {
    throw new Error("The source changed. Start the color edit again.");
  }
  if (!/^#[0-9a-f]{6}$/i.test(color)) throw new Error("Enter a color such as #2563eb.");
  if (target.start === undefined) {
    return source.slice(0, target.insertAt) + `\n${target.indent}${target.property}: ${color.toLowerCase()};` + source.slice(target.insertAt);
  }
  return source.slice(0, target.start) + color.toLowerCase() + source.slice(target.end);
}

export function replaceButtonLabel(source, target, label) {
  const current = findButtonLabel(source);
  if (!current || current.start !== target.start || current.value !== target.value) {
    throw new Error("The source changed. Start the label edit again.");
  }
  return source.slice(0, target.start) + JSON.stringify(label) + source.slice(target.end);
}
