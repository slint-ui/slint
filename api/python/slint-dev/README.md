# slint-dev

Development binary for [`slint`](https://pypi.org/project/slint/).

Install this package alongside and at the same version as `slint` to enable the
additional `system-testing` and MCP capabilities. There is nothing to import
from this package: `slint` picks the development binary up automatically, but
only when those capabilities are actually requested via the `SLINT_TEST_SERVER`
(system testing) or `SLINT_MCP_PORT` (MCP server) environment variables. A plain
run keeps the lean release binary. The variable must be set before `slint` is
first imported.

```sh
pip install "slint[dev]"
# or, explicitly, pinned to the matching version:
pip install slint slint-dev
```

See the [`slint` documentation](https://slint.dev/docs) for details.
