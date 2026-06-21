# Home Assistant plugin for Akasha

Contrôle domotique via **Home Assistant** (REST API) et événements temps réel (sidecar WebSocket → webhooks Akasha).

## Prérequis Home Assistant

1. **Installer Home Assistant** — [Home Assistant OS](https://www.home-assistant.io/installation/) (RPi / mini-PC) ou Container sur la machine du daemon.
2. **Utilisateur dédié** « akasha » (recommandé).
3. **Long-lived access token** : Profil → Sécurité → Jetons d'accès.
4. **URL** : détectée avec `akasha discover homeassistant` ou saisie manuelle (`http://127.0.0.1:8123`, `http://homeassistant.local:8123`).
5. **Intégrer vos appareils** dans HA (Zigbee, Z-Wave, Matter via HA — pas dans Akasha).
6. **Test API** : `GET {HA_BASE_URL}/api/` → `{"message":"API running."}`

## Configuration Akasha

| Emplacement | Variable / clé |
|-------------|----------------|
| `connectors.env` | `AKASHA_HOMEASSISTANT_ENABLED=1`, `HA_BASE_URL=...` |
| Vault | `ha_access_token` |
| Env (webhooks) | `AKASHA_AUTOMATION_WEBHOOK_SECRET` |

Activez le connecteur dans **Réglages → Connecteurs** (Tauri) ou :

```powershell
akasha vault set ha_access_token VOTRE_TOKEN
# éditer connectors.env : AKASHA_HOMEASSISTANT_ENABLED=1 et HA_BASE_URL
```

Découverte automatique :

```powershell
akasha discover homeassistant
```

## Build

```powershell
./scripts/build_homeassistant_plugin.ps1
cd plugins/homeassistant/sidecar
cargo build --release
```

## Install

```powershell
akasha plugin install C:\path\to\Akasha_plugins\plugins\homeassistant
akasha plugin reload
```

## Outils agent

| Outil | Description |
|-------|-------------|
| `ha_get_state` | État d'une entité |
| `ha_list_entities` | Liste (filtre `domain` optionnel) |
| `ha_call_service` | Appel service HA |
| `ha_run_script` | Lance un script HA |

Exemple :

```
TOOL: ha_get_state light.salon
TOOL: ha_call_service light turn_on light.salon
TOOL: ha_list_entities light
```

Domaines sensibles (`lock`, `alarm_control_panel`, `cover`, `valve`) exigent `confirm:true` dans le payload ou approbation HITL.

## Sidecar événements

```powershell
$env:HA_BASE_URL="http://127.0.0.1:8123"
$env:HA_ACCESS_TOKEN="..."
$env:AKASHA_AUTOMATION_WEBHOOK_SECRET="..."
$env:AKASHA_DAEMON_URL="http://127.0.0.1:3876"
./target/release/akasha-homeassistant-sidecar
```

Créez un **event trigger** webhook dans Akasha (Task Center) filtrant `payload.source=homeassistant`.

### Alternative : automation HA → webhook

Dans Home Assistant (`automations.yaml`) :

```yaml
- alias: Notify Akasha
  trigger:
    - platform: state
      entity_id: binary_sensor.porte_entree
      to: "on"
  action:
    - service: rest_command.akasha_webhook
```

Configurez `rest_command` avec l'URL `http://HOST:3876/api/automation/webhook` et l'en-tête HMAC (voir doc Akasha `automation-webhooks`).
