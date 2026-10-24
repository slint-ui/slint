# slint-compiler

The ahead-of-time compiler of the [Slint framework](https://slint.dev).
It generates TypeScript type declarations for a `.slint` file,
as well as C++, Rust, or Python code.

npm installs a prebuilt `slint-compiler` binary for your platform, so you don't need to compile it from source.

## Usage

Generate the type declarations for `ui/main.slint`, so that `tsc` and editors type-check its components:

```bash
npx slint-compiler -f typescript ui/main.slint -o ui/main.slint.d.ts
```

To keep the declarations in sync, add it to your project and run it from a `package.json` script:

```bash
npm install --save-dev slint-compiler
```

```json
{
  "scripts": {
    "generate": "slint-compiler -f typescript ui/main.slint -o ui/main.slint.d.ts"
  }
}
```

See [TypeScript Support](https://docs.slint.dev/latest/docs/node/#typescript-support) for the matching `tsconfig.json` setup.
Run `npx slint-compiler --help` for all options.
