# matrix-channel

Plugin canal **Matrix** pour Akasha — fait entrer les messages de vos salons Matrix dans les sessions de chat de l'agent.

## À quoi ça sert ?

Matrix est un protocole de messagerie fédéré (Element, FluffyChat, etc.). Ce plugin permet à Akasha de **recevoir** les messages Matrix comme s'ils arrivaient dans l'interface native :

- Un message posté dans un salon Matrix est transmis au daemon via `POST /api/message`.
- L'agent Akasha peut y répondre dans la session associée (`session_id` = `matrix-<room_id>`).
- Utile pour piloter Akasha depuis Element, relayer des alertes vers un salon d'équipe, ou centraliser plusieurs canaux de communication.

Ce plugin ne remplace pas un client Matrix complet : il assure un **pont entrant** (homeserver → Akasha). L'envoi de réponses vers Matrix depuis Akasha dépend des évolutions futures du bridge.

## Architecture

| Composant | Rôle |
|-----------|------|
| `wasm/` | Plugin WASM catalogue — enregistre les hooks `on_channel_connect`, `on_channel_message`, `on_channel_disconnect` |
| `sidecar/` | Binaire Rust natif — long-poll `/_matrix/client/v3/sync` et forward vers le daemon |

```
Homeserver Matrix  ←→  akasha-matrix-sidecar  →  Daemon Akasha
   (Client-Server API)     (sync + filtre)           POST /api/message
```

Le **sidecar** est le chemin runtime recommandé. Le WASM déclare le contrat canal au catalogue ; le sidecar porte le trafic réseau (homeserver + daemon).

## Prérequis

- Daemon Akasha sur `http://127.0.0.1:3876` (ou `AKASHA_DAEMON_URL`)
- Compte Matrix actif avec **access token** (généré via Element → Paramètres → Aide & informations → Access Token, ou via login API)
- Le compte doit avoir rejoint au moins un salon dont vous souhaitez recevoir les messages

## Variables d'environnement

| Variable | Obligatoire | Description |
|----------|-------------|-------------|
| `MATRIX_HOMESERVER_URL` | oui | URL du homeserver (ex. `https://matrix.example.org`) |
| `MATRIX_ACCESS_TOKEN` | oui | Token utilisateur Matrix |
| `AKASHA_DAEMON_URL` | non | Défaut `http://127.0.0.1:3876` |
| `MATRIX_ROOM_ID` | non | Filtrer un seul salon (ex. `!abc:matrix.org`) — sinon tous les salons rejoints |
| `MATRIX_SYNC_TIMEOUT_MS` | non | Timeout long-poll sync (défaut `30000`) |
| `MATRIX_SIDECAR_ENABLED` | non | `1` pour lancer sans vérifier `MATRIX_HOMESERVER_URL` au démarrage |

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
# Optionnel : un seul salon
# $env:MATRIX_ROOM_ID = "!roomid:matrix.example.org"
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

1. Boucle infinie de long-poll sur `GET /_matrix/client/v3/sync?timeout=…&since=…`.
2. Pour chaque événement `m.room.message` dans `rooms.join.*.timeline.events` :
   - Ignore les messages sans corps texte.
   - Construit le message Akasha : `[Matrix <room_id> from <sender>]\n<texte>`.
   - POST vers `{AKASHA_DAEMON_URL}/api/message` avec :

```json
{
  "message": "[Matrix !room:example.org from @alice:example.org]\nBonjour Akasha",
  "session_id": "matrix-!room:example.org",
  "channel": "matrix",
  "matrix_event_id": "$eventId",
  "matrix_room_id": "!room:example.org",
  "matrix_sender": "@alice:example.org"
}
```

3. `session_id` stable par salon → l'agent conserve le contexte de conversation Matrix dans Akasha.

## Installation plugin

```bash
akasha plugin install /chemin/vers/Akasha_plugins/plugins/matrix-channel
akasha plugin reload
```

Installer via le catalogue Akasha (`matrix-channel`), puis lancer le sidecar en parallèle du daemon. **Sans sidecar**, le plugin WASM seul n'ouvre aucune sync Matrix.

## Limitations actuelles (v0.1.0)

- Réception uniquement (pas d'envoi Matrix depuis les réponses Akasha).
- Messages texte (`m.room.message` / `content.body`) — pas de pièces jointes, réactions ou événements chiffrés E2EE sans déchiffrement côté sidecar.
- Pas de filtre par expéditeur ni par type de contenu.
