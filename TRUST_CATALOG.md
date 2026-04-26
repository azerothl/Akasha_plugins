# Plugin catalog: trust, WASM, MCP (Hermes alignment)

## Today

- Each plugin ships **`plugin.json`** + **`manifest.toml`** with **`permissions`** and **`entry_tools`** (see root `README.md`).
- Akasha daemon exposes **`GET /api/plugins/metrics`** for load/reload health.

## Roadmap (trust product)

| Item | Intent |
|------|--------|
| **SHA256 / signature** | `scripts/build_plugins.py` now embeds **`wasm_sha256`** in `plugins.json` when `plugins/<id>/plugin.wasm` exists (run after WASM build). Optional minisign/sigstore attestation later. |
| **Trust metadata** | Maintainer, review status, last security review date in `plugins.json`. |
| **MCP bridges** | Document which WASM tools wrap or delegate to MCP servers (`docs/mcp-runtime.md` in core). |
| **Hooks** | Align with daemon `lifecycle_hooks.json` / future gateway hooks (`docs/gateway-shell-hooks.md` in core). |

PRs welcome: extend `scripts/build_plugins.py` to embed `wasm_sha256` from built artifacts.
