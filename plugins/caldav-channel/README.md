# caldav-channel

Plugin canal **CalDAV** pour Akasha — synchronise un agenda externe (Nextcloud, Radicale, Google Calendar via bridge, etc.) avec le calendrier intégré du daemon.

## À quoi ça sert ?

Ce plugin connecte Akasha à un serveur CalDAV pour :

1. **Importer** les événements du serveur distant vers le calendrier Akasha (fenêtre −30 j / +90 j).
2. **Exporter** les créations, modifications et suppressions faites dans Akasha vers le serveur CalDAV (via la file d'attente *outbox*).

Cas d'usage typiques :

- Consulter et planifier avec l'agent Akasha en s'appuyant sur votre agenda professionnel ou personnel déjà hébergé ailleurs.
- Garder Akasha et votre CalDAV alignés sans copier-coller manuel des rendez-vous.
- Déclencher des rappels ou automatisations Akasha (`on_schedule_fire`) une fois les événements synchronisés.

Le plugin WASM **enregistre le contrat catalogue** (hooks, permissions). Le **sidecar** exécute la synchronisation réseau réelle.

## Architecture

| Composant | Rôle |
|-----------|------|
| `wasm/` | Plugin WASM catalogue — enregistre le hook `on_schedule_fire` |
| `sidecar/` | Binaire Rust natif — pull CalDAV + push outbox vers l'API calendrier Akasha |

```
Serveur CalDAV  ←→  akasha-caldav-sidecar  ←→  Daemon Akasha
   (PROPFIND,         (pull + outbox)           POST /api/calendar/external/sync
    REPORT, PUT)                              GET  /api/calendar/external/outbox/pending
```

Le sidecar est **obligatoire** pour une sync fonctionnelle. Le WASM seul ne contacte pas le serveur CalDAV.

## Prérequis

- Daemon Akasha sur `http://127.0.0.1:3876` (ou `AKASHA_DAEMON_URL`)
- Compte CalDAV avec URL, identifiant et mot de passe (ou mot de passe d'application)
- Compte calendrier Akasha configuré côté daemon (`CALDAV_ACCOUNT_ID` doit correspondre à un `account_id` existant)

## Variables d'environnement

| Variable | Obligatoire | Description |
|----------|-------------|-------------|
| `CALDAV_URL` | oui | URL de base du serveur (ex. `https://cloud.example.org/remote.php/dav`) |
| `CALDAV_USERNAME` | oui | Identifiant CalDAV |
| `CALDAV_PASSWORD` | oui | Mot de passe ou token d'application |
| `AKASHA_DAEMON_URL` | non | Défaut `http://127.0.0.1:3876` |
| `CALDAV_CALENDAR_PATH` | non | Chemin relatif du calendrier cible (sinon le *calendar-home-set* par défaut) |
| `CALDAV_ACCOUNT_ID` | non | UUID du compte calendrier Akasha — défaut `00000000-0000-4000-8000-000000000001` |
| `AKASHA_CALENDAR_SYNC_TOKEN` | non | Jeton partagé envoyé en en-tête `X-Akasha-Calendar-Token` |
| `CALDAV_SYNC_INTERVAL_SECS` | non | Intervalle entre deux syncs (défaut `300`) |
| `CALDAV_SIDECAR_ENABLED` | non | `1` pour lancer sans vérifier `CALDAV_URL` au démarrage |

Secrets vault côté host : `caldav_username`, `caldav_password` (injectés en `CALDAV_USERNAME` / `CALDAV_PASSWORD` au lancement du sidecar).

## Build WASM (catalogue)

```powershell
cd wasm
cargo build --release --target wasm32-unknown-unknown
copy target\wasm32-unknown-unknown\release\akasha_caldav_channel_plugin.wasm ..\plugin.wasm
```

Linux/macOS :

```bash
cd wasm
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/akasha_caldav_channel_plugin.wasm ../plugin.wasm
```

## Build & run sidecar

```powershell
cd sidecar
cargo build --release
$env:CALDAV_URL = "https://cloud.example.org/remote.php/dav"
$env:CALDAV_USERNAME = "user@example.org"
$env:CALDAV_PASSWORD = "<app-password>"
$env:CALDAV_ACCOUNT_ID = "00000000-0000-4000-8000-000000000001"
.\target\release\akasha-caldav-sidecar.exe
```

Linux/macOS :

```bash
cd sidecar
cargo build --release
export CALDAV_URL=https://cloud.example.org/remote.php/dav
export CALDAV_USERNAME=user@example.org
export CALDAV_PASSWORD=<app-password>
./target/release/akasha-caldav-sidecar
```

## Comportement de synchronisation

### Pull (CalDAV → Akasha)

À chaque cycle, le sidecar :

1. Découvre le *calendar-home-set* via `PROPFIND` (principal utilisateur → dossier calendrier).
2. Interroge les événements dans la fenêtre temporelle via `REPORT` + filtre `time-range`.
3. Parse les blocs `VCALENDAR` / `VEVENT` (UID, SUMMARY, DTSTART, DTEND, RRULE, etc.).
4. Envoie le lot à `POST /api/calendar/external/sync` avec `account_id`, `events`, `sync_token`.

En cas d'erreur réseau ou CalDAV, le sidecar signale l'échec au daemon via le champ `error` du même endpoint.

### Push (Akasha → CalDAV)

Après chaque pull réussi, le sidecar traite la file *outbox* :

1. `GET /api/calendar/external/outbox/pending?account_id=…`
2. Pour chaque entrée : `create` / `update` → `PUT` du fichier `.ics` ; `delete` → `DELETE` sur l'URL distante.
3. Accusé de réception : `POST /api/calendar/external/outbox/ack`

## Installation plugin

```bash
akasha plugin install /chemin/vers/Akasha_plugins/plugins/caldav-channel
akasha plugin reload
```

Puis lancer le sidecar **en parallèle** du daemon. Sans sidecar, le plugin apparaît dans le catalogue mais aucune synchronisation n'a lieu.

## Limitations actuelles (v0.1.0)

- Sync périodique (pas de webhook CalDAV temps réel).
- Fenêtre fixe −30 / +90 jours.
- Authentification Basic uniquement.
- Pas de gestion avancée des récurrences hors champ `RRULE` brut.
