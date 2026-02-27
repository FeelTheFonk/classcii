# clasSCII Engine

Real-time, audio-reactive ASCII/Unicode rendering engine for terminal-based Text User Interface (TUI) applications. Engineered for robust performance and deterministic execution in constraint-heavy CLI environments.

## 1. System Architecture & Concurrency Model

clasSCII implements a lock-free, zero-allocation (in hot paths) three-thread topology to guarantee stable frame rates and prevent acoustic or visual stuttering under high DSP load.

```text
┌────────────────────────┐      ┌────────────────────────┐      ┌────────────────────────┐
│     Source Thread      │      │      Main Thread       │      │      Audio Thread      │
│     (I/O, Decode)      │───▶  │  (Render, TUI, State)  │  ◀───│   (DSP, FFT, Capture)  │
│  [Asynchronous Stream] │      │  [Lock-Free Pipeline]  │      │  [Real-Time Priority]  │
└────────────────────────┘      └────────────────────────┘      └────────────────────────┘
```

- **Inter-Process Communication (IPC):** Lock-free memory synchronization is achieved via `triple_buffer` for frame payloads and `flume` channels for event signaling. This structure eliminates mutex contention and false sharing across CPU cache lines.
- **Memory Coherence:** The engine strictly enforces zero dynamic allocation (`Vec` creation, etc.) during the active render loop. Buffers are pre-allocated at initialization, and mutating state is handled in-place.

## 2. Workspace Topology & Internal Specifications

The codebase is partitioned into 6 distinct crates to enforce strict boundary isolation and separation of concerns.

| Crate | Abstraction Layer | Core Responsibilities |
|-------|-------------------|-----------------------|
| `af-core` | Shared primitives | Traits, unified configuration matrix, core frame buffers. |
| `af-audio` | Digital Signal Processing | Audio capture, Fast Fourier Transform (FFT) analysis, beat detection, continuous feature extraction. |
| `af-ascii` | Visual Algorithms | Luma-to-ASCII mapping, edge detection convolution, Braille/Halfblock/Quadrant quantization, spatial shape matching. |
| `af-render` | Display & TUI | Terminal rendering backend (via `ratatui`), partial redraws, UI overlays, effect pipelines, hardware FPS logic. |
| `af-source` | Input Pipeline | Visual input stream decoders: image payloads, video streams, webcam capture, procedural generation. |
| `af-app` | Orchestration | Application entry point, event loop management, thread lifecycle, pipeline orchestration. |

## 3. Algorithmic Complexity & DSP

- **Audio Processing:** Operates a continuous FFT analysis loop. Windowing functions and localized smoothing filters extract stable crest frequencies and percussive transients for real-time reactivity.
- **Pixel Mapping:** The visual translation algorithm extends beyond scalar luminance mapping. It implements kernel-based edge detection and high-density Unicode spatial quantization (Braille, Quadrants) to maximize structural fidelity within a discrete terminal matrix.
- **Render Output:** Terminal bandwidth is strictly gated. By utilizing `ratatui`'s buffer diffing algorithms, I/O bound `stdout` operations are minimized via partial frame redraws.

## 4. Deployment, Build & Compilation Profiles

The project is designed to leverage maximum compiler optimization (LTO, codegen-units) in deployment contexts.

```bash
# Development Profile (Unoptimized, fast iteration)
cargo build --workspace

# Production Release Profile (Maximized optimization, LTO enabled, Video support)
cargo build --release --features video
```

*Note: For optimal runtime performance, deployment environments must utilize hardware-accelerated terminal emulators (e.g., Alacritty, Kitty).*

## 5. Usage & Pipeline Instantiation

```bash
# Static Image Payload
classcii --image path/to/image.png

# Image Payload with Live Microphone Reactivity
classcii --image path/to/image.png --audio mic

# Video Stream with Embedded Audio Track
classcii --video path/to/video.mp4

# Algorithmic Override (Strict Render Mode & Target Frame Rate)
classcii --image photo.jpg --mode braille --fps 60

# Preset Configuration Instantiation
classcii --image photo.jpg --preset psychedelic
```

## 6. Real-Time Telemetry & Controls

| Keybind | Assigned Action |
|---------|-----------------|
| `Tab` | Cycle render topology (Ascii → HalfBlock → Braille → Quadrant) |
| `1`–`5` | Select target character set matrix |
| `c` | Toggle chromatic mode (Color) |
| `i` | Invert luminance output scale |
| `e` | Toggle convolutional edge detection |
| `s` | Toggle spatial shape matching algorithm |
| `m` | Cycle color mapping strategy |
| `b` | Cycle background style buffer |
| `d` / `D` | Render density scalar (±0.25) |
| `[` / `]` | Viewport contrast adjust (±0.1) |
| `{` / `}` | Viewport brightness adjust (±0.05) |
| `-` / `+` | Global saturation adjust (±0.1) |
| `f` / `F` | Temporal fade decay factor (±0.1) |
| `g` / `G` | Post-process glow intensity (±0.1) |
| `↑` / `↓` | Audio DSP reactivity sensitivity (±0.1) |
| `←` / `→` | Stream explicit seek (±5s video/audio) |
| `Space` | Pause/Resume core engine pipeline |
| `?` | Toggle telemetry & diagnostic overlay |
| `q` / `Esc` | Terminate application process gracefully |

## 7. Configuration & State Management

System boundaries and initial states are defined via strongly-typed TOML matrices. Dynamic hot-reloading native support ensures thread persistence during state changes.

**Default state boundary:** `config/default.toml`
**Presets directory:** `config/presets/`

```bash
# Load explicit configuration matrix path
classcii --image photo.jpg --config my_config.toml

# Execute via predefined visual preset
classcii --image photo.jpg --preset ambient
```

## 8. Quality Assurance & Structural Safety

- **Panic-Free Runtime Guarantee:** The invocation of `unwrap()` or `expect()` is strictly banned outside of explicit testing modules. All runtime exceptions are propagated, typed, and handled deterministically.
- **Static Analysis Compliance:** The integration pipeline enforces zero-tolerance `clippy` analysis.
  `cargo clippy --workspace -- -D warnings` must evaluate to 0 warnings.
- **Memory Safety Boundaries:** Any FFI linkage or raw pointer manipulation (e.g., C-bindings for video codecs or atomic lock-free structures) is strictly isolated within defined scopes.

## 9. License

MIT License.
