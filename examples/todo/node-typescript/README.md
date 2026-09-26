# Todo example (TypeScript)

This example demonstrates using Slint with TypeScript, taking advantage of
generated type definitions for full IDE autocomplete and type checking.

## Prerequisites

Before running, generate the type definitions from the `.slint` file:

```sh
slint-compiler -f typescript ../ui/todo.slint -o ../ui/todo.slint.d.ts
```

Or use the npm script:

```sh
pnpm run generate
```

## Running

```sh
pnpm start
```

`main.ts` imports `../ui/todo.slint` directly, which `slint-ui/register` makes possible.
The same source runs unchanged on Deno and Bun, which spell the flag differently:

```sh
node --import slint-ui/register main.ts
deno run --allow-all --preload npm:slint-ui/register main.ts
bun --preload slint-ui/register main.ts
```

