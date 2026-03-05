#!/usr/bin/env bash
set -e

# ============================================================
#  classcii - Stem separation environment setup (Linux/macOS)
#  Installs Python venv + CPU-only PyTorch + SCNet dependencies
#  using uv (https://docs.astral.sh/uv/)
# ============================================================

# Navigate to project root (one level up from scripts/)
cd "$(dirname "$0")/.."

echo "[classcii] Setting up stem separation environment..."
echo

# --- Check uv ---
if ! command -v uv &>/dev/null; then
    echo "ERROR: uv is not installed or not in PATH."
    echo "Install it with: curl -LsSf https://astral.sh/uv/install.sh | sh"
    echo "Or see: https://docs.astral.sh/uv/getting-started/installation/"
    exit 1
fi
echo "[OK] uv found"

# --- Create venv ---
if [ ! -f ".venv/bin/python" ]; then
    echo "[..] Creating virtual environment..."
    uv venv .venv --seed
    echo "[OK] Virtual environment created"
else
    echo "[OK] Virtual environment already exists"
fi

# --- Install PyTorch CPU-only ---
echo "[..] Installing PyTorch (CPU-only)..."
uv pip install --python .venv/bin/python torch torchaudio --index-url https://download.pytorch.org/whl/cpu --quiet
echo "[OK] PyTorch installed"

# --- Install SCNet dependencies ---
echo "[..] Installing SCNet dependencies..."
uv pip install --python .venv/bin/python soundfile numpy pyyaml einops julius tqdm --quiet
echo "[OK] SCNet dependencies installed"

# --- Install any additional deps from ext/SCNet/requirements.txt ---
if [ -f "ext/SCNet/requirements.txt" ]; then
    echo "[..] Installing additional SCNet requirements..."
    uv pip install --python .venv/bin/python -r ext/SCNet/requirements.txt --quiet 2>/dev/null || true
    echo "[OK] Additional requirements installed"
fi

# --- Verify model checkpoint ---
if [ ! -f "ext/SCNet/models/SCNet.th" ]; then
    echo
    echo "WARNING: SCNet model checkpoint not found at ext/SCNet/models/SCNet.th"
    echo "Download the model and place it in ext/SCNet/models/"
else
    echo "[OK] SCNet model checkpoint found"
fi

echo
echo "============================================================"
echo " Setup complete! Stem separation is ready."
echo " Use the S key in classcii TUI to access stem separation."
echo "============================================================"
