# matrix-channel

Plugin canal Matrix pour Akasha — bridge sidecar vers `POST /api/message`.

## Architecture

| Composant | Rôle |
|-----------|------|
| `wasm/` | Plugin WASM catalogue — enregistre les hooks `on_channel_connect` / `on_channel_message` / `on_channel_disconnect` |
| `sidecar/` | Binaire Rust natif — sync Matrix Client-Server API et forward vers le daemon Akasha |

Le **sidecar** est le chemin runtime recommandé : le WASM reste un contrat catalogue ; le sidecar fait le travail réseau (homeserver + daemon).

## Prérequis

- Daemon Akasha sur `http://127.0.0.1:3876` (ou `AKASHA_DAEMON_URL`)
- Compte Matrix avec access token
- Variables d'environnement (ou export vault) :

| Variable | Description |
|----------|-------------|
| `MATRIX_HOMESERVER_URL` | URL du homeserver (ex. `https://matrix.example.org`) |
| `MATRIX_ACCESS_TOKEN` | Token utilisateur Matrix |
| `AKASHA_DAEMON_URL` | Optionnel — défaut `http://127.0.0.1:3876` |
| `MATRIX_ROOM_ID` | Optionnel — filtrer un seul salon |
| `MATRIX_SYNC_TIMEOUT_MS` | Optionnel — long-poll sync (défaut 30000) |

Secret vault attendu côté host : `matrix_access_token` (injecté en `MATRIX_ACCESS_TOKEN` au lancement du sidecar).

## Build WASM (catalogue)

```powershell
cd wasm
cargo build --release --target wasm32-unknown-unknown
copy target\wasm32-unknown-unknown\release\akasha_matrix_channel_plugin.wasm ..\plugin.wasm
```

Linux/macOS :

```bash
cd wasm
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/akasha_matrix_channel_plugin.wasm ../plugin.wasm
```

## Build & run sidecar

```powershell
cd sidecar
cargo build --release
$env:MATRIX_HOMESERVER_URL = "https://matrix.example.org"
$env:MATRIX_ACCESS_TOKEN = "<token>"
.\target\release\akasha-matrix-sidecar.exe
```

Linux/macOS :

```bash
cd sidecar
cargo build --release
export MATRIX_HOMESERVER_URL=https://matrix.example.org
export MATRIX_ACCESS_TOKEN=<token>
./target/release/akasha-matrix-sidecar
```

## Comportement

1. Long-poll `/_matrix/client/v3/sync` sur le homeserver.
2. Pour chaque `m.room.message` reçu, POST vers `{AKASHA_DAEMON_URL}/api/message` avec :
   - `message` — texte préfixé `[Matrix <room> from <sender>]`
   - `session_id` — `matrix-<room_id>` (session continue par salon)
   - métadonnées `channel`, `matrix_event_id`, `matrix_room_id`, `matrix_sender`

## Installation plugin

Installer via catalogue Akasha (`matrix-channel`) puis lancer le sidecar en parallèle du daemon. Le plugin WASM seul n'ouvre pas de sync Matrix — le sidecar est requis pour le bridge complet.
