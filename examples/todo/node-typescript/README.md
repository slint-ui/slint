# Todo example (TypeScript)

This example demonstrates using Slint with TypeScript, taking advantage of
generated type definitions for full IDE autocomplete and type checking.

## Prerequisites

Before running, generate type definitions from the project file:

```sh
npx slint-compiler -f typescript ../slint-project.json -o ../slint-project.d.json.ts
```

Or use the npm script:

```sh
pnpm run generate
```

## Running

```sh
pnpm start
```

`main.ts` imports `../slint-project.json` directly, which `slint-ui/register` makes possible.
Importing `../ui/todo.slint` still works too.
For that import, generate `../ui/todo.slint.d.ts` from `../ui/todo.slint` instead.
The same source runs unchanged on Deno and Bun, which spell the flag differently:

```sh
node --import slint-ui/register main.ts
deno run --allow-all --preload npm:slint-ui/register main.ts
bun --preload slint-ui/register main.ts
```

