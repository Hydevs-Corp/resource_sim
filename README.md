# Resource Collection Simulation

Simulation en terminal et interface web de robots autonomes collectant des ressources sur une carte générée procéduralement, avec gestion des menaces (ennemis, météorites) et construction de base.

## Build & lancement

**Prérequis** : Rust + Cargo ([rustup.rs](https://rustup.rs))

### Mode Interface Terminal (TUI)

```bash
cargo run --release
```

Vous arriverez sur un menu d'accueil permettant de :
- Créer une nouvelle simulation (avec configuration des paramètres)
- Charger une simulation sauvegardée
- Quitter

### Mode Serveur Web (API & WebSocket)

```bash
cargo run --release -- --server [--port 3000]
```

Ce mode lance la simulation en arrière-plan et héberge :
- Une interface client sur `http://localhost:3000` (dossier `static/`).
- Un endpoint WebSocket sur `ws://localhost:3000/ws` qui diffuse l'état du jeu en JSON en temps réel.

## Options de Ligne de Commande (CLI)

En plus des menus interactifs, vous pouvez configurer le lancement via les arguments :

- `--server` : Lance en mode serveur Web.
- `--port <PORT>` : Définit le port du serveur (défaut: 3000).
- `--new` : Démarre une nouvelle simulation directement, sans passer par le menu.
- `--resume <FICHIER>` : Reprend une sauvegarde à partir d'un fichier JSON.
- Options de jeu : `--base-hp`, `--robot-hp`, `--enemy-hp`, `--enemy-spawn-speed-ms`, `--collector-capacity`, `--army-cost-metal`, `--army-cost-meat`, `--width`, `--height`.

## Contrôles en jeu (TUI)

- **Flèches directionnelles** : Déplacer la caméra.
- **Maj + Flèches** : Déplacer la caméra plus rapidement.
- **h** : Recentrer la caméra sur la base.
- **s** : Sauvegarder l'état actuel de la partie (crée un fichier `savegame_<timestamp>.json`).
- **F1 / F2** : Changer la police (Défaut / Nerd Font).
- **F3** : Exporter l'état de la carte dans un fichier texte Markdown (`.md`).
- **q** : Quitter le jeu.
- **Konami Code** (`Haut Haut Bas Bas Gauche Droite Gauche Droite b a`) : Activer le mode triche !
  - **c** (triche) : Faire apparaître des cristaux.
  - **e** (triche) : Faire apparaître de l'énergie.

## Fonctionnement

### Carte & Environnement

La carte est générée via du bruit de Perlin. 
- Les cellules au-dessus d'un certain seuil deviennent des obstacles.
- Les ressources — Énergie, Cristaux, Métal, et Viande — sont placées et découvertes aléatoirement ou à la bordure des obstacles.
- **Météorites** : Des météorites tombent aléatoirement sur la carte, détruisant des obstacles et créant potentiellement de nouvelles ressources.

### Robots

- **Scouts** (`s`) — explorent la carte aléatoirement en évitant les obstacles. Ils scannent les cases voisines et signalent toute ressource découverte à la base.
- **Collecteurs** (`c`) — naviguent vers les ressources découvertes pour les récolter et les ramener à la base.
- **Armée** (`A`) — défendent la base et attaquent les ennemis.

### Menaces & Défenses

- **Ennemis** (`V`) : Apparaissent sur les bords de la carte et attaquent les robots et la base.
- **Défenses automatiques** : Lorsque le facteur de peur de la colonie augmente trop, et si suffisamment de ressources ont été amassées, la base construit automatiquement une muraille et des portes pour se protéger.

### Légende (Police par défaut)

| Symbole | Couleur       | Signification        |
| ------- | ------------- | -------------------- |
| `O`     | Cyan clair    | Obstacle             |
| `Y`     | Jaune         | Source d'énergie     |
| `v`     | Magenta clair | Gisement de cristaux |
| `♦`     | Bleu clair    | Gisement de métal    |
| `m`     | Marron        | Viande               |
| `H`     | Jaune         | Base centrale        |
| `s`     | Rouge         | Robot scout          |
| `c`     | Magenta       | Robot collecteur     |
| `A`     | Jaune clair   | Robot de l'armée     |
| `V`     | Rouge clair   | Ennemi               |
| `☄`     | Jaune clair   | Météorite            |

## Architecture du code

```
src/
├── main.rs         — Point d'entrée, arguments CLI, menus et boucle UI
├── server.rs       — Serveur Axum / WebSocket
├── simulation/     — Moteur du jeu (carte, logique robots, ennemis, etc.)
│   ├── mod.rs      — Cœur de la simulation et mise à jour
│   ├── config.rs   — Configuration de la partie
│   ├── state.rs    — Structures pour la sauvegarde/chargement (JSON)
│   └── spawns.rs   — Logique d'apparition des entités
├── ui/             — Interface TUI
│   ├── mod.rs
│   ├── draw.rs     — Rendu Ratatui
│   └── visuals.rs  — Animations (météorites, etc.)
├── fonts/          — Configurations JSON des polices d'affichage
static/             — Fichiers du client Web (HTML/CSS/JS)
```
