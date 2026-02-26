# ASCIIFORGE — Spécification Technique Définitive

> **Ce document est la source de vérité absolue du projet.**
> Tout agent IA opérant sur ce codebase DOIT lire ce fichier en premier et s'y conformer sans déviation.
> Aucune décision architecturale ne doit contredire ce qui est spécifié ici sans validation humaine explicite.

---

## 0. RÈGLES ABSOLUES — LIRE EN PREMIER

```
R1  ZERO ALLOCATION dans les hot paths (boucle de rendu, callback audio, FFT).
    Pré-allouer tous les buffers au démarrage. Réutiliser via pool ou swap.

R2  ZERO UNSAFE sauf justification documentée in-situ avec commentaire // SAFETY:
    et preuve formelle que l'invariant est maintenu.

R3  ZERO UNWRAP en dehors des tests. Utiliser anyhow::Result en surface,
    thiserror pour les erreurs de domaine. Les chemins d'erreur audio utilisent
    des valeurs par défaut silencieuses (silence), jamais de panic.

R4  ZERO COPIE INUTILE. Les frames vidéo transitent par Arc<FrameBuffer>.
    Les échantillons audio transitent par ring buffer SPSC (rtrb).
    Les configs transitent par arc-swap.

R5  CHAQUE CRATE du workspace compile indépendamment.
    `cargo check -p <crate>` doit passer sans erreur pour chaque crate isolée.

R6  CHAQUE FONCTION PUBLIQUE a un doc-comment /// avec un exemple.
    `cargo doc --no-deps` sans warning.

R7  CI : `cargo clippy -- -D warnings` + `cargo test` + `cargo fmt --check`.
    Aucun merge sans les trois verts.

R8  PERFORMANCE : le budget par frame est 16.6ms (60fps) sur un terminal 200×60.
    Le rendu terminal (I/O) consomme 5-15ms. Il reste ≤10ms pour TOUT le reste.

R9  DÉPENDANCES : chaque crate externe doit avoir >10K downloads OU être
    justifiée par écrit dans ce document. Pas de micro-crate expérimentale
    en dépendance directe sans wrapper d'isolation.

R10 NAMING : snake_case partout. Pas de préfixe redondant (pas de
    AsciiRenderer::ascii_render, juste Renderer::render). Noms explicites,
    jamais d'abréviation ambiguë.
```

---

## 0.5 PROTOCOLE AGENT — INSTRUCTIONS POUR CLAUDE CODE / ANTIGRAVITY

```
CE DOCUMENT EST VOTRE UNIQUE SOURCE DE VÉRITÉ.
Ne cherchez pas d'alternatives aux choix faits ici.
Ne substituez pas de crates. Ne changez pas l'architecture.
Si un choix vous semble sous-optimal, implémentez-le tel quel
puis documentez votre objection dans un commentaire // REVIEW:
pour évaluation humaine ultérieure.
```

### Ordre d'exécution strict

```
PHASE 1 → PHASE 2 → ... → PHASE 9 (section 9)
Ne commencez JAMAIS une phase sans que la précédente compile ET passe les tests.
```

### Checklist par fichier créé

```
Avant de considérer un fichier terminé :
☐ cargo check -p <crate> passe
☐ cargo clippy -p <crate> -- -D warnings passe
☐ cargo fmt -- --check passe
☐ Chaque fn/struct/enum/trait pub a un /// doc-comment
☐ Aucun unwrap() (sauf dans #[cfg(test)])
☐ Aucun todo!() ou unimplemented!() — utiliser un stub fonctionnel
☐ Les types correspondent EXACTEMENT aux définitions de la section 5
☐ Les signatures de trait correspondent EXACTEMENT à la section 4
```

### Résolution d'ambiguïtés

```
SI ce document ne spécifie pas un détail d'implémentation :
  1. Choisir la solution la plus simple qui respecte R1-R10.
  2. Documenter le choix dans un commentaire // DECISION:
  3. Ne pas demander confirmation — avancer.

SI ce document contient une contradiction :
  1. La section avec le numéro le plus bas a priorité.
  2. Les règles R1-R10 ont priorité sur tout le reste.
  3. Documenter la contradiction dans un commentaire // CONFLICT:
```

### Anti-patterns — NE JAMAIS FAIRE

```
❌ NE PAS utiliser tokio, async-std, ou tout runtime async.
❌ NE PAS utiliser Box<dyn Error> — utiliser anyhow::Error ou thiserror.
❌ NE PAS créer de fichiers en dehors de la structure section 3.1.
❌ NE PAS ajouter de dépendance non listée en section 10 sans // DEVIATION: R9.
❌ NE PAS utiliser println! ou eprintln! — utiliser log::info!, log::warn!, etc.
❌ NE PAS utiliser String quand &str suffit.
❌ NE PAS utiliser clone() sur des données volumineuses — utiliser Arc ou références.
❌ NE PAS utiliser Mutex pour la communication audio — ring buffer uniquement.
❌ NE PAS créer de threads au-delà des 3 spécifiés (section 2.2).
❌ NE PAS écrire de wrapper ou abstraction sans utilisation immédiate.
❌ NE PAS générer de README.md, CHANGELOG, ou CI config avant Phase 9.
❌ NE PAS utiliser #[allow(clippy::...)] — corriger le code.
```

### Validation de phase

```
Après chaque phase, exécuter dans cet ordre :
  1. cargo fmt --all
  2. cargo clippy --workspace -- -D warnings
  3. cargo test --workspace
  4. cargo run --release -- [args de la phase]
  5. Vérifier visuellement le résultat dans le terminal

Si l'étape 4 échoue, NE PAS passer à la phase suivante.
```

---

## 1. VISION PRODUIT

### 1.1 Quoi

AsciiForge est un moteur de rendu ASCII temps réel, audio-réactif, paramétrable, fonctionnant exclusivement en TUI (terminal). Il transforme n'importe quelle source visuelle (image, vidéo, webcam, génération procédurale) en art ASCII/Unicode animé, modulé par l'analyse spectrale d'une source audio (fichier, microphone, loopback système).

### 1.2 Pourquoi en Rust

Latence déterministe (pas de GC), zero-cost abstractions pour le pipeline de traitement, ownership model pour la gestion des buffers partagés entre threads audio/vidéo/rendu sans data race, écosystème audio/TUI mature.

### 1.3 Non-objectifs explicites

- Pas de GUI fenêtrée (ni winit, ni egui, ni iced). Terminal uniquement.
- Pas de streaming réseau (pas de serveur HTTP/WebSocket intégré).
- Pas de machine learning / neural style transfer (hors scope v1).
- Pas d'éditeur de timeline / séquenceur. C'est un instrument temps réel.

---

## 2. ARCHITECTURE GLOBALE

### 2.1 Diagramme de flux

```
┌─────────────────────────────────────────────────────────────────┐
│                        THREAD PRINCIPAL                          │
│                                                                  │
│  ┌──────────┐    ┌──────────────┐    ┌───────────────────────┐  │
│  │ Event    │───▶│ State        │───▶│ Ratatui Renderer      │  │
│  │ Loop     │    │ Machine      │    │ (Buffer direct write) │  │
│  │(crossterm)│    │              │    │                       │  │
│  └──────────┘    └──────┬───────┘    └───────────┬───────────┘  │
│                         │ reads                   │ writes       │
│                    ┌────▼────┐              ┌─────▼─────┐       │
│                    │ Config  │              │ Terminal   │       │
│                    │(arc-swap)│              │ (stdout)  │       │
│                    └────▲────┘              └───────────┘       │
│                         │                                        │
└─────────────────────────┼────────────────────────────────────────┘
                          │ watch
┌─────────────────────────┼────────────────────────────────────────┐
│ THREAD AUDIO             │                                        │
│                          │                                        │
│  ┌───────┐  lock-free  ┌▼──────────┐  triple   ┌────────────┐  │
│  │ cpal  │────────────▶│ DSP       │──buffer──▶│ AudioState │  │
│  │capture│   (rtrb)    │ (FFT +    │           │ (features) │  │
│  └───────┘             │  features)│           └────────────┘  │
│                        └───────────┘                             │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│ THREAD SOURCE (optionnel, selon le type de source)                │
│                                                                   │
│  ┌──────────────┐  bounded    ┌──────────────┐                   │
│  │ VideoDecoder │──channel──▶│ FramePool    │                   │
│  │ / Webcam     │  (flume)   │ (Arc<Frame>) │                   │
│  │ / Procedural │            └──────────────┘                   │
│  └──────────────┘                                                │
└──────────────────────────────────────────────────────────────────┘
```

### 2.2 Principes architecturaux

**Trois threads, pas plus.** Chaque thread a une responsabilité unique :

1. **Main thread** : event loop crossterm + state machine + rendu ratatui. Lit les AudioFeatures via triple buffer (non-bloquant). Lit la frame vidéo courante via `Arc<FrameBuffer>`. Écrit dans le buffer terminal.

2. **Audio thread** : callback cpal (haute priorité OS) → pousse les échantillons dans un ring buffer SPSC → un sous-thread analyse (FFT, features) → écrit les résultats dans un triple buffer.

3. **Source thread** (conditionnel) : décode les frames vidéo, capture webcam, ou génère du procédural. Envoie les frames via un canal borné (capacité 2-3) pour back-pressure naturelle.

**Pas de tokio.** Le travail est CPU-bound. L'async ajoute de la complexité sans bénéfice. Les channels `flume` et `crossbeam-channel` suffisent.

---

## 3. CARGO WORKSPACE

### 3.1 Structure

```
asciiforge/
├── Cargo.toml                    # [workspace]
├── SPEC.md                       # CE DOCUMENT
├── README.md
├── config/
│   └── default.toml              # Configuration par défaut
├── crates/
│   ├── af-core/                  # Types partagés, traits, config
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs         # Structures de config serde
│   │       ├── frame.rs          # FrameBuffer, PixelFormat
│   │       ├── charset.rs        # CharacterSet, ramps, LUT
│   │       ├── color.rs          # Conversion RGB/HSV, quantization
│   │       ├── error.rs          # Types d'erreur (thiserror)
│   │       └── traits.rs         # Source, Processor, Renderer traits
│   │
│   ├── af-audio/                 # Capture + DSP + feature extraction
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── capture.rs        # Wrapper cpal
│   │       ├── decode.rs         # Décodage fichier (symphonia)
│   │       ├── fft.rs            # FFT pipeline (realfft)
│   │       ├── features.rs       # Spectral centroid, flux, RMS, etc.
│   │       ├── beat.rs           # Onset detection, beat tracking
│   │       ├── smoothing.rs      # EMA, peak hold, envelope follower
│   │       └── state.rs          # AudioFeatures struct partagée
│   │
│   ├── af-source/                # Sources d'image/vidéo/procédural
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── image.rs          # Chargement image statique
│   │       ├── video.rs          # Décodage vidéo (ffmpeg-the-third)
│   │       ├── webcam.rs         # Capture webcam (nokhwa)
│   │       ├── procedural.rs     # Noise, particles, raymarching
│   │       └── resize.rs         # Wrapper fast_image_resize
│   │
│   ├── af-ascii/                 # Conversion pixels → caractères
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── luminance.rs      # Mapping luminance → char (LUT)
│   │       ├── edge.rs           # Détection de contours → chars directionnels
│   │       ├── shape_match.rs    # Shape-matching (bitmap correlation)
│   │       ├── braille.rs        # Conversion en patterns braille
│   │       ├── halfblock.rs      # Rendu half-block (▄/▀ + fg/bg)
│   │       ├── color_map.rs      # HSV trick : V=1.0, char encode luminance
│   │       └── compositor.rs     # Combine les layers (edge + fill + color)
│   │
│   ├── af-render/                # Rendu TUI (ratatui)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── canvas.rs         # Écriture directe dans ratatui::Buffer
│   │       ├── ui.rs             # Layout : canvas + sidebar params
│   │       ├── widgets.rs        # Sliders, toggles, param panels
│   │       ├── fps.rs            # FPS counter, frame timing
│   │       └── effects.rs        # Post-processing sur buffer (glow, fade)
│   │
│   └── af-app/                   # Point d'entrée, orchestration
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs            # State machine principale
│           ├── pipeline.rs       # Wiring des modules
│           ├── hotreload.rs      # Watch config + reload
│           └── midi.rs           # Contrôle MIDI optionnel
│
├── tests/                        # Tests d'intégration
│   ├── pipeline_test.rs
│   └── ascii_quality_test.rs
│
└── assets/                       # Fichiers de test
    ├── test_image.png
    ├── test_video.mp4
    └── test_audio.wav
```

### 3.2 Cargo.toml racine

```toml
[workspace]
resolver = "2"
members = [
    "crates/af-core",
    "crates/af-audio",
    "crates/af-source",
    "crates/af-ascii",
    "crates/af-render",
    "crates/af-app",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
authors = ["AsciiForge Contributors"]

[workspace.dependencies]
# === Core ===
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
log = "0.4"
env_logger = "0.11"

# === Concurrence ===
flume = "0.11"
rtrb = "0.3"
triple_buffer = "8"
arc-swap = "1"
rayon = "1.10"
crossbeam-utils = "0.8"

# === TUI ===
ratatui = "0.30"
crossterm = "0.29"

# === Image ===
image = "0.25"
fast_image_resize = "6"

# === Audio ===
cpal = "0.16"
realfft = "3.5"
symphonia = { version = "0.5", features = ["mp3", "flac", "ogg", "wav", "aac"] }

# === Vidéo (feature-gated) ===
# ffmpeg-the-third = "2"    # Activé via feature flag
# nokhwa = "0.10"           # Activé via feature flag

# === Utilitaires ===
notify = "7"
midir = "0.10"
glam = "0.29"
clap = { version = "4", features = ["derive"] }
ctrlc = { version = "3", features = ["termination"] }

[workspace.lints.rust]
unsafe_code = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "warn"
```

### 3.3 Feature flags

```toml
# Dans crates/af-source/Cargo.toml
[features]
default = ["image-source"]
image-source = []
video = ["dep:ffmpeg-the-third"]
webcam = ["dep:nokhwa"]
procedural = ["dep:noise", "dep:glam"]

# Dans crates/af-app/Cargo.toml
[features]
default = ["image-source"]
full = ["video", "webcam", "procedural", "midi"]
video = ["af-source/video"]
webcam = ["af-source/webcam"]
procedural = ["af-source/procedural"]
midi = ["dep:midir"]
```

Cela permet `cargo build` minimal (image seulement) et `cargo build --features full` pour tout.

---

## 4. TRAITS FONDAMENTAUX

Tous définis dans `af-core/src/traits.rs`. Ce sont les contrats que chaque module respecte.

### 4.1 Source

```rust
/// Fournit des frames visuelles au pipeline.
/// Implémenté par : ImageSource, VideoSource, WebcamSource, ProceduralSource.
pub trait Source: Send + 'static {
    /// Retourne la prochaine frame disponible.
    /// Retourne None si la source est épuisée (fin de vidéo).
    /// Ne bloque JAMAIS — retourne la dernière frame connue si pas de nouvelle.
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>>;

    /// Dimensions natives de la source (avant resize).
    fn native_size(&self) -> (u32, u32);

    /// Indique si la source est infinie (webcam, procédural) ou finie (fichier).
    fn is_live(&self) -> bool;
}
```

### 4.2 Processor

```rust
/// Transforme une frame pixel en une grille de cellules ASCII.
/// Le pipeline peut chaîner plusieurs Processors.
pub trait Processor: Send + Sync {
    /// Traite une frame et écrit le résultat dans output_grid.
    /// `audio` contient les features audio courantes (peut être None si pas d'audio).
    /// `config` contient les paramètres utilisateur courants.
    ///
    /// CONTRAT : ne doit PAS allouer. `output_grid` est pré-alloué et réutilisé.
    fn process(
        &self,
        input: &FrameBuffer,
        audio: Option<&AudioFeatures>,
        config: &RenderConfig,
        output: &mut AsciiGrid,
    );

    /// Nom lisible pour le debug/UI.
    fn name(&self) -> &'static str;
}
```

### 4.3 AudioAnalyzer

```rust
/// Analyse un buffer d'échantillons audio et produit des features.
pub trait AudioAnalyzer: Send + 'static {
    /// Traite un bloc d'échantillons (mono, f32, normalisé [-1, 1]).
    /// Écrit les résultats dans `features`.
    ///
    /// CONTRAT : ne doit PAS allouer. Tous les buffers internes sont
    /// pré-alloués dans le constructeur.
    fn analyze(&mut self, samples: &[f32], features: &mut AudioFeatures);
}
```

---

## 5. TYPES DE DONNÉES CRITIQUES

Tous dans `af-core/src/`.

### 5.1 FrameBuffer

```rust
/// Buffer de pixels réutilisable. Pré-alloué, jamais redimensionné en hot path.
pub struct FrameBuffer {
    /// Pixels RGBA, row-major, 4 bytes par pixel.
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl FrameBuffer {
    /// Crée un buffer pré-alloué aux dimensions données.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    /// Accès au pixel (x, y) → (r, g, b, a).
    #[inline(always)]
    pub fn pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let idx = ((y * self.width + x) * 4) as usize;
        (self.data[idx], self.data[idx + 1], self.data[idx + 2], self.data[idx + 3])
    }

    /// Luminance perceptuelle BT.709.
    #[inline(always)]
    pub fn luminance(&self, x: u32, y: u32) -> u8 {
        let (r, g, b, _) = self.pixel(x, y);
        ((r as u32 * 2126 + g as u32 * 7152 + b as u32 * 722) / 10000) as u8
    }
}
```

### 5.2 AsciiGrid

```rust
/// Grille de sortie ASCII. Pré-allouée, réutilisée chaque frame.
pub struct AsciiGrid {
    pub cells: Vec<AsciiCell>,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Default)]
pub struct AsciiCell {
    /// Caractère à afficher.
    pub ch: char,
    /// Couleur foreground (RGB).
    pub fg: (u8, u8, u8),
    /// Couleur background (RGB). (0,0,0) = transparent/default.
    pub bg: (u8, u8, u8),
}

impl AsciiGrid {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            cells: vec![AsciiCell::default(); width as usize * height as usize],
            width,
            height,
        }
    }

    #[inline(always)]
    pub fn set(&mut self, x: u16, y: u16, cell: AsciiCell) {
        self.cells[y as usize * self.width as usize + x as usize] = cell;
    }

    #[inline(always)]
    pub fn get(&self, x: u16, y: u16) -> &AsciiCell {
        &self.cells[y as usize * self.width as usize + x as usize]
    }
}
```

### 5.3 AudioFeatures

```rust
/// Résultat de l'analyse audio pour une frame temporelle.
/// Écrit par le thread audio, lu par le thread de rendu.
/// Taille fixe, Copy, jamais alloué dynamiquement.
#[derive(Clone, Copy, Default)]
pub struct AudioFeatures {
    // === Amplitude ===
    /// RMS (Root Mean Square) normalisé [0.0, 1.0].
    pub rms: f32,
    /// Peak amplitude de la fenêtre courante [0.0, 1.0].
    pub peak: f32,

    // === Bandes de fréquence (énergie normalisée [0.0, 1.0]) ===
    /// Sub-bass : 20–60 Hz.
    pub sub_bass: f32,
    /// Bass : 60–250 Hz.
    pub bass: f32,
    /// Low-mid : 250–500 Hz.
    pub low_mid: f32,
    /// Mid : 500–2000 Hz.
    pub mid: f32,
    /// High-mid : 2000–4000 Hz.
    pub high_mid: f32,
    /// Presence : 4000–6000 Hz.
    pub presence: f32,
    /// Brilliance : 6000–20000 Hz.
    pub brilliance: f32,

    // === Features spectrales ===
    /// Centroïde spectral normalisé [0.0, 1.0] (brillance du timbre).
    pub spectral_centroid: f32,
    /// Flux spectral [0.0, 1.0] (changement entre frames).
    pub spectral_flux: f32,
    /// Flatness spectrale [0.0, 1.0] (bruit vs tonal).
    pub spectral_flatness: f32,

    // === Détection d'événements ===
    /// True si un onset (attaque) est détecté dans cette frame.
    pub onset: bool,
    /// BPM estimé (0.0 si inconnu).
    pub bpm: f32,
    /// Phase du beat [0.0, 1.0] (0.0 = sur le beat, 0.5 = entre deux beats).
    pub beat_phase: f32,

    // === Spectre compressé pour visualisation ===
    /// 32 bandes log-fréquence, normalisées [0.0, 1.0].
    /// Suffisant pour toute visualisation. Pas besoin du spectre FFT complet.
    pub spectrum_bands: [f32; 32],
}
```

### 5.4 RenderConfig

```rust
/// Configuration complète du rendu, hot-rechargeable.
/// Sérialisable en TOML. Chaque champ a une valeur par défaut saine.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RenderConfig {
    // === Mode de rendu ===
    /// "ascii" | "braille" | "halfblock" | "quadrant"
    pub render_mode: RenderMode,
    /// Charset pour le mode ASCII (du plus clair au plus dense).
    pub charset: String,
    /// Inverser la luminance (pour fond clair).
    pub invert: bool,
    /// Activer la couleur truecolor.
    pub color_enabled: bool,

    // === Conversion ===
    /// Seuil de détection de contours [0.0, 1.0]. 0 = désactivé.
    pub edge_threshold: f32,
    /// Mix edge/fill [0.0, 1.0]. 0 = fill seulement, 1 = edges seulement.
    pub edge_mix: f32,
    /// Activer le shape-matching (plus lent mais meilleure qualité).
    pub shape_matching: bool,
    /// Correction aspect ratio (typiquement 2.0 pour les polices terminal).
    pub aspect_ratio: f32,

    // === Couleur ===
    /// Méthode de mapping couleur : "direct" | "hsv_bright" | "quantized"
    pub color_mode: ColorMode,
    /// Saturation boost [0.0, 2.0]. 1.0 = neutre.
    pub saturation: f32,
    /// Contraste [0.0, 2.0]. 1.0 = neutre.
    pub contrast: f32,
    /// Brightness offset [-1.0, 1.0]. 0.0 = neutre.
    pub brightness: f32,

    // === Audio-réactivité ===
    /// Mapping des features audio vers les paramètres visuels.
    pub audio_mappings: Vec<AudioMapping>,
    /// Lissage global des features audio [0.0, 1.0]. 0 = brut, 0.9 = très lissé.
    pub audio_smoothing: f32,
    /// Sensibilité globale de la réactivité audio [0.0, 5.0].
    pub audio_sensitivity: f32,

    // === Performance ===
    /// FPS cible. 30 ou 60.
    pub target_fps: u32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AudioMapping {
    /// Feature source : "rms", "bass", "spectral_flux", "onset", etc.
    pub source: String,
    /// Paramètre cible : "edge_threshold", "contrast", "charset_index", etc.
    pub target: String,
    /// Amplitude du mapping [0.0, ∞).
    pub amount: f32,
    /// Offset ajouté après multiplication.
    pub offset: f32,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub enum RenderMode {
    #[default]
    Ascii,
    Braille,
    HalfBlock,
    Quadrant,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub enum ColorMode {
    /// RGB direct du pixel source.
    Direct,
    /// HSV avec V forcé à 1.0 (char encode la luminance).
    #[default]
    HsvBright,
    /// Quantifié sur palette réduite.
    Quantized,
}
```

---

## 6. IMPLÉMENTATIONS DÉTAILLÉES PAR MODULE

### 6.1 af-audio — Pipeline audio

#### 6.1.1 Capture (capture.rs)

```
FLUX : cpal input stream → callback → rtrb::Producer (push échantillons f32 mono)

Détails :
- Ouvrir le device d'entrée par défaut (cpal::default_input_device).
- Préférer f32, 44100 Hz ou 48000 Hz, mono.
- Si stéréo : downmix par moyenne (L+R)/2 dans le callback.
- Le callback cpal est un thread RT haute priorité — NE JAMAIS allouer,
  NE JAMAIS locker un mutex, NE JAMAIS appeler de syscall bloquant.
- Écrire dans rtrb::Producer. Si le consumer est en retard, les échantillons
  les plus anciens sont perdus (overwrite). Pas grave pour du temps réel.
- Taille du ring buffer : 4096 échantillons (≈93ms à 44.1kHz). Suffisant
  pour que le thread d'analyse ne manque jamais de données.
```

#### 6.1.2 Décodage fichier (decode.rs)

```
FLUX : fichier → symphonia decoder → échantillons f32 → cpal output stream
                                                       └→ même rtrb que le capture
                                                          (analyse le playback)

Détails :
- Utiliser symphonia pour décoder MP3/FLAC/WAV/OGG/AAC.
- Convertir en f32 mono normalisé [-1, 1].
- Envoyer à cpal output stream pour le playback.
- SIMULTANÉMENT, copier les échantillons dans le même rtrb::Producer
  utilisé par l'analyse. Ainsi l'analyse reçoit exactement ce qui est joué.
- Rate matching : si le sample rate du fichier ≠ device, resample via
  `rubato` ou simple linear interpolation (qualité suffisante pour analyse).
```

#### 6.1.3 FFT Pipeline (fft.rs)

```
FLUX : rtrb::Consumer → fenêtrage Hann → realfft → magnitudes → bandes

Paramètres fixes :
- FFT size : 2048 (≈46ms à 44.1kHz, résolution fréquentielle ≈21Hz).
- Hop size : 512 (≈11.6ms, ≈86 analyses/sec, largement > 60fps).
- Window : Hann (bon compromis résolution/leakage).

Implémentation :
1. Lire 2048 échantillons du ring buffer.
2. Appliquer la fenêtre de Hann (pré-calculée au démarrage, Vec<f32>).
3. FFT via realfft::RealFftPlanner (plan mis en cache, zéro allocation).
4. Calculer les magnitudes : sqrt(re² + im²) pour chaque bin.
5. Convertir en dB : 20 * log10(magnitude / ref). Clamp à [-80, 0] dB.
6. Projeter sur 32 bandes log-fréquence (mapping pré-calculé).
7. Calculer les 7 bandes de fréquence nommées (sub_bass..brilliance)
   par somme des bins correspondants.

TOUS les buffers (window, fft_input, fft_output, magnitudes, prev_magnitudes)
sont des champs du struct FftPipeline, alloués une seule fois dans new().
```

#### 6.1.4 Features (features.rs)

Chaque feature est une fonction pure `fn(magnitudes, prev_magnitudes, samples) -> f32`.

```
RMS = sqrt(sum(samples²) / N)

Spectral Centroid = sum(f_i * |X_i|) / sum(|X_i|)
  Normalisé par : centroid / (sample_rate / 2)

Spectral Flux = sum((|X_i| - |X_prev_i|)²)  (seulement les positifs : half-wave rectified)
  Normalisé par le nombre de bins.

Spectral Flatness = geometric_mean(|X_i|) / arithmetic_mean(|X_i|)
  Exp(mean(log(|X_i|))) / mean(|X_i|). Clamp les zeros à epsilon.

Onset detection :
  1. Calculer spectral flux.
  2. Maintenir une moyenne mobile (EMA, α=0.1) du flux.
  3. Si flux > moyenne * seuil (typ. 1.5), onset = true.
  4. Cooldown de 50ms entre deux onsets.

BPM estimation :
  1. Maintenir un historique circulaire des 512 dernières valeurs de spectral flux.
  2. Auto-corrélation sur cet historique.
  3. Trouver le pic d'auto-corrélation dans la plage [60, 200] BPM.
  4. Lissage : EMA sur le BPM estimé (α=0.05) pour stabilité.
```

#### 6.1.5 Smoothing (smoothing.rs)

```rust
/// Exponential Moving Average — le lissage fondamental.
/// α proche de 0 = très lissé (lent). α proche de 1 = brut (rapide).
pub struct Ema {
    value: f32,
    alpha: f32,
}

impl Ema {
    pub fn new(alpha: f32) -> Self { Self { value: 0.0, alpha } }

    #[inline(always)]
    pub fn update(&mut self, input: f32) -> f32 {
        self.value = self.alpha * input + (1.0 - self.alpha) * self.value;
        self.value
    }
}

/// Peak hold with decay — pour les onsets et transitoires.
/// Monte instantanément, descend exponentiellement.
pub struct PeakHold {
    value: f32,
    decay: f32, // Par frame. 0.95 = decay lent, 0.8 = rapide.
}
```

Chaque feature dans AudioFeatures passe par son propre Ema dont l'alpha est dérivé de `config.audio_smoothing`.

### 6.2 af-ascii — Moteur de conversion

#### 6.2.1 Luminance mapping (luminance.rs)

```
ALGORITHME :
1. Au démarrage : construire une LUT de 256 entrées.
   Pour chaque valeur de luminance [0..255], pré-calculer le char correspondant.
   Index = luminance * (charset.len() - 1) / 255.
   Stocker dans un array [char; 256].

2. Par pixel :
   luma = frame.luminance(x, y)
   si config.invert : luma = 255 - luma
   char = LUT[luma as usize]

Coût : 1 array lookup par cellule. ~0.5ns.

La LUT est recalculée UNIQUEMENT quand le charset change (événement rare).
```

#### 6.2.2 Edge detection (edge.rs)

```
ALGORITHME (Sobel simplifié, optimisé pour grid ASCII) :

Pour chaque cellule (cx, cy) de la grille de sortie :
  1. Mapper vers le bloc de pixels source correspondant.
  2. Calculer le gradient horizontal Gx et vertical Gy via Sobel 3×3
     sur les luminances du bloc.
  3. Magnitude = sqrt(Gx² + Gy²). Si < edge_threshold × 255 → pas d'edge.
  4. Angle = atan2(Gy, Gx).
  5. Quantifier l'angle en 4 directions :
     - Horizontal (±22.5° de 0°/180°) → '─'
     - Vertical (±22.5° de 90°/270°)  → '│'
     - Diag montante (±22.5° de 45°)  → '/'
     - Diag descendante (±22.5° de 135°) → '\'
  6. Optionnel : détecter les coins et intersections → '+', '┼', '┌', etc.

Mix avec luminance fill :
  Si edge détecté : char = edge_char, fg = couleur du pixel.
  Si pas d'edge : char = luminance_LUT[luma], fg = couleur du pixel.
  Le ratio est contrôlé par config.edge_mix.

Optimisation : atan2 est coûteux. Utiliser une LUT d'angle pré-calculée
sur les 256×256 valeurs possibles de (Gx, Gy) quantifiés en i8.
Ou plus simplement : comparer |Gx| et |Gy| et les signes, sans atan2.
```

#### 6.2.3 Shape-matching (shape_match.rs)

```
ALGORITHME (technique SOTA issue des ASCII shaders) :

Pré-calcul (une fois au démarrage) :
  Pour chaque caractère du charset, rasteriser le glyph en bitmap NxN
  (typiquement 5×5 ou 8×8). Encoder chaque bitmap comme un entier :
  bits[i] = 1 si pixel (i%N, i/N) est "allumé" (luminance > 50%).
  Stocker dans un Vec<(char, u32)>.

  ALTERNATIVE sans rasterisation de font :
  Utiliser des bitmaps hardcodés pour les 95 printable ASCII.
  Source : mesure expérimentale documentée ou bitmap tables existantes.

Par cellule :
  1. Extraire le bloc de pixels source correspondant à la cellule.
  2. Sous-échantillonner le bloc en NxN (même résolution que les bitmaps).
  3. Binariser : chaque sous-pixel est 1 si luminance > médiane du bloc.
  4. Encoder en entier (même format que les bitmaps pré-calculés).
  5. Pour chaque caractère du charset, calculer la distance de Hamming
     (XOR + popcount) entre le pattern du bloc et le bitmap du char.
  6. Le char avec la distance minimale gagne.

Coût : popcount est une instruction CPU unique (POPCNT). Pour un charset
de 70 chars et des bitmaps 25-bit : 70 XOR + 70 POPCNT par cellule.
Sur un terminal 200×60 (12000 cellules) : 840K opérations, ≈0.5ms.

C'est le différenciateur qualité principal du projet.
Aucune crate Rust existante n'implémente cette technique.
```

#### 6.2.4 Color mapping (color_map.rs)

```
ALGORITHME HSV Bright (technique identifiée comme SOTA) :

Pour chaque cellule :
  1. Obtenir la couleur RGB du pixel source.
  2. Convertir RGB → HSV.
  3. Saturation ajustée : s' = min(s * config.saturation, 1.0)
  4. Value forcée : v' = 1.0
  5. Reconvertir HSV(h, s', v') → RGB pour la couleur ANSI foreground.
  6. Le caractère ASCII encode la luminance apparente.

Résultat : couleurs vibrantes même sur fond noir. Le contraste est porté
par la densité du caractère, pas par la couleur.

ALTERNATIVE "direct" : fg = RGB source tel quel. Plus fidèle mais moins punchy.
ALTERNATIVE "quantized" : réduire à 16/32 couleurs via LUT 3D pré-calculée.
```

### 6.3 af-render — Rendu terminal

#### 6.3.1 Canvas (canvas.rs)

```
MÉTHODE : écriture directe dans ratatui::buffer::Buffer.

PAS de widget Canvas ratatui. On écrit directement dans le Buffer exposé
par Frame::buffer_mut() dans le callback draw(). C'est le chemin le plus
rapide — zéro overhead de layout, zéro allocation de widget.

Pour chaque cellule (x, y) de l'AsciiGrid :
  let cell = grid.get(x, y);
  let buf_cell = buf.cell_mut((area.x + x, area.y + y));
  buf_cell.set_char(cell.ch);
  buf_cell.set_fg(Color::Rgb(cell.fg.0, cell.fg.1, cell.fg.2));
  if cell.bg != (0, 0, 0) {
      buf_cell.set_bg(Color::Rgb(cell.bg.0, cell.bg.1, cell.bg.2));
  }

Pour le mode HalfBlock :
  Deux lignes de pixels par ligne de terminal.
  Le bg colore le pixel du haut, le fg colore le pixel du bas via '▄'.
  buf_cell.set_char('▄');
  buf_cell.set_bg(Color::Rgb(top_r, top_g, top_b));
  buf_cell.set_fg(Color::Rgb(bot_r, bot_g, bot_b));

Ratatui gère le diff et la sortie synchronisée via crossterm.
```

#### 6.3.2 Layout (ui.rs)

```
┌─────────────────────────────────────────┬──────────────┐
│                                         │  PARAMETERS  │
│                                         │              │
│              ASCII CANVAS               │  Mode: [▼]   │
│           (zone principale)             │  Charset: _  │
│                                         │  Edges: ═══  │
│                                         │  Color: [✓]  │
│                                         │  Smooth: ═══ │
│                                         │              │
│                                         │  ── Audio ── │
│                                         │  Sens: ═════ │
│                                         │  Bass→Edge   │
│                                         │  Flux→Ctrst  │
│                                         │              │
├─────────────────────────────────────────┤  ── Info ──  │
│  ▁▂▃▅▆█▆▅▃▂▁▂▃▅▆█▆▅▃▂▁▂▃▅▆█▆▅▃▂▁    │  60 FPS      │
│  Spectrum visualization (Sparkline)     │  44.1kHz     │
│                                         │  120 BPM     │
└─────────────────────────────────────────┴──────────────┘

Layout ratatui :
- Layout horizontal : [Constraint::Min(40), Constraint::Length(16)]
- Canvas : zone gauche complète.
- Sidebar : zone droite, 16 colonnes, scrollable si terminal petit.
- Spectrum bar : Layout vertical du canvas, Constraint::Length(3) en bas.
- La sidebar est redessinée SEULEMENT si un param change (dirty flag).
  Le canvas est redessiné à chaque frame.
```

---

## 7. PIPELINE D'EXÉCUTION — FRAME PAR FRAME

```
CHAQUE FRAME (≤16.6ms budget à 60fps) :

T=0ms     │ Main thread poll : crossterm::event::poll(Duration::ZERO)
          │ Traiter les événements clavier/souris s'il y en a.
          │
T≈0.1ms   │ Lire AudioFeatures depuis triple_buffer::Output::read().
          │ (Non-bloquant, toujours la dernière valeur.)
          │
T≈0.2ms   │ Lire la frame source :
          │   - Image statique : même Arc<FrameBuffer> (pas de coût).
          │   - Vidéo/webcam : flume::Receiver::try_recv(). Si Ok, swap.
          │     Si Empty, réutiliser la frame précédente.
          │
T≈0.3ms   │ Resize si nécessaire (fast_image_resize).
          │ Dimensions cible = taille du canvas en cellules
          │ × facteur selon le mode (1× ASCII, 2×4 braille, 1×2 halfblock).
          │
T≈1-3ms   │ Appliquer les AudioMappings : modifier temporairement les
          │ params de RenderConfig selon les features audio.
          │ Ex: config.edge_threshold += bass * mapping.amount
          │
T≈2-5ms   │ Conversion pixels → AsciiGrid via le Processor actif :
          │   - Luminance LUT : ~0.5ms pour 12K cellules.
          │   - Edge detection : ~1-2ms.
          │   - Shape matching : ~2-4ms (si activé).
          │   - Color mapping : ~0.3ms.
          │
T≈5-7ms   │ terminal.draw(|frame| { ... })
          │   Écrire AsciiGrid dans frame.buffer_mut().
          │   Écrire sidebar widgets (si dirty).
          │   Écrire spectrum sparkline.
          │   Ratatui diff + crossterm flush.
          │
T≈7-15ms  │ Terminal I/O (le vrai bottleneck).
          │
T≈15ms    │ sleep_until(next_frame_time) si en avance.
          │ Pas de busy-wait.
```

---

## 8. FICHIER DE CONFIGURATION PAR DÉFAUT

```toml
# config/default.toml — Configuration AsciiForge par défaut.

[render]
render_mode = "Ascii"
charset = " .:-=+*#%@"
invert = false
color_enabled = true
edge_threshold = 0.3
edge_mix = 0.5
shape_matching = false
aspect_ratio = 2.0
color_mode = "HsvBright"
saturation = 1.2
contrast = 1.0
brightness = 0.0
target_fps = 30

[audio]
smoothing = 0.7
sensitivity = 1.0

[[audio.mappings]]
source = "bass"
target = "edge_threshold"
amount = 0.3
offset = 0.0

[[audio.mappings]]
source = "spectral_flux"
target = "contrast"
amount = 0.5
offset = 0.0

[[audio.mappings]]
source = "onset"
target = "invert"
amount = 1.0
offset = 0.0

[[audio.mappings]]
source = "rms"
target = "brightness"
amount = 0.2
offset = 0.0
```

---

## 9. PLAN D'IMPLÉMENTATION PHASÉ

L'ordre est strict. Chaque phase produit un livrable fonctionnel testable.

### Phase 1 — Squelette (1-2 jours)

```
Livrable : cargo build compile, cargo run affiche un écran ratatui vide avec FPS counter.

1. Créer le workspace complet (structure de fichiers section 3.1).
2. af-core : tous les types de données (section 5), traits (section 4), config.
3. af-render : event loop minimal, layout vide, FPS counter.
4. af-app : main.rs qui lance l'event loop.
5. Config : charger default.toml, hot-reload basique via notify.

Tests : cargo check, cargo clippy, cargo run affiche un terminal interactif.
```

### Phase 2 — Image → ASCII statique (1-2 jours)

```
Livrable : cargo run -- --image photo.png affiche l'image en ASCII dans le terminal.

1. af-source/image.rs : charger une image via `image` crate, stocker en FrameBuffer.
2. af-source/resize.rs : wrapper fast_image_resize pour downscale aux dimensions terminal.
3. af-ascii/luminance.rs : LUT mapping avec charset configurable.
4. af-ascii/color_map.rs : modes Direct et HsvBright.
5. af-render/canvas.rs : écriture directe dans le Buffer ratatui.
6. af-render/ui.rs : layout canvas + sidebar basique.

Tests : comparer la sortie avec artem sur la même image (sanity check visuel).
```

### Phase 3 — Modes de rendu avancés (2-3 jours)

```
Livrable : switch entre ASCII, HalfBlock, Braille, Quadrant via touche clavier.

1. af-ascii/halfblock.rs : rendu ▄/▀ avec fg/bg indépendants.
2. af-ascii/braille.rs : conversion en patterns braille Unicode.
3. af-ascii/edge.rs : Sobel + caractères directionnels.
4. af-ascii/shape_match.rs : shape-matching avec bitmaps hardcodés.
5. af-ascii/compositor.rs : combinaison edge + fill + couleur.
6. af-render/widgets.rs : sidebar avec mode selector, sliders basiques.

Tests : benchmark chaque mode sur image 1920×1080, vérifier <10ms total.
```

### Phase 4 — Audio capture + analyse (2-3 jours)

```
Livrable : barre de spectre animée réagissant au microphone en temps réel.

1. af-audio/capture.rs : cpal input → rtrb ring buffer.
2. af-audio/fft.rs : pipeline FFT complet (section 6.1.3).
3. af-audio/features.rs : RMS, bandes, centroid, flux, flatness.
4. af-audio/beat.rs : onset detection + BPM estimation.
5. af-audio/smoothing.rs : EMA + peak hold.
6. af-audio/state.rs : AudioFeatures → triple_buffer.
7. af-render : sparkline spectrum dans le canvas.

Tests : jouer une musique, vérifier que le spectre bouge. Mesurer la latence
audio→visuel (doit être <50ms perçu).
```

### Phase 5 — Audio-réactivité (1-2 jours)

```
Livrable : image ASCII modulée par la musique en temps réel.

1. af-app/pipeline.rs : câblage AudioFeatures → AudioMappings → RenderConfig.
2. Appliquer les mappings dynamiquement avant chaque frame.
3. Tester les presets de mapping (bass→edges, flux→contrast, onset→invert).
4. Ajouter les sliders audio dans la sidebar (sensitivity, smoothing).

Tests : test subjectif — la réactivité doit être perceptuellement synchrone.
```

### Phase 6 — Décodage fichier audio (1 jour)

```
Livrable : cargo run -- --image photo.png --audio song.mp3

1. af-audio/decode.rs : symphonia decoder → cpal output + rtrb pour analyse.
2. Contrôles playback : play/pause/seek (touches clavier).
3. Affichage temps écoulé / durée totale dans la sidebar.
```

### Phase 7 — Vidéo (2-3 jours, feature-gated)

```
Livrable : cargo run --features video -- --video clip.mp4

1. af-source/video.rs : ffmpeg-the-third decoder → frames → flume channel.
2. Synchronisation video/audio par timestamps.
3. Si le fichier vidéo a une piste audio, l'extraire et la router vers af-audio.
4. Contrôles : play/pause/seek.
```

### Phase 8 — Webcam + Procédural (2 jours, feature-gated)

```
Livrable : cargo run --features webcam -- --webcam
           cargo run --features procedural -- --procedural noise

1. af-source/webcam.rs : nokhwa capture → frames.
2. af-source/procedural.rs : générateurs Perlin noise, particules, plasma.
3. Les générateurs procéduraux acceptent AudioFeatures comme input pour modulation.
```

### Phase 9 — MIDI + Polish (1-2 jours)

```
Livrable : contrôle des paramètres via contrôleur MIDI.

1. af-app/midi.rs : midir input → mapping CC → config params.
2. Hot-reload TOML finalisé.
3. Presets de configuration (--preset ambient, --preset aggressive, etc.).
4. Documentation utilisateur (README.md complet).
5. Profiling final, optimisation des hot paths restants.
```

---

## 10. DÉPENDANCES VÉRIFIÉES — REGISTRE COMPLET

Chaque dépendance listée ici a été vérifiée sur crates.io au 26/02/2026.

| Crate | Version | Downloads | Rôle | Justification R9 |
|-------|---------|-----------|------|-------------------|
| ratatui | 0.30 | 18.1M | Framework TUI | Standard de facto |
| crossterm | 0.29 | 28M+ | Backend terminal | Requis par ratatui |
| image | 0.25 | 48M+ | Chargement image | Standard universel |
| fast_image_resize | 6.0 | 1.5M+ | Resize SIMD | 10-50× vs image resize |
| cpal | 0.16 | 2.8M+ | Audio I/O | Standard RustAudio |
| realfft | 3.5 | 1.4M+ | FFT real-valued | Wraps rustfft, SIMD |
| symphonia | 0.5 | 3M+ | Décodage audio | Pure Rust, rapide |
| rayon | 1.10 | 112M+ | Parallélisme data | Standard de facto |
| flume | 0.11 | 19M+ | Channels MPMC | ~17ns send/recv |
| rtrb | 0.3 | 490K+ | Ring buffer SPSC | Conçu pour audio RT |
| triple_buffer | 8.0 | 64K+ | Triple buffering | Lock-free, latest-value |
| arc-swap | 1.7 | 34M+ | Atomic Arc swap | Config hot-swap |
| serde | 1.0 | 288M+ | Serialization | Standard universel |
| toml | 0.8 | 40M+ | Config parser | Standard pour config |
| anyhow | 1.0 | 148M+ | Error handling | Surface errors |
| thiserror | 2.0 | 120M+ | Error derive | Domain errors |
| notify | 7.0 | 14M+ | File watcher | Hot-reload config |
| log | 0.4 | 185M+ | Logging facade | Standard |
| env_logger | 0.11 | 38M+ | Log backend | Simple, configurable |
| glam | 0.29 | 30M+ | Math vecteur | SIMD vec2/vec3/vec4 |
| crossbeam-utils | 0.8 | 193M+ | Concurrency utils | CachePadded, etc. |
| clap | 4.x | 82M+ | CLI parsing | Derive-based args |
| ctrlc | 3.x | 12M+ | Signal handler | Graceful shutdown |

**Feature-gated (non compilées par défaut) :**

| Crate | Version | Rôle | Gate |
|-------|---------|------|------|
| ffmpeg-the-third | 2.x | Décodage vidéo | `video` |
| nokhwa | 0.10 | Webcam | `webcam` |
| noise | 0.9 | Bruit procédural | `procedural` |
| midir | 0.10 | MIDI I/O | `midi` |
| criterion | 0.5 | Benchmarks | dev-dependency |

---

## 11. CHARSETS ET BITMAPS DE RÉFÉRENCE

### 11.1 Charsets ASCII ordonnés par densité

```rust
/// 10 caractères — compact, bon contraste.
pub const CHARSET_COMPACT: &str = " .:-=+*#%@";

/// 16 caractères — bon équilibre.
pub const CHARSET_STANDARD: &str = " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// 70 caractères — Paul Bourke, résolution maximale.
pub const CHARSET_FULL: &str = "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ";

/// Blocs Unicode — pseudo-pixels.
pub const CHARSET_BLOCKS: &str = " ░▒▓█";

/// Minimal — haut contraste.
pub const CHARSET_MINIMAL: &str = " .:░▒▓█";
```

### 11.2 Caractères directionnels (edge detection)

```rust
/// Mapping angle quantifié → caractère.
pub const EDGE_CHARS: [(f32, char); 8] = [
    (0.0,    '─'),   // Horizontal
    (45.0,   '╱'),   // Diag montante
    (90.0,   '│'),   // Vertical
    (135.0,  '╲'),   // Diag descendante
    (180.0,  '─'),   // Horizontal (symétrie)
    (225.0,  '╱'),   // Diag montante (symétrie)
    (270.0,  '│'),   // Vertical (symétrie)
    (315.0,  '╲'),   // Diag descendante (symétrie)
];

/// Fallback ASCII pur si le terminal ne supporte pas Unicode box-drawing.
pub const EDGE_CHARS_ASCII: [(f32, char); 4] = [
    (0.0,   '-'),
    (45.0,  '/'),
    (90.0,  '|'),
    (135.0, '\\'),
];
```

---

## 12. RACCOURCIS CLAVIER

```
q / Esc         Quitter
Space           Play/Pause (audio/vidéo)
Tab             Cycle render mode (ASCII → HalfBlock → Braille → Quadrant)
i               Toggle invert
c               Toggle color
e               Toggle edge detection
s               Toggle shape matching
+/-             Ajuster FPS cible (30/60)
←/→             Seek audio/vidéo ±5s
↑/↓             Ajuster audio sensitivity ±0.1
1-9             Charger preset 1-9
r               Reload config depuis fichier
?               Afficher/masquer aide
```

---

## 13. CRITÈRES DE QUALITÉ ET BENCHMARKS

### 13.1 Performance

```
CIBLE : 60 FPS sur terminal 200×60 (12K cellules) avec :
- Source : image 1920×1080
- Mode : ASCII + edges + couleur
- Audio : micro actif, FFT + toutes features

MESURES REQUISES (via criterion ou manuel) :
- Resize 1920×1080 → 200×60 : < 2ms
- Luminance LUT 12K cells : < 0.5ms
- Edge detection 12K cells : < 2ms
- Shape matching 12K cells : < 4ms
- Color mapping 12K cells : < 0.5ms
- Buffer write ratatui 12K cells : < 1ms
- Total CPU (hors terminal I/O) : < 8ms
- FFT 2048 points : < 0.1ms
- Feature extraction : < 0.05ms

TERMINAL I/O (non contrôlable, dépend de l'émulateur) :
- Alacritty/Kitty/Ghostty : 3-8ms
- WezTerm : 5-12ms
- macOS Terminal : 8-20ms
- Recommandation utilisateur : Kitty ou Ghostty pour 60fps.
```

### 13.2 Mémoire

```
Budget mémoire total : < 50MB en usage normal.

Allocations principales :
- FrameBuffer 1920×1080 RGBA : 8.3 MB × 2 (double buffer) = 16.6 MB
- AsciiGrid 200×60 : 12K × ~12 bytes = 144 KB
- FFT buffers : 2048 × 8 bytes × 3 = 48 KB
- Audio ring buffer : 4096 × 4 bytes = 16 KB
- Spectrum history (beat) : 512 × 4 bytes = 2 KB
- Ratatui buffers : ~200 KB (interne)

Total estimé : ~20 MB. Largement dans le budget.
```

### 13.3 Latence audio→visuel

```
Chaîne de latence :
- Capture audio (cpal buffer) : ~5ms (dépend du backend)
- Ring buffer transit : ~0ms (lock-free)
- FFT (2048 @ 44.1kHz) : 46ms de signal requis
- Hop size 512 : nouvelle analyse chaque ~11ms
- Triple buffer transit : ~0ms
- Rendu frame : ~16ms (60fps) ou ~33ms (30fps)

Latence totale perçue : ~30-60ms à 60fps, ~50-80ms à 30fps.
Seuil perceptuel humain audio→visuel : ~100ms.
On est dans le budget avec marge.
```

---

## 14. CONVENTIONS DE CODE

```rust
// === Imports ===
// Grouper : std, external crates, workspace crates, local modules.
// Séparer chaque groupe par une ligne vide.

use std::sync::Arc;

use anyhow::Result;
use ratatui::buffer::Buffer;

use af_core::{AsciiGrid, FrameBuffer, RenderConfig};

use crate::luminance::LuminanceLut;

// === Nommage ===
// Structs : PascalCase, pas de préfixe de module.
// Fonctions : snake_case, verbe d'action.
// Constants : SCREAMING_SNAKE_CASE.
// Modules privés : pas de pub mod sauf nécessité.

// === Documentation ===
/// Chaque item pub a un doc-comment.
/// Les exemples dans les doc-comments sont exécutés par cargo test.
/// Format : description courte, ligne vide, détails si nécessaire.

/// Convertit une luminance [0, 255] en caractère ASCII.
///
/// Utilise une LUT pré-calculée pour un coût O(1) par pixel.
///
/// # Example
/// ```
/// let lut = LuminanceLut::new(" .:#@");
/// assert_eq!(lut.map(0), ' ');
/// assert_eq!(lut.map(255), '@');
/// ```
pub fn map(&self, luminance: u8) -> char { ... }

// === Error handling ===
// Fonctions faillibles retournent Result<T>.
// Les hot paths (callback audio, inner loops) ne sont JAMAIS faillibles.
// Si une erreur est possible dans un hot path, la gérer silencieusement
// (valeur par défaut, skip frame) et logger à niveau warn.
```

---

## 15. COMMANDES ESSENTIELLES

```bash
# Build minimal (image seulement)
cargo build --release

# Build complet
cargo build --release --features full

# Run avec image
cargo run --release -- --image assets/test_image.png

# Run avec audio micro
cargo run --release -- --image assets/test_image.png --audio mic

# Run avec fichier audio
cargo run --release -- --image assets/test_image.png --audio assets/test_audio.wav

# Run avec vidéo (nécessite feature)
cargo run --release --features video -- --video assets/test_video.mp4

# Tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Benchmark (à créer)
cargo bench --bench ascii_bench

# Documentation
cargo doc --workspace --no-deps --open
```

---

## 16. CLI — ARGUMENTS LIGNE DE COMMANDE

Utiliser `clap` v4 (derive). Défini dans `af-app/src/main.rs`.

```rust
use clap::Parser;
use std::path::PathBuf;

/// AsciiForge — Audio-reactive ASCII art engine.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Source visuelle : chemin vers une image (PNG, JPEG, BMP, GIF).
    #[arg(long)]
    pub image: Option<PathBuf>,

    /// Source visuelle : chemin vers une vidéo. Requiert --features video.
    #[arg(long)]
    pub video: Option<PathBuf>,

    /// Utiliser la webcam comme source. Requiert --features webcam.
    #[arg(long, default_value_t = false)]
    pub webcam: bool,

    /// Générateur procédural : "noise", "plasma", "particles", "starfield".
    #[arg(long)]
    pub procedural: Option<String>,

    /// Source audio : "mic" pour microphone, ou chemin vers fichier audio.
    #[arg(long)]
    pub audio: Option<String>,

    /// Fichier de configuration TOML. Défaut : config/default.toml.
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: PathBuf,

    /// Charger un preset nommé (ignore --config).
    #[arg(long)]
    pub preset: Option<String>,

    /// Mode de rendu initial : ascii, halfblock, braille, quadrant.
    #[arg(long)]
    pub mode: Option<String>,

    /// FPS cible (30 ou 60).
    #[arg(long)]
    pub fps: Option<u32>,

    /// Désactiver la couleur.
    #[arg(long, default_value_t = false)]
    pub no_color: bool,

    /// Niveau de log : error, warn, info, debug, trace.
    #[arg(long, default_value = "warn")]
    pub log_level: String,
}
```

Ajouter `clap` aux workspace.dependencies :
```toml
clap = { version = "4", features = ["derive"] }
```

**Validation au démarrage :** exactement une source visuelle doit être fournie (image XOR video XOR webcam XOR procedural). Si zéro ou plus d'une, afficher l'erreur et quitter avec code 1.

---

## 17. TYPES D'ERREUR

### 17.1 af-core/src/error.rs

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Configuration invalide : {0}")]
    Config(String),

    #[error("Fichier introuvable : {path}")]
    FileNotFound { path: String },

    #[error("Format non supporté : {format}")]
    UnsupportedFormat { format: String },

    #[error("Dimensions invalides : {width}×{height}")]
    InvalidDimensions { width: u32, height: u32 },
}
```

### 17.2 af-audio/src/error.rs (équivalent pour chaque crate)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Aucun périphérique audio d'entrée trouvé")]
    NoInputDevice,

    #[error("Aucun périphérique audio de sortie trouvé")]
    NoOutputDevice,

    #[error("Format audio non supporté : {0}")]
    UnsupportedFormat(String),

    #[error("Erreur de stream audio : {0}")]
    StreamError(String),

    #[error("Erreur de décodage : {0}")]
    DecodeError(String),
}
```

**Règle** : chaque crate a son propre type d'erreur. `af-app` utilise `anyhow::Result` en surface pour agréger. Les hot paths ne retournent JAMAIS de Result — ils utilisent des valeurs par défaut silencieuses.

---

## 18. MACHINE D'ÉTAT — af-app/src/app.rs

```rust
pub enum AppState {
    /// L'application est en cours d'exécution normale.
    Running,
    /// Pause (audio et vidéo gelés, rendu continue sur dernière frame).
    Paused,
    /// Overlay d'aide affiché (touche ?).
    Help,
    /// L'application doit se terminer au prochain tour de boucle.
    Quitting,
}

pub struct App {
    pub state: AppState,
    /// Config courante (lecture via arc-swap depuis tous les threads).
    pub config: arc_swap::ArcSwap<RenderConfig>,
    /// Features audio courantes (lecture via triple buffer).
    pub audio_output: triple_buffer::Output<AudioFeatures>,
    /// Frame source courante.
    pub current_frame: Option<Arc<FrameBuffer>>,
    /// Grille ASCII pré-allouée, réutilisée chaque frame.
    pub grid: AsciiGrid,
    /// Frame resizée, pré-allouée.
    pub resized_frame: FrameBuffer,
    /// Flag dirty pour la sidebar (éviter redessin inutile).
    pub sidebar_dirty: bool,
    /// Compteur FPS.
    pub fps_counter: FpsCounter,
    /// Récepteur de frames depuis le source thread.
    pub frame_rx: Option<flume::Receiver<Arc<FrameBuffer>>>,
    /// Dernier terminal size connu (pour détecter les resize).
    pub terminal_size: (u16, u16),
}
```

**Transitions d'état :**
```
Running  ─── Space ───▶ Paused
Paused   ─── Space ───▶ Running
Running  ─── ?     ───▶ Help
Help     ─── ?/Esc ───▶ Running
*        ─── q/Esc ───▶ Quitting  (Esc depuis Help retourne à Running, pas Quitting)
```

---

## 19. POINT D'ENTRÉE — af-app/src/main.rs

Structure exacte. L'agent doit implémenter ce squelette.

```rust
use anyhow::Result;
use clap::Parser;

mod app;
mod cli;
mod hotreload;
mod pipeline;

fn main() -> Result<()> {
    // 1. Parser CLI
    let cli = cli::Cli::parse();

    // 2. Initialiser le logging
    env_logger::Builder::new()
        .filter_level(cli.log_level.parse().unwrap_or(log::LevelFilter::Warn))
        .init();

    // 3. Charger la config
    let config = af_core::config::load_config(&cli.config)?;
    let config = arc_swap::ArcSwap::from_pointee(config);

    // 4. Lancer le hot-reload config (thread interne notify)
    let _watcher = hotreload::spawn_config_watcher(&cli.config, &config)?;

    // 5. Démarrer le thread audio (si --audio fourni)
    let audio_output = if let Some(ref audio_arg) = cli.audio {
        Some(pipeline::start_audio(audio_arg, &config)?)
    } else {
        None
    };

    // 6. Démarrer le source thread (si vidéo/webcam/procédural)
    let frame_rx = pipeline::start_source(&cli)?;

    // 7. Initialiser le terminal ratatui
    let terminal = ratatui::init();

    // 8. Construire l'App
    let mut app = app::App::new(config, audio_output, frame_rx, &cli)?;

    // 9. Boucle principale
    let result = app.run(terminal);

    // 10. Restaurer le terminal (TOUJOURS, même en cas d'erreur)
    ratatui::restore();

    result
}
```

**Point critique** : `ratatui::restore()` doit être appelé même si `app.run()` retourne une erreur. Le pattern ci-dessus le garantit car `restore()` est après le `let result =`.

---

## 20. HOT-RELOAD — af-app/src/hotreload.rs

```rust
use std::path::Path;
use std::sync::Arc;
use arc_swap::ArcSwap;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use af_core::config::RenderConfig;
use anyhow::Result;

/// Lance un thread qui surveille le fichier config et met à jour l'ArcSwap.
/// Retourne le Watcher (doit rester vivant tant que l'app tourne).
pub fn spawn_config_watcher(
    config_path: &Path,
    config: &Arc<ArcSwap<RenderConfig>>,
) -> Result<impl Watcher> {
    let config = Arc::clone(config);
    let path = config_path.to_path_buf();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_)) {
                match af_core::config::load_config(&path) {
                    Ok(new_config) => {
                        config.store(Arc::new(new_config));
                        log::info!("Config rechargée depuis {}", path.display());
                    }
                    Err(e) => {
                        log::warn!("Erreur de rechargement config : {e}");
                        // On garde l'ancienne config. Pas de panic.
                    }
                }
            }
        }
    })?;

    watcher.watch(config_path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
```

**Mécanisme** : `notify` v7 utilise inotify (Linux) / FSEvents (macOS) / ReadDirectoryChanges (Windows). Le callback s'exécute dans un thread interne de notify. `arc_swap::ArcSwap::store` est atomique — les lecteurs (main thread) voient la nouvelle config au prochain `load()`.

---

## 21. GRACEFUL SHUTDOWN

Séquence exacte quand `AppState::Quitting` est atteint :

```
1. Signaler l'arrêt aux threads workers :
   - Dropper le flume::Sender (le source thread détectera la déconnexion).
   - Dropper le rtrb::Producer (le thread audio détectera la déconnexion).
   - Les threads workers terminent naturellement quand leur channel se ferme.

2. Attendre la fin des threads (JoinHandle::join avec timeout 1s).
   Si timeout : log::warn et continuer (ne pas bloquer l'utilisateur).

3. Arrêter le stream cpal (drop le stream).

4. Restaurer le terminal :
   - crossterm::terminal::disable_raw_mode()
   - crossterm::execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)
   - ratatui::restore() encapsule ces appels.

5. Flusher les logs.

6. Quitter avec code 0.
```

**Gestion de SIGINT/SIGTERM** : installer un handler via `ctrlc` crate ou manuellement qui set un `AtomicBool`. La boucle principale le vérifie à chaque itération.

Ajouter aux workspace.dependencies :
```toml
ctrlc = { version = "3", features = ["termination"] }
```

---

## 22. GESTION DU REDIMENSIONNEMENT TERMINAL

```rust
// Dans la boucle principale (app.rs), à chaque frame :

let new_size = crossterm::terminal::size()?; // (cols, rows)
if new_size != self.terminal_size {
    self.terminal_size = new_size;

    // Recalculer les dimensions du canvas
    let sidebar_width = 16u16;
    let spectrum_height = 3u16;
    let canvas_width = new_size.0.saturating_sub(sidebar_width);
    let canvas_height = new_size.1.saturating_sub(spectrum_height);

    // Réallouer la grille ASCII (rare, OK d'allouer ici)
    self.grid = AsciiGrid::new(canvas_width, canvas_height);

    // Recalculer les dimensions cibles du resize image
    let (pixel_w, pixel_h) = match config.render_mode {
        RenderMode::Ascii    => (canvas_width as u32, canvas_height as u32),
        RenderMode::HalfBlock => (canvas_width as u32, canvas_height as u32 * 2),
        RenderMode::Braille  => (canvas_width as u32 * 2, canvas_height as u32 * 4),
        RenderMode::Quadrant => (canvas_width as u32 * 2, canvas_height as u32 * 2),
    };

    // Appliquer la correction aspect ratio
    let pixel_h_corrected = (pixel_h as f32 / config.aspect_ratio) as u32;

    // Réallouer le buffer de frame resizée
    self.resized_frame = FrameBuffer::new(pixel_w, pixel_h_corrected);

    // Forcer un redessin complet de la sidebar
    self.sidebar_dirty = true;

    log::debug!("Terminal resized to {canvas_width}×{canvas_height}");
}
```

**Note** : crossterm émet aussi un `Event::Resize(cols, rows)` dans le stream d'événements. Les deux méthodes fonctionnent. Le polling explicite est plus fiable car il n'attend pas le prochain event poll.

---

## 23. PROTOCOLE DE SORTIE SYNCHRONISÉE

Ratatui + crossterm gèrent cela automatiquement depuis ratatui 0.28+ via `crossterm::queue!` et le mode synchronized output. Mais si un agent implémente du rendu bas niveau :

```rust
use std::io::Write;

/// Encadrer un bloc de rendu pour éviter le tearing.
fn synchronized_write(stdout: &mut impl Write, render_fn: impl FnOnce(&mut impl Write)) {
    // BSU — Begin Synchronized Update
    stdout.write_all(b"\x1B[?2026h").ok();

    render_fn(stdout);

    // ESU — End Synchronized Update
    stdout.write_all(b"\x1B[?2026l").ok();
    stdout.flush().ok();
}
```

**Terminaux supportés** (2026) : Kitty, Ghostty, WezTerm, Alacritty 0.14+, foot, tmux 3.4+, Windows Terminal 1.22+.
**Non supportés** : GNOME Terminal (VTE < 0.78), xterm, macOS Terminal.app.

Ratatui utilise ce protocole automatiquement quand le backend crossterm est configuré. **Ne pas implémenter manuellement sauf si on bypasse ratatui.**

---

## 24. ENCODAGE BRAILLE — af-ascii/src/braille.rs

Chaque caractère braille Unicode (U+2800–U+28FF) encode une matrice 2×4 de points :

```
Bit layout dans un char braille :
  Col 0  Col 1
  bit 0  bit 3   ← Row 0
  bit 1  bit 4   ← Row 1
  bit 2  bit 5   ← Row 2
  bit 6  bit 7   ← Row 3

Caractère = '\u{2800}' + (bits as u32)

Exemple : tous les points allumés = '\u{2800}' + 0xFF = '⣿'
```

```rust
/// Convertit un bloc 2×4 de valeurs booléennes en caractère braille.
#[inline(always)]
pub fn encode_braille(dots: [[bool; 2]; 4]) -> char {
    let mut byte: u8 = 0;
    if dots[0][0] { byte |= 0x01; }
    if dots[1][0] { byte |= 0x02; }
    if dots[2][0] { byte |= 0x04; }
    if dots[0][1] { byte |= 0x08; }
    if dots[1][1] { byte |= 0x10; }
    if dots[2][1] { byte |= 0x20; }
    if dots[3][0] { byte |= 0x40; }
    if dots[3][1] { byte |= 0x80; }
    // SAFETY: 0x2800 + [0, 255] est toujours un codepoint Unicode valide.
    char::from_u32(0x2800 + byte as u32).unwrap_or('⠀')
}

/// Processeur braille complet.
/// Pour chaque cellule terminal (cx, cy) :
///   - Extraire le bloc 2×4 de pixels de la frame source.
///   - Seuiller chaque pixel (luminance > threshold → allumé).
///   - Encoder en braille.
///   - Couleur : moyenne des pixels allumés (une seule couleur par cellule braille).
pub fn process_braille(
    frame: &FrameBuffer,
    config: &RenderConfig,
    grid: &mut AsciiGrid,
) {
    let threshold = if config.invert { 128u8 } else { 128u8 };

    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let px = cx as u32 * 2;
            let py = cy as u32 * 4;
            let mut dots = [[false; 2]; 4];
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut count = 0u32;

            for dy in 0..4 {
                for dx in 0..2 {
                    let x = px + dx;
                    let y = py + dy;
                    if x < frame.width && y < frame.height {
                        let luma = frame.luminance(x, y);
                        let lit = if config.invert { luma < threshold } else { luma >= threshold };
                        dots[dy as usize][dx as usize] = lit;
                        if lit {
                            let (r, g, b, _) = frame.pixel(x, y);
                            r_sum += r as u32;
                            g_sum += g as u32;
                            b_sum += b as u32;
                            count += 1;
                        }
                    }
                }
            }

            let ch = encode_braille(dots);
            let fg = if count > 0 {
                ((r_sum / count) as u8, (g_sum / count) as u8, (b_sum / count) as u8)
            } else {
                (255, 255, 255)
            };

            grid.set(cx, cy, AsciiCell { ch, fg, bg: (0, 0, 0) });
        }
    }
}
```

**Résolution effective** : terminal 200×60 → 400×240 points braille = 96000 sous-pixels.

---

## 25. BLOCS QUADRANT — af-ascii/src/quadrant.rs (optionnel après braille)

Caractères quadrant Unicode (U+2596–U+259F) + espace + full block :

```
Chaque cellule est divisée en 2×2 quadrants :
  ┌───┬───┐
  │ TL│ TR│
  │───┼───│
  │ BL│ BR│
  └───┴───┘

Mapping bits → caractère :
  0000 = ' '     (rien)
  0001 = '▗'     (BR)
  0010 = '▖'     (BL)
  0011 = '▄'     (bas)
  0100 = '▝'     (TR)
  0101 = '▐'     (droite)
  0110 = '▞'     (diag)
  0111 = '▟'     (pas TL)
  1000 = '▘'     (TL)
  1001 = '▚'     (diag inv)
  1010 = '▌'     (gauche)
  1011 = '▙'     (pas TR)
  1100 = '▀'     (haut)
  1101 = '▜'     (pas BL)
  1110 = '▛'     (pas BR)
  1111 = '█'     (plein)
```

```rust
pub const QUADRANT_CHARS: [char; 16] = [
    ' ', '▗', '▖', '▄', '▝', '▐', '▞', '▟',
    '▘', '▚', '▌', '▙', '▀', '▜', '▛', '█',
];
```

**Avantage** : 2 couleurs indépendantes par cellule (fg + bg), résolution 2× horizontale et 2× verticale. Compromis entre half-block (2 couleurs, 1×2) et braille (1 couleur, 2×4).

---

## 26. GÉNÉRATEURS PROCÉDURAUX — af-source/src/procedural.rs

### 26.1 Perlin Noise animé

```rust
use noise::{NoiseFn, Perlin, Seedable};

pub struct NoiseGenerator {
    perlin: Perlin,
    time: f64,
    scale: f64,       // 0.01–0.1 typique
    speed: f64,       // Vitesse d'animation
    octaves: u32,     // 1–8, complexité du bruit
}

impl NoiseGenerator {
    /// Génère une frame de bruit Perlin 3D (x, y, time).
    pub fn generate(&mut self, frame: &mut FrameBuffer, audio: Option<&AudioFeatures>) {
        // Moduler scale par bass, speed par spectral flux
        let scale = self.scale * (1.0 + audio.map_or(0.0, |a| a.bass as f64 * 2.0));
        let speed = self.speed * (1.0 + audio.map_or(0.0, |a| a.spectral_flux as f64));

        for y in 0..frame.height {
            for x in 0..frame.width {
                let nx = x as f64 * scale;
                let ny = y as f64 * scale;
                // Perlin retourne [-1, 1], convertir en [0, 255]
                let val = (self.perlin.get([nx, ny, self.time * speed]) + 1.0) * 0.5;
                let byte = (val.clamp(0.0, 1.0) * 255.0) as u8;

                let idx = ((y * frame.width + x) * 4) as usize;
                // Coloriser via HSV : hue = f(position + time), sat = 0.8, val = bruit
                let hue = ((x as f64 / frame.width as f64) + self.time * 0.1) % 1.0;
                let (r, g, b) = hsv_to_rgb(hue as f32, 0.8, val as f32);
                frame.data[idx]     = r;
                frame.data[idx + 1] = g;
                frame.data[idx + 2] = b;
                frame.data[idx + 3] = 255;
            }
        }
        self.time += 1.0 / 60.0; // Incrément fixe par frame
    }
}
```

### 26.2 Plasma

```
Algorithme classique — superposition de fonctions sinusoïdales :

Pour chaque pixel (x, y) :
  v1 = sin(x * freq1 + time)
  v2 = sin(y * freq2 + time * 0.7)
  v3 = sin((x + y) * freq3 + time * 1.3)
  v4 = sin(sqrt(x² + y²) * freq4 + time * 0.5)
  value = (v1 + v2 + v3 + v4) / 4.0  → [-1, 1]

Coloriser via palette cyclique (LUT de 256 couleurs, indexée par value).

Audio-réactivité :
  - freq1..freq4 modulés par bandes de fréquence (bass, mid, high, brilliance).
  - Vitesse de time modulée par RMS.
  - Palette shift par spectral centroid.
```

### 26.3 Système de particules

```
struct Particle {
    x: f32, y: f32,       // Position
    vx: f32, vy: f32,     // Vélocité
    life: f32,             // Durée de vie restante [0, 1]
    color: (u8, u8, u8),  // Couleur
}

Pool fixe de N particules (typ. 500-2000). Pré-alloué.
Les particules mortes (life ≤ 0) sont réutilisées.

Chaque frame :
  1. Spawn : si onset détecté, activer K particules au centre avec vélocité aléatoire.
     K proportionnel à l'énergie du onset.
  2. Update : pour chaque particule vivante :
     - x += vx * dt
     - y += vy * dt
     - vy += gravity * dt (optionnel)
     - life -= decay * dt
     - Appliquer drag : vx *= 0.98, vy *= 0.98
  3. Render : écrire chaque particule dans le FrameBuffer.
     Luminosité = life (fade out naturel).
     
Pas d'allocation dynamique — le Vec<Particle> est fixe.
```

---

## 27. STRATÉGIE DE TESTS

### 27.1 Tests unitaires (dans chaque crate)

```rust
// af-core/src/charset.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_lut_maps_extremes() {
        let lut = LuminanceLut::new(" .:#@");
        assert_eq!(lut.map(0), ' ');
        assert_eq!(lut.map(255), '@');
    }

    #[test]
    fn luminance_lut_monotonic() {
        let lut = LuminanceLut::new(" .:#@");
        let mut prev = 0u32;
        for i in 0..=255u8 {
            let ch = lut.map(i) as u32;
            assert!(ch >= prev, "LUT non monotone à luminance {i}");
            prev = ch;
        }
    }
}

// af-core/src/color.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsv_roundtrip() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let (h, s, v) = rgb_to_hsv(r, g, b);
                    let (r2, g2, b2) = hsv_to_rgb(h, s, v);
                    assert!((r as i16 - r2 as i16).abs() <= 1);
                    assert!((g as i16 - g2 as i16).abs() <= 1);
                    assert!((b as i16 - b2 as i16).abs() <= 1);
                }
            }
        }
    }

    #[test]
    fn hsv_bright_keeps_hue() {
        let (h, s, _v) = rgb_to_hsv(200, 50, 50);
        let (h2, _s2, v2) = apply_hsv_bright(200, 50, 50);
        assert!((h - h2).abs() < 0.01);
        assert!((v2 - 1.0).abs() < 0.001);
    }
}

// af-audio/src/fft.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_detects_pure_sine() {
        let mut pipeline = FftPipeline::new(44100);
        let freq = 440.0; // La 440 Hz
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / 44100.0).sin())
            .collect();

        let mut features = AudioFeatures::default();
        pipeline.analyze(&samples, &mut features);

        // Le bin dominant doit être proche de 440 Hz
        // Bin index = freq * fft_size / sample_rate = 440 * 2048 / 44100 ≈ 20
        // Le spectral centroid doit être autour de 440/22050 ≈ 0.02
        assert!(features.spectral_centroid > 0.01 && features.spectral_centroid < 0.05);
    }

    #[test]
    fn fft_silence_produces_zero_features() {
        let mut pipeline = FftPipeline::new(44100);
        let samples = vec![0.0f32; 2048];
        let mut features = AudioFeatures::default();
        pipeline.analyze(&samples, &mut features);

        assert!(features.rms < 0.001);
        assert!(features.peak < 0.001);
    }
}

// af-ascii/src/braille.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_empty_is_blank() {
        let dots = [[false; 2]; 4];
        assert_eq!(encode_braille(dots), '⠀'); // U+2800
    }

    #[test]
    fn braille_full_is_solid() {
        let dots = [[true; 2]; 4];
        assert_eq!(encode_braille(dots), '⣿'); // U+28FF
    }
}
```

### 27.2 Tests d'intégration (tests/)

```rust
// tests/pipeline_test.rs
//! Vérifie que le pipeline complet image → ASCII produit une sortie non-vide.

#[test]
fn image_to_ascii_produces_output() {
    let frame = af_source::image::load_image("assets/test_image.png").unwrap();
    let resized = af_source::resize::resize_frame(&frame, 80, 40);
    let config = af_core::config::RenderConfig::default();
    let mut grid = af_core::AsciiGrid::new(80, 40);

    af_ascii::luminance::process_luminance(&resized, &config, &mut grid);

    // Vérifier qu'au moins certaines cellules ne sont pas vides
    let non_empty = grid.cells.iter().filter(|c| c.ch != ' ').count();
    assert!(non_empty > 0, "La grille ne devrait pas être entièrement vide");
}
```

### 27.3 Benchmarks (benches/)

```rust
// benches/ascii_bench.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_luminance_lut(c: &mut Criterion) {
    let frame = FrameBuffer::new(200, 60);
    // Remplir avec des données aléatoires...
    let config = RenderConfig::default();
    let mut grid = AsciiGrid::new(200, 60);

    c.bench_function("luminance_12k_cells", |b| {
        b.iter(|| af_ascii::luminance::process_luminance(&frame, &config, &mut grid))
    });
}

fn bench_shape_matching(c: &mut Criterion) { /* similaire */ }
fn bench_fft_2048(c: &mut Criterion) { /* similaire */ }
fn bench_braille(c: &mut Criterion) { /* similaire */ }

criterion_group!(benches, bench_luminance_lut, bench_shape_matching, bench_fft_2048, bench_braille);
criterion_main!(benches);
```

Ajouter aux workspace.dependencies :
```toml
criterion = { version = "0.5", features = ["html_reports"] }
```

---

## 28. GPU PIPELINE OPTIONNEL (Phase future, non bloquant)

Cette section est informative pour une v2. **Ne pas implémenter avant que les phases 1–9 soient terminées.**

```
Architecture conceptuelle :
  1. wgpu::Device + Queue initialisés au démarrage.
  2. Fragment shader WGSL reçoit les AudioFeatures comme uniform buffer.
  3. Le shader génère une texture (N×M pixels).
  4. Readback GPU → CPU via staging buffer (coût ~1-3ms).
  5. La texture CPU est convertie en FrameBuffer → pipeline ASCII normal.

Uniforms injectés :
  @group(0) @binding(0) var<uniform> time: vec4<f32>;
    // x = time_seconds, y = time*10, z = sin(time), w = cos(time)
  @group(0) @binding(1) var<uniform> audio: array<f32, 48>;
    // [0-31] = spectrum_bands, [32] = rms, [33] = bass, ...
    // [47] = beat_phase
  @group(0) @binding(2) var<uniform> resolution: vec2<u32>;

Crate de référence : tui-shader v0.0.9 (architecture uniquement, ne pas dépendre).
Crate solide : wgpu v26+ (standard, > 4M downloads).

L'intérêt : générer des effets visuels complexes (ray-marching, reaction-diffusion,
fluid simulation) à un coût CPU quasi nul, modulés par l'audio.
Le terminal affiche le résultat ASCII de ces effets GPU.
```

---

## 29. BOUCLE PRINCIPALE — CODE COMPLET

Le cœur de `af-app/src/app.rs`. L'agent doit implémenter exactement cette structure.

```rust
use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use anyhow::Result;

impl App {
    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut last_frame = Instant::now();

        loop {
            // === Sortie si quitting ===
            if matches!(self.state, AppState::Quitting) {
                break;
            }

            // === Calcul du frame timing ===
            let frame_duration = Duration::from_secs_f64(
                1.0 / self.config.load().target_fps as f64
            );
            let now = Instant::now();
            let elapsed = now - last_frame;

            if elapsed < frame_duration {
                // Dormir le temps restant, mais rester réactif aux événements
                let remaining = frame_duration - elapsed;
                if event::poll(remaining)? {
                    self.handle_event(event::read()?);
                }
                continue;
            }
            last_frame = now;

            // === Polling événements non-bloquant ===
            while event::poll(Duration::ZERO)? {
                self.handle_event(event::read()?);
            }

            // === Vérifier resize terminal ===
            self.check_resize()?;

            // === Lire audio features (non-bloquant) ===
            let audio_features = self.audio_output
                .as_mut()
                .map(|out| *out.read());

            // === Lire frame source ===
            if let Some(ref rx) = self.frame_rx {
                if let Ok(frame) = rx.try_recv() {
                    self.current_frame = Some(frame);
                }
            }

            // === Skip si pas de frame source ===
            let Some(ref source_frame) = self.current_frame else { continue };

            // === Resize image vers dimensions terminal ===
            af_source::resize::resize_into(
                source_frame,
                &mut self.resized_frame,
            );

            // === Appliquer audio mappings à la config ===
            let config = self.config.load();
            let mut render_config = (**config).clone();
            if let Some(ref features) = audio_features {
                apply_audio_mappings(&mut render_config, features);
            }

            // === Conversion ASCII ===
            match render_config.render_mode {
                RenderMode::Ascii => {
                    af_ascii::compositor::process(
                        &self.resized_frame,
                        audio_features.as_ref(),
                        &render_config,
                        &mut self.grid,
                    );
                }
                RenderMode::HalfBlock => {
                    af_ascii::halfblock::process(
                        &self.resized_frame,
                        &render_config,
                        &mut self.grid,
                    );
                }
                RenderMode::Braille => {
                    af_ascii::braille::process_braille(
                        &self.resized_frame,
                        &render_config,
                        &mut self.grid,
                    );
                }
                RenderMode::Quadrant => {
                    af_ascii::quadrant::process(
                        &self.resized_frame,
                        &render_config,
                        &mut self.grid,
                    );
                }
            }

            // === Rendu terminal ===
            self.fps_counter.tick();
            terminal.draw(|frame| {
                af_render::ui::draw(
                    frame,
                    &self.grid,
                    &render_config,
                    audio_features.as_ref(),
                    &self.fps_counter,
                    self.sidebar_dirty,
                    &self.state,
                );
                self.sidebar_dirty = false;
            })?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        if let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = event {
            match (&self.state, code) {
                (_, KeyCode::Char('q')) | (AppState::Running, KeyCode::Esc) => {
                    self.state = AppState::Quitting;
                }
                (AppState::Help, KeyCode::Esc) | (_, KeyCode::Char('?')) => {
                    self.state = if matches!(self.state, AppState::Help) {
                        AppState::Running
                    } else {
                        AppState::Help
                    };
                    self.sidebar_dirty = true;
                }
                (_, KeyCode::Char(' ')) => {
                    self.state = if matches!(self.state, AppState::Paused) {
                        AppState::Running
                    } else {
                        AppState::Paused
                    };
                    self.sidebar_dirty = true;
                }
                (_, KeyCode::Tab) => {
                    let config = self.config.load();
                    let mut new = (**config).clone();
                    new.render_mode = match new.render_mode {
                        RenderMode::Ascii     => RenderMode::HalfBlock,
                        RenderMode::HalfBlock => RenderMode::Braille,
                        RenderMode::Braille   => RenderMode::Quadrant,
                        RenderMode::Quadrant  => RenderMode::Ascii,
                    };
                    self.config.store(std::sync::Arc::new(new));
                    self.sidebar_dirty = true;
                }
                (_, KeyCode::Char('c')) => {
                    self.toggle_config(|c| c.color_enabled = !c.color_enabled);
                }
                (_, KeyCode::Char('i')) => {
                    self.toggle_config(|c| c.invert = !c.invert);
                }
                (_, KeyCode::Char('e')) => {
                    self.toggle_config(|c| {
                        c.edge_threshold = if c.edge_threshold > 0.0 { 0.0 } else { 0.3 };
                    });
                }
                (_, KeyCode::Char('s')) => {
                    self.toggle_config(|c| c.shape_matching = !c.shape_matching);
                }
                (_, KeyCode::Up) => {
                    self.toggle_config(|c| {
                        c.audio_sensitivity = (c.audio_sensitivity + 0.1).min(5.0);
                    });
                }
                (_, KeyCode::Down) => {
                    self.toggle_config(|c| {
                        c.audio_sensitivity = (c.audio_sensitivity - 0.1).max(0.0);
                    });
                }
                _ => {}
            }
        }
    }

    fn toggle_config(&mut self, mutate: impl FnOnce(&mut RenderConfig)) {
        let config = self.config.load();
        let mut new = (**config).clone();
        mutate(&mut new);
        self.config.store(std::sync::Arc::new(new));
        self.sidebar_dirty = true;
    }
}
```

---

## 30. CONVERSION RGB ↔ HSV — af-core/src/color.rs

Implémentation complète requise. Ne pas dépendre d'une crate externe pour cela.

```rust
/// Convertit RGB [0,255] → HSV. H ∈ [0.0, 1.0), S ∈ [0.0, 1.0], V ∈ [0.0, 1.0].
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        (((g - b) / delta) % 6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let h = if h < 0.0 { h + 1.0 } else { h };

    (h, s, v)
}

/// Convertit HSV → RGB [0,255]. H ∈ [0.0, 1.0), S ∈ [0.0, 1.0], V ∈ [0.0, 1.0].
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h * 6.0;
    let i = h.floor() as u32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Technique HSV Bright : force V=1.0, le char ASCII encode la luminance.
/// Produit des couleurs vibrantes sur fond noir.
pub fn apply_hsv_bright(r: u8, g: u8, b: u8, saturation_boost: f32) -> (u8, u8, u8) {
    let (h, s, _v) = rgb_to_hsv(r, g, b);
    let s = (s * saturation_boost).min(1.0);
    hsv_to_rgb(h, s, 1.0)
}
```

---

## 31. AUDIO MAPPING DYNAMIQUE — af-app/src/pipeline.rs

```rust
/// Applique les mappings audio à une copie de la config avant le rendu.
pub fn apply_audio_mappings(config: &mut RenderConfig, features: &AudioFeatures) {
    let sensitivity = config.audio_sensitivity;

    for mapping in &config.audio_mappings {
        let source_value = match mapping.source.as_str() {
            "rms"              => features.rms,
            "peak"             => features.peak,
            "sub_bass"         => features.sub_bass,
            "bass"             => features.bass,
            "low_mid"          => features.low_mid,
            "mid"              => features.mid,
            "high_mid"         => features.high_mid,
            "presence"         => features.presence,
            "brilliance"       => features.brilliance,
            "spectral_centroid" => features.spectral_centroid,
            "spectral_flux"    => features.spectral_flux,
            "spectral_flatness" => features.spectral_flatness,
            "onset"            => if features.onset { 1.0 } else { 0.0 },
            "beat_phase"       => features.beat_phase,
            "bpm"              => features.bpm / 200.0, // Normaliser
            _                  => { log::warn!("Source audio inconnue : {}", mapping.source); 0.0 }
        };

        let delta = source_value * mapping.amount * sensitivity + mapping.offset;

        match mapping.target.as_str() {
            "edge_threshold" => config.edge_threshold = (config.edge_threshold + delta).clamp(0.0, 1.0),
            "edge_mix"       => config.edge_mix = (config.edge_mix + delta).clamp(0.0, 1.0),
            "contrast"       => config.contrast = (config.contrast + delta).clamp(0.1, 3.0),
            "brightness"     => config.brightness = (config.brightness + delta).clamp(-1.0, 1.0),
            "saturation"     => config.saturation = (config.saturation + delta).clamp(0.0, 3.0),
            "invert"         => { if delta > 0.5 { config.invert = !config.invert; } }
            _ => { log::warn!("Cible de mapping inconnue : {}", mapping.target); }
        }
    }
}
```

---

## 32. RÉSUMÉ DES FICHIERS À CRÉER — CHECKLIST COMPLÈTE

L'agent doit créer exactement ces fichiers, dans cet ordre (par phase).

### Phase 1

```
asciiforge/Cargo.toml
asciiforge/SPEC.md                           (copie de ce document)
asciiforge/config/default.toml               (section 8)
asciiforge/crates/af-core/Cargo.toml
asciiforge/crates/af-core/src/lib.rs
asciiforge/crates/af-core/src/config.rs      (RenderConfig + load/save)
asciiforge/crates/af-core/src/frame.rs       (FrameBuffer)
asciiforge/crates/af-core/src/charset.rs     (LuminanceLut, charsets constants)
asciiforge/crates/af-core/src/color.rs       (RGB↔HSV, section 30)
asciiforge/crates/af-core/src/error.rs       (CoreError, section 17.1)
asciiforge/crates/af-core/src/traits.rs      (Source, Processor, AudioAnalyzer)
asciiforge/crates/af-render/Cargo.toml
asciiforge/crates/af-render/src/lib.rs
asciiforge/crates/af-render/src/canvas.rs    (écriture directe buffer)
asciiforge/crates/af-render/src/ui.rs        (layout)
asciiforge/crates/af-render/src/widgets.rs   (stub vide)
asciiforge/crates/af-render/src/fps.rs       (FpsCounter)
asciiforge/crates/af-render/src/effects.rs   (stub vide)
asciiforge/crates/af-app/Cargo.toml
asciiforge/crates/af-app/src/main.rs         (section 19)
asciiforge/crates/af-app/src/app.rs          (section 18 + 29)
asciiforge/crates/af-app/src/cli.rs          (section 16)
asciiforge/crates/af-app/src/pipeline.rs     (stub)
asciiforge/crates/af-app/src/hotreload.rs    (section 20)
```

### Phase 2

```
asciiforge/crates/af-source/Cargo.toml
asciiforge/crates/af-source/src/lib.rs
asciiforge/crates/af-source/src/image.rs
asciiforge/crates/af-source/src/resize.rs
asciiforge/crates/af-ascii/Cargo.toml
asciiforge/crates/af-ascii/src/lib.rs
asciiforge/crates/af-ascii/src/luminance.rs
asciiforge/crates/af-ascii/src/color_map.rs
asciiforge/crates/af-ascii/src/compositor.rs (délègue au mode actif)
```

### Phase 3

```
asciiforge/crates/af-ascii/src/halfblock.rs  (section 6.3.1 mode HalfBlock)
asciiforge/crates/af-ascii/src/braille.rs    (section 24)
asciiforge/crates/af-ascii/src/quadrant.rs   (section 25)
asciiforge/crates/af-ascii/src/edge.rs       (section 6.2.2)
asciiforge/crates/af-ascii/src/shape_match.rs (section 6.2.3)
```

### Phase 4

```
asciiforge/crates/af-audio/Cargo.toml
asciiforge/crates/af-audio/src/lib.rs
asciiforge/crates/af-audio/src/capture.rs    (section 6.1.1)
asciiforge/crates/af-audio/src/fft.rs        (section 6.1.3)
asciiforge/crates/af-audio/src/features.rs   (section 6.1.4)
asciiforge/crates/af-audio/src/beat.rs       (section 6.1.4 onset+BPM)
asciiforge/crates/af-audio/src/smoothing.rs  (section 6.1.5)
asciiforge/crates/af-audio/src/state.rs      (AudioFeatures → triple_buffer wiring)
asciiforge/crates/af-audio/src/error.rs      (section 17.2)
```

### Phase 5

```
asciiforge/crates/af-app/src/pipeline.rs     (compléter avec audio wiring, section 31)
```

### Phase 6

```
asciiforge/crates/af-audio/src/decode.rs     (section 6.1.2)
```

### Phase 7

```
asciiforge/crates/af-source/src/video.rs
```

### Phase 8

```
asciiforge/crates/af-source/src/webcam.rs
asciiforge/crates/af-source/src/procedural.rs (section 26)
```

### Phase 9

```
asciiforge/crates/af-app/src/midi.rs
asciiforge/README.md
asciiforge/tests/pipeline_test.rs            (section 27.2)
asciiforge/benches/ascii_bench.rs            (section 27.3)
```

---

## 33. FPS COUNTER — af-render/src/fps.rs

```rust
use std::time::Instant;
use std::collections::VecDeque;

/// Compteur FPS par fenêtre glissante. Zéro allocation après init.
pub struct FpsCounter {
    /// Timestamps des dernières N frames.
    timestamps: VecDeque<Instant>,
    /// Taille de la fenêtre (nombre de frames à moyenner).
    window: usize,
    /// FPS calculé, mis à jour à chaque tick.
    fps: f64,
    /// Temps de la dernière frame en ms (pour debug).
    pub frame_time_ms: f64,
}

impl FpsCounter {
    pub fn new(window: usize) -> Self {
        Self {
            timestamps: VecDeque::with_capacity(window + 1),
            window,
            fps: 0.0,
            frame_time_ms: 0.0,
        }
    }

    /// Appeler une fois par frame, APRÈS le rendu.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(&last) = self.timestamps.back() {
            self.frame_time_ms = now.duration_since(last).as_secs_f64() * 1000.0;
        }
        self.timestamps.push_back(now);
        if self.timestamps.len() > self.window {
            self.timestamps.pop_front();
        }
        if self.timestamps.len() >= 2 {
            let first = self.timestamps.front().copied().unwrap_or(now);
            let duration = now.duration_since(first);
            let secs = duration.as_secs_f64();
            if secs > 0.0 {
                self.fps = (self.timestamps.len() - 1) as f64 / secs;
            }
        }
    }

    /// FPS moyen sur la fenêtre.
    pub fn fps(&self) -> f64 {
        self.fps
    }
}
```

**Note** : `VecDeque::with_capacity` pré-alloue. Après le warm-up (N frames), plus aucune allocation.

---

## 34. CONFIG LOADER — af-core/src/config.rs

```rust
use std::path::Path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// RenderConfig, AudioMapping, RenderMode, ColorMode : section 5.4 (tous dans ce fichier)

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Ascii,
            charset: " .:-=+*#%@".to_string(),
            invert: false,
            color_enabled: true,
            edge_threshold: 0.3,
            edge_mix: 0.5,
            shape_matching: false,
            aspect_ratio: 2.0,
            color_mode: ColorMode::HsvBright,
            saturation: 1.2,
            contrast: 1.0,
            brightness: 0.0,
            audio_mappings: vec![
                AudioMapping { source: "bass".into(), target: "edge_threshold".into(), amount: 0.3, offset: 0.0 },
                AudioMapping { source: "spectral_flux".into(), target: "contrast".into(), amount: 0.5, offset: 0.0 },
                AudioMapping { source: "onset".into(), target: "invert".into(), amount: 1.0, offset: 0.0 },
                AudioMapping { source: "rms".into(), target: "brightness".into(), amount: 0.2, offset: 0.0 },
            ],
            audio_smoothing: 0.7,
            audio_sensitivity: 1.0,
            target_fps: 30,
        }
    }
}

/// Structure TOML intermédiaire pour désérialisation avec valeurs optionnelles.
/// Permet de n'override que les champs présents dans le fichier.
#[derive(Deserialize)]
struct ConfigFile {
    render: RenderSection,
    audio: Option<AudioSection>,
}

#[derive(Deserialize)]
struct RenderSection {
    render_mode: Option<RenderMode>,
    charset: Option<String>,
    invert: Option<bool>,
    color_enabled: Option<bool>,
    edge_threshold: Option<f32>,
    edge_mix: Option<f32>,
    shape_matching: Option<bool>,
    aspect_ratio: Option<f32>,
    color_mode: Option<ColorMode>,
    saturation: Option<f32>,
    contrast: Option<f32>,
    brightness: Option<f32>,
    target_fps: Option<u32>,
}

#[derive(Deserialize)]
struct AudioSection {
    smoothing: Option<f32>,
    sensitivity: Option<f32>,
    mappings: Option<Vec<AudioMapping>>,
}

/// Charge un fichier TOML et fusionne avec les valeurs par défaut.
pub fn load_config(path: &Path) -> Result<RenderConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Impossible de lire {}", path.display()))?;

    let file: ConfigFile = toml::from_str(&content)
        .with_context(|| format!("Erreur de parsing TOML dans {}", path.display()))?;

    let mut config = RenderConfig::default();

    let r = file.render;
    if let Some(v) = r.render_mode { config.render_mode = v; }
    if let Some(v) = r.charset { config.charset = v; }
    if let Some(v) = r.invert { config.invert = v; }
    if let Some(v) = r.color_enabled { config.color_enabled = v; }
    if let Some(v) = r.edge_threshold { config.edge_threshold = v; }
    if let Some(v) = r.edge_mix { config.edge_mix = v; }
    if let Some(v) = r.shape_matching { config.shape_matching = v; }
    if let Some(v) = r.aspect_ratio { config.aspect_ratio = v; }
    if let Some(v) = r.color_mode { config.color_mode = v; }
    if let Some(v) = r.saturation { config.saturation = v; }
    if let Some(v) = r.contrast { config.contrast = v; }
    if let Some(v) = r.brightness { config.brightness = v; }
    if let Some(v) = r.target_fps { config.target_fps = v; }

    if let Some(a) = file.audio {
        if let Some(v) = a.smoothing { config.audio_smoothing = v; }
        if let Some(v) = a.sensitivity { config.audio_sensitivity = v; }
        if let Some(v) = a.mappings { config.audio_mappings = v; }
    }

    Ok(config)
}
```

---

## 35. RESIZE PIPELINE — af-source/src/resize.rs

```rust
use af_core::frame::FrameBuffer;
use fast_image_resize::{self as fir, images::Image};

/// Resizer réutilisable. Pré-alloue les buffers internes.
pub struct Resizer {
    resizer: fir::Resizer,
}

impl Resizer {
    pub fn new() -> Self {
        Self { resizer: fir::Resizer::new() }
    }

    /// Resize `src` dans `dst` (pré-alloué aux dimensions cibles).
    /// CONTRAT : `dst` doit déjà avoir les bonnes dimensions.
    /// Ne réalloue jamais `dst.data`.
    pub fn resize_into(&mut self, src: &FrameBuffer, dst: &mut FrameBuffer) {
        if src.width == dst.width && src.height == dst.height {
            dst.data.copy_from_slice(&src.data);
            return;
        }

        let src_image = Image::from_slice_u8(
            src.width, src.height, &src.data, fir::PixelType::U8x4,
        ).expect("src dimensions mismatch");

        let mut dst_image = Image::from_slice_u8_mut(
            dst.width, dst.height, &mut dst.data, fir::PixelType::U8x4,
        ).expect("dst dimensions mismatch");

        self.resizer.resize(
            &src_image, &mut dst_image,
            Some(&fir::ResizeOptions::new().resize_alg(
                fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3)
            )),
        ).expect("resize failed");
        // DECISION: expect() acceptable car un échec ici est un bug, pas une erreur runtime.
    }
}

/// Convenance pour usage one-shot. NE PAS utiliser dans le hot path.
pub fn resize_frame(src: &FrameBuffer, width: u32, height: u32) -> FrameBuffer {
    let mut dst = FrameBuffer::new(width, height);
    let mut resizer = Resizer::new();
    resizer.resize_into(src, &mut dst);
    dst
}
```

**Note API** : `fast_image_resize` v6 peut avoir des signatures légèrement différentes. L'agent doit adapter si l'API a changé, en respectant le contrat : pas d'allocation dans `resize_into`.

---

## 36. IMAGE SOURCE — af-source/src/image.rs

```rust
use std::path::Path;
use std::sync::Arc;
use af_core::frame::FrameBuffer;
use af_core::traits::Source;
use anyhow::{Context, Result};

/// Source d'image statique. Retourne toujours la même frame.
pub struct ImageSource {
    frame: Arc<FrameBuffer>,
}

impl ImageSource {
    pub fn new(path: &Path) -> Result<Self> {
        let img = image::open(path)
            .with_context(|| format!("Impossible de charger {}", path.display()))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Self {
            frame: Arc::new(FrameBuffer { data: rgba.into_raw(), width, height }),
        })
    }
}

impl Source for ImageSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        Some(Arc::clone(&self.frame))
    }
    fn native_size(&self) -> (u32, u32) {
        (self.frame.width, self.frame.height)
    }
    fn is_live(&self) -> bool { false }
}

/// Convenance pour les tests.
pub fn load_image(path: &str) -> Result<FrameBuffer> {
    let img = image::open(path).with_context(|| format!("Impossible de charger {path}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(FrameBuffer { data: rgba.into_raw(), width: w, height: h })
}
```

---

## 37. HALF-BLOCK RENDERER — af-ascii/src/halfblock.rs

```rust
use af_core::frame::FrameBuffer;
use af_core::config::RenderConfig;
use af_core::grid::{AsciiGrid, AsciiCell};
use crate::color_map;

/// Rendu half-block : '▄' avec fg = pixel bas, bg = pixel haut.
/// Résolution effective : 1×2 par cellule terminal.
/// CONTRAT : `frame` doit avoir dimensions (grid.width, grid.height * 2).
pub fn process(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let px = cx as u32;
            let py_top = cy as u32 * 2;
            let py_bot = py_top + 1;

            let (tr, tg, tb) = pixel_color(frame, px, py_top, config);
            let (br, bg_c, bb) = pixel_color(frame, px, py_bot, config);

            grid.set(cx, cy, AsciiCell {
                ch: '▄',
                fg: (br, bg_c, bb),
                bg: (tr, tg, tb),
            });
        }
    }
}

#[inline(always)]
fn pixel_color(frame: &FrameBuffer, x: u32, y: u32, config: &RenderConfig) -> (u8, u8, u8) {
    if x >= frame.width || y >= frame.height { return (0, 0, 0); }
    let (r, g, b, _) = frame.pixel(x, y);
    color_map::map_color(r, g, b, config)
}
```

---

## 38. COMPOSITOR — af-ascii/src/compositor.rs

Point d'entrée pour le mode ASCII. Orchestre luminance, edges, shape-matching, couleur.

```rust
use af_core::frame::FrameBuffer;
use af_core::config::RenderConfig;
use af_core::grid::{AsciiGrid, AsciiCell};
use af_core::audio::AudioFeatures;
use af_core::charset::LuminanceLut;
use crate::edge;
use crate::shape_match::ShapeMatcher;
use crate::color_map;

/// État du compositor, pré-alloué.
pub struct Compositor {
    lum_lut: LuminanceLut,
    shape_matcher: Option<ShapeMatcher>,
    current_charset: String,
}

impl Compositor {
    pub fn new(charset: &str) -> Self {
        Self {
            lum_lut: LuminanceLut::new(charset),
            shape_matcher: None,
            current_charset: charset.to_string(),
        }
    }

    fn update_if_needed(&mut self, config: &RenderConfig) {
        if config.charset != self.current_charset {
            self.lum_lut = LuminanceLut::new(&config.charset);
            self.shape_matcher = None;
            self.current_charset = config.charset.clone();
        }
        if config.shape_matching && self.shape_matcher.is_none() {
            self.shape_matcher = Some(ShapeMatcher::new(&config.charset));
        }
    }
}

pub fn process(
    compositor: &mut Compositor,
    frame: &FrameBuffer,
    _audio: Option<&AudioFeatures>,
    config: &RenderConfig,
    grid: &mut AsciiGrid,
) {
    compositor.update_if_needed(config);

    let use_edges = config.edge_threshold > 0.0;
    let use_shapes = config.shape_matching && compositor.shape_matcher.is_some();

    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let px = cx as u32;
            let py = cy as u32;

            if px >= frame.width || py >= frame.height {
                grid.set(cx, cy, AsciiCell::default());
                continue;
            }

            let luma = frame.luminance(px, py);
            let (r, g, b, _) = frame.pixel(px, py);

            // Edge detection
            let edge_char = if use_edges {
                edge::detect_edge(frame, px, py, config.edge_threshold)
            } else {
                None
            };

            // Fill character
            let fill_ch = if use_shapes {
                compositor.shape_matcher.as_ref()
                    .map(|sm| sm.match_cell(frame, px, py))
                    .unwrap_or_else(|| compositor.lum_lut.map(if config.invert { 255 - luma } else { luma }))
            } else {
                compositor.lum_lut.map(if config.invert { 255 - luma } else { luma })
            };

            // Mix edge et fill
            let ch = match edge_char {
                Some(ec) if config.edge_mix >= 1.0 => ec,
                Some(ec) => {
                    // Déterministe : seuil basé sur edge_mix
                    if (luma as f32 / 255.0) < config.edge_mix { ec } else { fill_ch }
                }
                None => fill_ch,
            };

            let fg = color_map::map_color(r, g, b, config);
            grid.set(cx, cy, AsciiCell { ch, fg, bg: (0, 0, 0) });
        }
    }
}
```

---

## 39. AUDIO STATE WIRING — af-audio/src/state.rs

```rust
use af_core::audio::AudioFeatures;
use triple_buffer::triple_buffer;
use rtrb::Consumer;
use crate::fft::FftPipeline;
use crate::smoothing::SmootherBank;

/// Crée le couple (writer, reader) pour le triple buffer AudioFeatures.
pub fn create_audio_channel() -> (
    triple_buffer::Input<AudioFeatures>,
    triple_buffer::Output<AudioFeatures>,
) {
    triple_buffer(&AudioFeatures::default())
}

/// Thread d'analyse audio. Boucle tant que le consumer est vivant.
pub fn analysis_thread(
    mut consumer: Consumer<f32>,
    mut audio_writer: triple_buffer::Input<AudioFeatures>,
    sample_rate: u32,
) {
    let fft_size = 2048;
    let hop_size = 512;
    let mut pipeline = FftPipeline::new(sample_rate, fft_size);
    let mut smoothers = SmootherBank::new(0.7);
    let mut features = AudioFeatures::default();
    let mut accumulator = Vec::with_capacity(fft_size);

    loop {
        let available = consumer.slots();
        if available == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }

        match consumer.read_chunk(available.min(hop_size)) {
            Ok(chunk) => {
                let (a, b) = chunk.as_slices();
                accumulator.extend_from_slice(a);
                accumulator.extend_from_slice(b);
                chunk.commit_all();
            }
            Err(_) => {
                log::info!("Audio analysis: producer disconnected, exiting.");
                return;
            }
        }

        while accumulator.len() >= fft_size {
            pipeline.analyze(&accumulator[..fft_size], &mut features);
            smoothers.apply(&mut features);
            audio_writer.write(features);
            accumulator.drain(..hop_size);
        }
    }
}
```

---

## 40. PER-CRATE CARGO.TOML — CONTENU EXACT

### af-core/Cargo.toml

```toml
[package]
name = "af-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
toml = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }

[lints]
workspace = true
```

### af-audio/Cargo.toml

```toml
[package]
name = "af-audio"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
af-core = { path = "../af-core" }
cpal = { workspace = true }
realfft = { workspace = true }
symphonia = { workspace = true }
rtrb = { workspace = true }
triple_buffer = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }

[lints]
workspace = true
```

### af-source/Cargo.toml

```toml
[package]
name = "af-source"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
af-core = { path = "../af-core" }
image = { workspace = true }
fast_image_resize = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }

ffmpeg-the-third = { version = "2", optional = true }
nokhwa = { version = "0.10", optional = true }
noise = { version = "0.9", optional = true }
glam = { workspace = true, optional = true }

[features]
default = ["image-source"]
image-source = []
video = ["dep:ffmpeg-the-third"]
webcam = ["dep:nokhwa"]
procedural = ["dep:noise", "dep:glam"]

[lints]
workspace = true
```

### af-ascii/Cargo.toml

```toml
[package]
name = "af-ascii"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
af-core = { path = "../af-core" }
log = { workspace = true }

[lints]
workspace = true
```

### af-render/Cargo.toml

```toml
[package]
name = "af-render"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
af-core = { path = "../af-core" }
ratatui = { workspace = true }
crossterm = { workspace = true }
log = { workspace = true }

[lints]
workspace = true
```

### af-app/Cargo.toml

```toml
[package]
name = "af-app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "asciiforge"
path = "src/main.rs"

[dependencies]
af-core = { path = "../af-core" }
af-audio = { path = "../af-audio" }
af-source = { path = "../af-source" }
af-ascii = { path = "../af-ascii" }
af-render = { path = "../af-render" }
clap = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }
arc-swap = { workspace = true }
triple_buffer = { workspace = true }
flume = { workspace = true }
crossterm = { workspace = true }
ratatui = { workspace = true }
notify = { workspace = true }
ctrlc = { workspace = true }
midir = { workspace = true, optional = true }

[features]
default = ["image-source"]
image-source = ["af-source/image-source"]
full = ["video", "webcam", "procedural", "midi"]
video = ["af-source/video"]
webcam = ["af-source/webcam"]
procedural = ["af-source/procedural"]
midi = ["dep:midir"]

[lints]
workspace = true
```

---

## 41. LIB.RS RE-EXPORTS — CHAQUE CRATE

### af-core/src/lib.rs

```rust
pub mod config;
pub mod frame;
pub mod charset;
pub mod color;
pub mod error;
pub mod traits;

pub use config::RenderConfig;
pub use frame::{FrameBuffer, AsciiGrid, AsciiCell, AudioFeatures};
pub use charset::LuminanceLut;
pub use error::CoreError;

/// Re-exports pour accès par chemin sémantique.
pub mod grid {
    pub use crate::frame::{AsciiGrid, AsciiCell};
}
pub mod audio {
    pub use crate::frame::AudioFeatures;
}
```

**Note** : `AsciiGrid`, `AsciiCell`, `AudioFeatures` sont tous dans `frame.rs` pour simplicité. L'agent peut les séparer, mais les re-exports ci-dessus doivent exister.

### af-audio/src/lib.rs

```rust
pub mod capture;
pub mod decode;
pub mod fft;
pub mod features;
pub mod beat;
pub mod smoothing;
pub mod state;
pub mod error;
```

### af-source/src/lib.rs

```rust
pub mod image;
pub mod resize;

#[cfg(feature = "video")]
pub mod video;
#[cfg(feature = "webcam")]
pub mod webcam;
#[cfg(feature = "procedural")]
pub mod procedural;
```

### af-ascii/src/lib.rs

```rust
pub mod luminance;
pub mod edge;
pub mod shape_match;
pub mod braille;
pub mod halfblock;
pub mod quadrant;
pub mod color_map;
pub mod compositor;
```

### af-render/src/lib.rs

```rust
pub mod canvas;
pub mod ui;
pub mod widgets;
pub mod fps;
pub mod effects;
```

---

## 42. MIDI CONTROLLER — af-app/src/midi.rs

Feature-gated derrière `midi`. Phase 9.

```rust
#[cfg(feature = "midi")]
use midir::{MidiInput, MidiInputConnection};
use std::sync::Arc;
use arc_swap::ArcSwap;
use af_core::config::RenderConfig;

pub struct MidiMapping {
    pub cc: u8,
    pub target: String,
    pub min: f32,
    pub max: f32,
}

pub fn default_midi_mappings() -> Vec<MidiMapping> {
    vec![
        MidiMapping { cc: 1, target: "audio_sensitivity".into(), min: 0.0, max: 5.0 },
        MidiMapping { cc: 2, target: "edge_threshold".into(),    min: 0.0, max: 1.0 },
        MidiMapping { cc: 3, target: "edge_mix".into(),          min: 0.0, max: 1.0 },
        MidiMapping { cc: 4, target: "contrast".into(),          min: 0.1, max: 3.0 },
        MidiMapping { cc: 5, target: "brightness".into(),        min: -1.0, max: 1.0 },
        MidiMapping { cc: 6, target: "saturation".into(),        min: 0.0, max: 3.0 },
        MidiMapping { cc: 7, target: "audio_smoothing".into(),   min: 0.0, max: 1.0 },
    ]
}

#[cfg(feature = "midi")]
pub fn start_midi_listener(
    config: Arc<ArcSwap<RenderConfig>>,
    mappings: Vec<MidiMapping>,
) -> anyhow::Result<MidiInputConnection<()>> {
    let midi_in = MidiInput::new("asciiforge-midi")?;
    let ports = midi_in.ports();
    let port = ports.first()
        .ok_or_else(|| anyhow::anyhow!("Aucun port MIDI d'entrée trouvé"))?;
    log::info!("MIDI connecté à : {}", midi_in.port_name(port).unwrap_or_default());

    let conn = midi_in.connect(port, "asciiforge-midi-in", move |_ts, msg, _| {
        if msg.len() == 3 && (msg[0] & 0xF0) == 0xB0 {
            let cc = msg[1];
            let value = msg[2] as f32 / 127.0;
            for mapping in &mappings {
                if mapping.cc == cc {
                    let mapped = mapping.min + value * (mapping.max - mapping.min);
                    let current = config.load();
                    let mut new_config = (**current).clone();
                    match mapping.target.as_str() {
                        "audio_sensitivity" => new_config.audio_sensitivity = mapped,
                        "edge_threshold"    => new_config.edge_threshold = mapped,
                        "edge_mix"          => new_config.edge_mix = mapped,
                        "contrast"          => new_config.contrast = mapped,
                        "brightness"        => new_config.brightness = mapped,
                        "saturation"        => new_config.saturation = mapped,
                        "audio_smoothing"   => new_config.audio_smoothing = mapped,
                        _ => {}
                    }
                    config.store(Arc::new(new_config));
                }
            }
        }
    }, ())?;
    Ok(conn)
}
```

---

## 43. PRESETS

```
config/
├── default.toml
└── presets/
    ├── ambient.toml
    ├── aggressive.toml
    ├── minimal.toml
    ├── retro.toml
    └── psychedelic.toml
```

### config/presets/ambient.toml

```toml
[render]
charset = " .:-=+*#%@"
color_mode = "HsvBright"
edge_threshold = 0.1
edge_mix = 0.3
saturation = 0.8
contrast = 0.8
target_fps = 30

[audio]
smoothing = 0.9
sensitivity = 0.5

[[audio.mappings]]
source = "rms"
target = "brightness"
amount = 0.1
offset = 0.0

[[audio.mappings]]
source = "spectral_centroid"
target = "saturation"
amount = 0.3
offset = 0.0
```

### config/presets/aggressive.toml

```toml
[render]
charset = " .:░▒▓█"
color_mode = "HsvBright"
edge_threshold = 0.5
edge_mix = 0.8
saturation = 1.8
contrast = 1.5
target_fps = 60

[audio]
smoothing = 0.3
sensitivity = 2.5

[[audio.mappings]]
source = "bass"
target = "edge_threshold"
amount = 0.6
offset = 0.0

[[audio.mappings]]
source = "spectral_flux"
target = "contrast"
amount = 1.0
offset = 0.0

[[audio.mappings]]
source = "onset"
target = "invert"
amount = 1.0
offset = 0.0
```

### config/presets/minimal.toml

```toml
[render]
render_mode = "Braille"
color_enabled = false
edge_threshold = 0.0
shape_matching = false
target_fps = 30

[audio]
smoothing = 0.5
sensitivity = 1.0
```

### config/presets/retro.toml

```toml
[render]
charset = "@%#*+=-:. "
color_enabled = false
invert = true
edge_threshold = 0.4
edge_mix = 0.7
shape_matching = true
target_fps = 30

[audio]
smoothing = 0.6
sensitivity = 1.5

[[audio.mappings]]
source = "bass"
target = "contrast"
amount = 0.4
offset = 0.0
```

### config/presets/psychedelic.toml

```toml
[render]
render_mode = "HalfBlock"
color_mode = "HsvBright"
saturation = 2.0
contrast = 1.3
brightness = 0.1
target_fps = 60

[audio]
smoothing = 0.4
sensitivity = 3.0

[[audio.mappings]]
source = "spectral_centroid"
target = "saturation"
amount = 1.5
offset = 0.0

[[audio.mappings]]
source = "spectral_flux"
target = "brightness"
amount = 0.8
offset = 0.0

[[audio.mappings]]
source = "bass"
target = "contrast"
amount = 0.7
offset = 0.0

[[audio.mappings]]
source = "onset"
target = "invert"
amount = 1.0
offset = 0.0
```

Chargement dans CLI :
```rust
fn resolve_config(cli: &Cli) -> Result<RenderConfig> {
    if let Some(ref name) = cli.preset {
        let path = PathBuf::from(format!("config/presets/{name}.toml"));
        if path.exists() {
            af_core::config::load_config(&path)
        } else {
            anyhow::bail!("Preset inconnu : {name}. Disponibles : ambient, aggressive, minimal, retro, psychedelic");
        }
    } else {
        af_core::config::load_config(&cli.config)
    }
}
```

---

## 44. PIPELINE START — af-app/src/pipeline.rs (complet)

```rust
use std::sync::Arc;
use std::thread;
use std::path::PathBuf;
use arc_swap::ArcSwap;
use anyhow::Result;
use af_core::config::RenderConfig;
use af_core::frame::FrameBuffer;
use af_core::audio::AudioFeatures;

pub fn start_audio(
    audio_arg: &str,
    _config: &Arc<ArcSwap<RenderConfig>>,
) -> Result<triple_buffer::Output<AudioFeatures>> {
    let (audio_writer, audio_reader) = af_audio::state::create_audio_channel();
    let (producer, consumer) = rtrb::RingBuffer::new(4096);

    let sample_rate = if audio_arg == "mic" {
        af_audio::capture::start_capture(producer)?
    } else {
        let path = PathBuf::from(audio_arg);
        af_audio::decode::start_playback(&path, producer)?
    };

    thread::Builder::new()
        .name("audio-analysis".to_string())
        .spawn(move || {
            af_audio::state::analysis_thread(consumer, audio_writer, sample_rate);
        })?;

    Ok(audio_reader)
}

pub fn start_source(
    cli: &crate::cli::Cli,
) -> Result<Option<flume::Receiver<Arc<FrameBuffer>>>> {
    #[cfg(feature = "video")]
    if let Some(ref path) = cli.video {
        let (tx, rx) = flume::bounded(2);
        let path = path.clone();
        thread::Builder::new().name("video-source".into()).spawn(move || {
            if let Err(e) = af_source::video::decode_loop(&path, tx) {
                log::error!("Erreur vidéo : {e}");
            }
        })?;
        return Ok(Some(rx));
    }

    #[cfg(feature = "webcam")]
    if cli.webcam {
        let (tx, rx) = flume::bounded(2);
        thread::Builder::new().name("webcam-source".into()).spawn(move || {
            if let Err(e) = af_source::webcam::capture_loop(tx) {
                log::error!("Erreur webcam : {e}");
            }
        })?;
        return Ok(Some(rx));
    }

    #[cfg(feature = "procedural")]
    if let Some(ref proc_type) = cli.procedural {
        let (tx, rx) = flume::bounded(2);
        let pt = proc_type.clone();
        thread::Builder::new().name("procedural-source".into()).spawn(move || {
            if let Err(e) = af_source::procedural::generate_loop(&pt, tx) {
                log::error!("Erreur procédurale : {e}");
            }
        })?;
        return Ok(Some(rx));
    }

    Ok(None)
}

pub fn apply_audio_mappings(config: &mut RenderConfig, features: &AudioFeatures) {
    let sensitivity = config.audio_sensitivity;
    for mapping in &config.audio_mappings {
        let source_value = match mapping.source.as_str() {
            "rms"               => features.rms,
            "peak"              => features.peak,
            "sub_bass"          => features.sub_bass,
            "bass"              => features.bass,
            "low_mid"           => features.low_mid,
            "mid"               => features.mid,
            "high_mid"          => features.high_mid,
            "presence"          => features.presence,
            "brilliance"        => features.brilliance,
            "spectral_centroid" => features.spectral_centroid,
            "spectral_flux"     => features.spectral_flux,
            "spectral_flatness" => features.spectral_flatness,
            "onset"             => if features.onset { 1.0 } else { 0.0 },
            "beat_phase"        => features.beat_phase,
            "bpm"               => features.bpm / 200.0,
            _ => 0.0,
        };
        let delta = source_value * mapping.amount * sensitivity + mapping.offset;
        match mapping.target.as_str() {
            "edge_threshold" => config.edge_threshold = (config.edge_threshold + delta).clamp(0.0, 1.0),
            "edge_mix"       => config.edge_mix = (config.edge_mix + delta).clamp(0.0, 1.0),
            "contrast"       => config.contrast = (config.contrast + delta).clamp(0.1, 3.0),
            "brightness"     => config.brightness = (config.brightness + delta).clamp(-1.0, 1.0),
            "saturation"     => config.saturation = (config.saturation + delta).clamp(0.0, 3.0),
            "invert"         => { if delta > 0.5 { config.invert = !config.invert; } }
            _ => {}
        }
    }
}
```

---

## 45. AUDIO CAPTURE — af-audio/src/capture.rs (complet)

```rust
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, SampleFormat};
use rtrb::Producer;
use anyhow::{Context, Result};

pub fn start_capture(mut producer: Producer<f32>) -> Result<u32> {
    let host = cpal::default_host();
    let device = host.default_input_device()
        .context("Aucun périphérique audio d'entrée trouvé")?;
    let config = device.default_input_config()
        .context("Impossible d'obtenir la config audio")?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    log::info!("Audio capture: {} @ {}Hz, {} ch",
        device.name().unwrap_or_default(), sample_rate, channels);

    let err_fn = |err: cpal::StreamError| log::error!("Stream audio: {err}");

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if channels == 1 {
                    for &s in data { let _ = producer.push(s); }
                } else {
                    for chunk in data.chunks(channels) {
                        let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                        let _ = producer.push(mono);
                    }
                }
            }, err_fn, None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                if channels == 1 {
                    for &s in data { let _ = producer.push(s as f32 / i16::MAX as f32); }
                } else {
                    for chunk in data.chunks(channels) {
                        let mono = chunk.iter().map(|&s| s as f32 / i16::MAX as f32)
                            .sum::<f32>() / channels as f32;
                        let _ = producer.push(mono);
                    }
                }
            }, err_fn, None,
        )?,
        fmt => anyhow::bail!("Format audio non supporté : {fmt:?}"),
    };

    stream.play().context("Impossible de démarrer la capture")?;
    // DECISION: mem::forget acceptable — stream doit vivre jusqu'au exit du process.
    std::mem::forget(stream);
    Ok(sample_rate)
}
```

---

## 46. SMOOTHER BANK — af-audio/src/smoothing.rs (complet)

```rust
use af_core::audio::AudioFeatures;

#[derive(Clone)]
pub struct Ema { value: f32, alpha: f32 }
impl Ema {
    pub fn new(alpha: f32) -> Self { Self { value: 0.0, alpha } }
    #[inline(always)]
    pub fn update(&mut self, input: f32) -> f32 {
        self.value = self.alpha * input + (1.0 - self.alpha) * self.value;
        self.value
    }
    pub fn set_alpha(&mut self, alpha: f32) { self.alpha = alpha; }
}

#[derive(Clone)]
pub struct PeakHold { value: f32, decay: f32 }
impl PeakHold {
    pub fn new(decay: f32) -> Self { Self { value: 0.0, decay } }
    #[inline(always)]
    pub fn update(&mut self, input: f32) -> f32 {
        if input > self.value { self.value = input; } else { self.value *= self.decay; }
        self.value
    }
}

pub struct SmootherBank {
    rms: Ema, peak: PeakHold,
    sub_bass: Ema, bass: Ema, low_mid: Ema, mid: Ema,
    high_mid: Ema, presence: Ema, brilliance: Ema,
    spectral_centroid: Ema, spectral_flux: Ema, spectral_flatness: Ema,
    bpm: Ema, beat_phase: Ema,
    spectrum_bands: [Ema; 32],
}

impl SmootherBank {
    pub fn new(alpha: f32) -> Self {
        Self {
            rms: Ema::new(alpha), peak: PeakHold::new(0.95),
            sub_bass: Ema::new(alpha), bass: Ema::new(alpha),
            low_mid: Ema::new(alpha), mid: Ema::new(alpha),
            high_mid: Ema::new(alpha), presence: Ema::new(alpha),
            brilliance: Ema::new(alpha),
            spectral_centroid: Ema::new(alpha * 0.5),
            spectral_flux: Ema::new(alpha),
            spectral_flatness: Ema::new(alpha * 0.5),
            bpm: Ema::new(0.05), beat_phase: Ema::new(alpha),
            spectrum_bands: std::array::from_fn(|_| Ema::new(alpha)),
        }
    }

    pub fn apply(&mut self, f: &mut AudioFeatures) {
        f.rms = self.rms.update(f.rms);
        f.peak = self.peak.update(f.peak);
        f.sub_bass = self.sub_bass.update(f.sub_bass);
        f.bass = self.bass.update(f.bass);
        f.low_mid = self.low_mid.update(f.low_mid);
        f.mid = self.mid.update(f.mid);
        f.high_mid = self.high_mid.update(f.high_mid);
        f.presence = self.presence.update(f.presence);
        f.brilliance = self.brilliance.update(f.brilliance);
        f.spectral_centroid = self.spectral_centroid.update(f.spectral_centroid);
        f.spectral_flux = self.spectral_flux.update(f.spectral_flux);
        f.spectral_flatness = self.spectral_flatness.update(f.spectral_flatness);
        f.bpm = self.bpm.update(f.bpm);
        f.beat_phase = self.beat_phase.update(f.beat_phase);
        for (i, band) in f.spectrum_bands.iter_mut().enumerate() {
            *band = self.spectrum_bands[i].update(*band);
        }
    }
}
```

---

## 47. COLOR MAP — af-ascii/src/color_map.rs

```rust
use af_core::color;
use af_core::config::{ColorMode, RenderConfig};

#[inline(always)]
pub fn map_color(r: u8, g: u8, b: u8, config: &RenderConfig) -> (u8, u8, u8) {
    if !config.color_enabled {
        let luma = ((r as u32 * 2126 + g as u32 * 7152 + b as u32 * 722) / 10000) as u8;
        return (luma, luma, luma);
    }
    let (r, g, b) = apply_contrast_brightness(r, g, b, config.contrast, config.brightness);
    match config.color_mode {
        ColorMode::Direct => (r, g, b),
        ColorMode::HsvBright => color::apply_hsv_bright(r, g, b, config.saturation),
        ColorMode::Quantized => {
            let q = |v: u8| { let l = (v as u32 * 5 / 255) as u8; l * 51 };
            (q(r), q(g), q(b))
        }
    }
}

#[inline(always)]
fn apply_contrast_brightness(r: u8, g: u8, b: u8, contrast: f32, brightness: f32) -> (u8, u8, u8) {
    let adj = |v: u8| -> u8 {
        ((v as f32 - 128.0) * contrast + 128.0 + brightness * 255.0).clamp(0.0, 255.0) as u8
    };
    (adj(r), adj(g), adj(b))
}
```

---

## 48. EDGE DETECTION — af-ascii/src/edge.rs (complet)

```rust
use af_core::frame::FrameBuffer;

const EDGE_H: char = '─';
const EDGE_V: char = '│';
const EDGE_D1: char = '╱';
const EDGE_D2: char = '╲';

/// Détecte un edge via Sobel 3×3. Retourne Some(char) si magnitude > seuil.
#[inline]
pub fn detect_edge(frame: &FrameBuffer, x: u32, y: u32, threshold: f32) -> Option<char> {
    if x == 0 || y == 0 || x >= frame.width - 1 || y >= frame.height - 1 {
        return None;
    }
    let l = |dx: u32, dy: u32| frame.luminance(dx, dy) as i32;
    let tl = l(x-1,y-1); let tc = l(x,y-1); let tr = l(x+1,y-1);
    let ml = l(x-1,y);                        let mr = l(x+1,y);
    let bl = l(x-1,y+1); let bc = l(x,y+1); let br = l(x+1,y+1);

    let gx = -tl + tr - 2*ml + 2*mr - bl + br;
    let gy = -tl - 2*tc - tr + bl + 2*bc + br;
    let magnitude = (gx.abs() + gy.abs()) as f32 / 1020.0;

    if magnitude < threshold { return None; }

    let abs_gx = gx.abs();
    let abs_gy = gy.abs();
    Some(if abs_gx > abs_gy * 2 { EDGE_V }
         else if abs_gy > abs_gx * 2 { EDGE_H }
         else if (gx > 0 && gy > 0) || (gx < 0 && gy < 0) { EDGE_D2 }
         else { EDGE_D1 })
}
```

---

## 49. SHAPE MATCHER — af-ascii/src/shape_match.rs (complet)

```rust
use af_core::frame::FrameBuffer;

pub struct ShapeMatcher {
    glyphs: Vec<(char, u32)>,
}

impl ShapeMatcher {
    pub fn new(charset: &str) -> Self {
        Self { glyphs: charset.chars().map(|ch| (ch, get_bitmap(ch))).collect() }
    }

    pub fn match_cell(&self, frame: &FrameBuffer, bx: u32, by: u32) -> char {
        let mut lumas = [0u8; 25];
        let mut count = 0usize;
        for dy in 0..5u32 {
            for dx in 0..5u32 {
                let (x, y) = (bx + dx, by + dy);
                if x < frame.width && y < frame.height {
                    lumas[count] = frame.luminance(x, y);
                    count += 1;
                }
            }
        }
        let median = if count > 0 { lumas[..count].sort_unstable(); lumas[count / 2] } else { 128 };

        let mut bitmap: u32 = 0;
        for dy in 0..5u32 {
            for dx in 0..5u32 {
                let (x, y) = (bx + dx, by + dy);
                if x < frame.width && y < frame.height && frame.luminance(x, y) >= median {
                    bitmap |= 1 << (dy * 5 + dx);
                }
            }
        }

        let mut best = (' ', u32::MAX);
        for &(ch, glyph) in &self.glyphs {
            let dist = (bitmap ^ glyph).count_ones();
            if dist < best.1 { best = (ch, dist); }
        }
        best.0
    }
}

fn get_bitmap(ch: char) -> u32 {
    match ch {
        ' '  => 0b00000_00000_00000_00000_00000,
        '.'  => 0b00000_00000_00000_00000_00100,
        ':'  => 0b00000_00100_00000_00100_00000,
        '-'  => 0b00000_00000_11111_00000_00000,
        '='  => 0b00000_11111_00000_11111_00000,
        '+'  => 0b00000_00100_01110_00100_00000,
        '*'  => 0b00000_10101_01110_10101_00000,
        '#'  => 0b01010_11111_01010_11111_01010,
        '%'  => 0b11001_11010_00100_01011_10011,
        '@'  => 0b01110_10001_10111_10000_01111,
        '/'  => 0b00001_00010_00100_01000_10000,
        '\\' => 0b10000_01000_00100_00010_00001,
        '|'  => 0b00100_00100_00100_00100_00100,
        '_'  => 0b00000_00000_00000_00000_11111,
        '!'  => 0b00100_00100_00100_00000_00100,
        '0'  => 0b01110_10011_10101_11001_01110,
        'O'  => 0b01110_10001_10001_10001_01110,
        'M'  => 0b10001_11011_10101_10001_10001,
        'W'  => 0b10001_10001_10101_11011_10001,
        '█'  => 0b11111_11111_11111_11111_11111,
        '░'  => 0b10100_01010_10100_01010_10100,
        '▒'  => 0b10101_01010_10101_01010_10101,
        '▓'  => 0b01011_10101_01011_10101_01011,
        _ => estimate_density(ch),
    }
}

fn estimate_density(ch: char) -> u32 {
    let density = match ch {
        'a'..='z' => 12, 'A'..='Z' => 14, '0'..='9' => 13, _ => 8,
    };
    // Centre-out fill pattern
    let order: [u32; 25] = [12,7,2,8,14, 6,1,0,3,9, 11,5,4,10,16, 13,17,18,19,23, 20,21,22,24,15];
    let mut bm = 0u32;
    for &bit in order.iter().take(density) { bm |= 1 << bit; }
    bm
}
```

---

## 50. LUMINANCE LUT — af-core/src/charset.rs (complet)

```rust
pub const CHARSET_COMPACT: &str = " .:-=+*#%@";
pub const CHARSET_STANDARD: &str = " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
pub const CHARSET_FULL: &str = "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ";
pub const CHARSET_BLOCKS: &str = " ░▒▓█";
pub const CHARSET_MINIMAL: &str = " .:░▒▓█";

pub struct LuminanceLut { lut: [char; 256] }

impl LuminanceLut {
    /// # Example
    /// ```
    /// use af_core::charset::LuminanceLut;
    /// let lut = LuminanceLut::new(" .:#@");
    /// assert_eq!(lut.map(0), ' ');
    /// assert_eq!(lut.map(255), '@');
    /// ```
    pub fn new(charset: &str) -> Self {
        let chars: Vec<char> = charset.chars().collect();
        let len = chars.len();
        assert!(len >= 2, "Charset doit avoir >= 2 caractères");
        let mut lut = [' '; 256];
        for (i, slot) in lut.iter_mut().enumerate() {
            *slot = chars[i * (len - 1) / 255];
        }
        Self { lut }
    }

    #[inline(always)]
    pub fn map(&self, luminance: u8) -> char { self.lut[luminance as usize] }
}
```

---

## 51. CHECKLIST COMPLÈTE FINALE — TOUS LES FICHIERS PAR PHASE

### Phase 1 — Squelette (~22 fichiers)

```
asciiforge/Cargo.toml                           § 3.2
asciiforge/SPEC.md                               ce document
asciiforge/config/default.toml                   § 8
asciiforge/crates/af-core/Cargo.toml             § 40
asciiforge/crates/af-core/src/lib.rs             § 41
asciiforge/crates/af-core/src/config.rs          § 5.4 + § 34
asciiforge/crates/af-core/src/frame.rs           § 5.1 + § 5.2 + § 5.3
asciiforge/crates/af-core/src/charset.rs         § 50
asciiforge/crates/af-core/src/color.rs           § 30
asciiforge/crates/af-core/src/error.rs           § 17.1
asciiforge/crates/af-core/src/traits.rs          § 4
asciiforge/crates/af-render/Cargo.toml           § 40
asciiforge/crates/af-render/src/lib.rs           § 41
asciiforge/crates/af-render/src/canvas.rs        § 6.3.1
asciiforge/crates/af-render/src/ui.rs            § 6.3.2
asciiforge/crates/af-render/src/widgets.rs       stub
asciiforge/crates/af-render/src/fps.rs           § 33
asciiforge/crates/af-render/src/effects.rs       stub
asciiforge/crates/af-app/Cargo.toml              § 40
asciiforge/crates/af-app/src/main.rs             § 19
asciiforge/crates/af-app/src/app.rs              § 18 + § 29
asciiforge/crates/af-app/src/cli.rs              § 16
asciiforge/crates/af-app/src/pipeline.rs         stub
asciiforge/crates/af-app/src/hotreload.rs        § 20
```

### Phase 2 — Image → ASCII (+9 fichiers)

```
asciiforge/crates/af-source/Cargo.toml           § 40
asciiforge/crates/af-source/src/lib.rs           § 41
asciiforge/crates/af-source/src/image.rs         § 36
asciiforge/crates/af-source/src/resize.rs        § 35
asciiforge/crates/af-ascii/Cargo.toml            § 40
asciiforge/crates/af-ascii/src/lib.rs            § 41
asciiforge/crates/af-ascii/src/luminance.rs      fn process utilisant LuminanceLut
asciiforge/crates/af-ascii/src/color_map.rs      § 47
asciiforge/crates/af-ascii/src/compositor.rs     § 38
```

### Phase 3 — Modes avancés (+5 fichiers)

```
asciiforge/crates/af-ascii/src/halfblock.rs      § 37
asciiforge/crates/af-ascii/src/braille.rs        § 24
asciiforge/crates/af-ascii/src/quadrant.rs       § 25
asciiforge/crates/af-ascii/src/edge.rs           § 48
asciiforge/crates/af-ascii/src/shape_match.rs    § 49
```

### Phase 4 — Audio (+9 fichiers)

```
asciiforge/crates/af-audio/Cargo.toml            § 40
asciiforge/crates/af-audio/src/lib.rs            § 41
asciiforge/crates/af-audio/src/capture.rs        § 45
asciiforge/crates/af-audio/src/fft.rs            § 6.1.3
asciiforge/crates/af-audio/src/features.rs       § 6.1.4
asciiforge/crates/af-audio/src/beat.rs           § 6.1.4
asciiforge/crates/af-audio/src/smoothing.rs      § 46
asciiforge/crates/af-audio/src/state.rs          § 39
asciiforge/crates/af-audio/src/error.rs          § 17.2
```

### Phase 5 — Audio-réactivité (modification)

```
asciiforge/crates/af-app/src/pipeline.rs         § 44
```

### Phase 6 — Décodage fichier (+1)

```
asciiforge/crates/af-audio/src/decode.rs         § 6.1.2
```

### Phase 7 — Vidéo (+1)

```
asciiforge/crates/af-source/src/video.rs
```

### Phase 8 — Webcam + Procédural (+2)

```
asciiforge/crates/af-source/src/webcam.rs
asciiforge/crates/af-source/src/procedural.rs    § 26
```

### Phase 9 — MIDI + Polish (+8)

```
asciiforge/crates/af-app/src/midi.rs             § 42
asciiforge/config/presets/ambient.toml            § 43
asciiforge/config/presets/aggressive.toml         § 43
asciiforge/config/presets/minimal.toml            § 43
asciiforge/config/presets/retro.toml              § 43
asciiforge/config/presets/psychedelic.toml        § 43
asciiforge/README.md
asciiforge/tests/pipeline_test.rs                § 27.2
asciiforge/benches/ascii_bench.rs                § 27.3
```

**Total : ~56 fichiers source, 9 phases.**

---

*Ce document est vivant. Toute modification doit être justifiée, datée, et cohérente avec les règles R1-R10.
Dernière mise à jour : 2026-02-26. Version : 1.0.0.*
