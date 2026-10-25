# tsess

Retrouve et reprend tes sessions de harness IA (Claude Code, Codex, Gemini CLI, opencode…) ou d'un simple shell, depuis le terminal ou une icône de barre de menus.

tsess lit directement l'historique de Claude Code (`~/.claude/projects`) et de Codex (`~/.codex/sessions`) : toutes tes sessions récentes apparaissent, même celles lancées sans tsess, et la reprise se fait par identifiant, donc sur la bonne session.

Deux binaires :
- `tsess` : la ligne de commande
- `tsess-bar` : une icône dans la barre de menus (macOS) ou la zone de notification (Linux, Windows)

## Installation

Il faut [Rust](https://rustup.rs).

```bash
cargo install --git https://github.com/seydou91/tsess
```

Sous Linux, installe d'abord : `sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev`

## Utilisation

```bash
tsess new audit -- claude   # lance claude dans le dossier courant et enregistre le terminal
tsess open audit            # retourne dans le dossier et reprend exactement cette session
tsess open 40fb7fba         # reprend une session Claude/Codex récente par son id (≥ 6 caractères)
tsess new                   # assistant : nom, commande, dossier
tsess ls                    # sessions nommées + 20 plus récentes de Claude/Codex
tsess log audit             # relit l'enregistrement
tsess rm audit              # supprime la session
tsess-bar &                 # icône : clic sur une session pour la reprendre
```

Reprise selon le harness :

| Harness | Commande de reprise |
|---|---|
| claude | `claude --resume <id>` (sinon `--continue`) |
| codex | `codex resume <id>` (sinon `--last`) |
| gemini | `gemini --resume latest` |
| opencode | `opencode --continue` |
| autre | relance la commande d'origine |

Les sessions sont dans `~/.tsess/`. Le menu de l'icône propose aussi « Nouvelle session… » et « Lancer au démarrage ».

## Limites

- Découverte automatique : Claude Code et Codex seulement. Gemini et opencode utilisent « dernière session du dossier ».
- Windows : pas d'enregistrement du terminal (`script` n'existe pas), seulement la reprise.
- Linux et Windows sont peu testés : les retours sont bienvenus.

## Licence

MIT
