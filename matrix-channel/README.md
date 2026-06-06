# matrix-channel (stub)

Plugin canal Matrix pour Akasha — Phase 5 roadmap KinBot.

## Statut

Stub catalogue avec `manifest.toml`, `plugin.json`, `wasm/src/lib.rs` (enregistre les hooks `on_channel_connect` / `on_channel_message` / `on_channel_disconnect`). Build WASM :

```powershell
cd wasm
cargo build --release --target wasm32-unknown-unknown
copy target\wasm32-unknown-unknown\release\akasha_matrix_channel_plugin.wasm ..\plugin.wasm
```

Implémentation complète à venir : homeserver URL, access token (vault), messages → `POST /api/message`.

## Configuration prévue

- `MATRIX_HOMESERVER_URL`
- Secret vault : `matrix_access_token`
