# Resource Collection Simulation

Simulation en terminal de robots autonomes collectant des ressources sur une carte générée procéduralement.

## Build & lancement

**Prérequis** : Rust + Cargo ([rustup.rs](https://rustup.rs))

```bash
cargo run --release
```

Appuyer sur n'importe quelle touche pour quitter.

## Fonctionnement

### Carte

La carte est générée via du bruit de Perlin. Les cellules au-dessus d'un seuil deviennent des obstacles (`O`). Les ressources — sources d'énergie (`E`) et gisements de cristaux (`C`) — sont placées aléatoirement avec une quantité entre 50 et 200 unités. La base centrale (`#`) est dégagée à la génération.

### Robots

Deux types de robots partent tous de la base au démarrage.

**Scouts (`x`)** — explorent la carte aléatoirement en évitant les obstacles. À chaque pas, ils scannent les 8 cases voisines et signalent toute ressource découverte à la base via un message `ResourceFound`. La base ajoute la position à la liste des ressources connues, partagée avec les collecteurs.

**Collecteurs (`o`)** — lisent la liste des ressources connues et réclament une cible non déjà assignée à un autre collecteur (système de réservation via `HashSet` partagé). Ils naviguent ensuite vers la ressource grâce à un algorithme BFS qui contourne les obstacles. À l'arrivée, ils récupèrent **toutes** les unités de la ressource en un seul voyage, envoient un message `ResourceCollected` pour vider la cellule sur la carte, puis rentrent à la base décharger. Sans cible connue, ils se positionnent à une case de la base et attendent.

### Architecture concurrente

Chaque robot tourne dans un thread indépendant. La communication repose sur deux mécanismes :

- **`mpsc::channel`** : les threads robots envoient des messages au thread principal (`Moved`, `ResourceFound`, `ResourceCollected`, `Unloaded`). Le thread principal traite ces messages dans `update()` et met à jour l'état global.
- **`Arc<RwLock<...>>`** : la carte et la liste des ressources connues sont partagées en lecture par les threads robots, et modifiées en écriture uniquement par le thread principal (via les messages).

### Légende

| Symbole | Couleur       | Signification        |
| ------- | ------------- | -------------------- |
| `O`     | Cyan clair    | Obstacle             |
| `E`     | Vert          | Source d'énergie     |
| `C`     | Magenta clair | Gisement de cristaux |
| `#`     | Vert clair    | Base centrale        |
| `x`     | Rouge         | Robot scout          |
| `o`     | Magenta       | Robot collecteur     |

## Structure du code

```
src/
├── main.rs         — initialisation du terminal, boucle principale
├── simulation.rs   — carte, robots, threads, messages, BFS
└── ui.rs           — rendu Ratatui
```
