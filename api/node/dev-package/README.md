# slint-ui-dev

Development binaries for [`slint-ui`](https://www.npmjs.com/package/slint-ui).

Install this package as a dev dependency, alongside and at the same version as
`slint-ui`, to enable the additional `system-testing` and MCP capabilities.
There is nothing to import from this package: `slint-ui` picks the development
binary up automatically, but only when those capabilities are actually requested
via the `SLINT_MCP_PORT` (MCP server) or `SLINT_TEST_SERVER` (system testing)
environment variables. A plain run keeps the lean release binary. The variable
must be set before `slint-ui` is first imported.

```sh
pnpm add -D slint-ui-dev
```

See the [`slint-ui` README](https://www.npmjs.com/package/slint-ui) for details.
