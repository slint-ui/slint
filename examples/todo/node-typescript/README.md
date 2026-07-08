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
