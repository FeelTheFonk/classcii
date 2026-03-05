@echo off
setlocal

REM ============================================================
REM  classcii - Stem separation environment setup (Windows)
REM  Installs Python venv + CPU-only PyTorch + SCNet dependencies
REM  using uv (https://docs.astral.sh/uv/)
REM ============================================================

REM Navigate to project root (one level up from scripts/)
cd /d "%~dp0.."

echo [classcii] Setting up stem separation environment...
echo.

REM --- Check uv ---
where uv >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: uv is not installed or not in PATH.
    echo Install it with: powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
    echo Or see: https://docs.astral.sh/uv/getting-started/installation/
    exit /b 1
)
echo [OK] uv found

REM --- Create venv ---
if not exist ".venv\Scripts\python.exe" (
    echo [..] Creating virtual environment...
    uv venv .venv --seed
    if %errorlevel% neq 0 (
        echo ERROR: Failed to create virtual environment.
        exit /b 1
    )
    echo [OK] Virtual environment created
) else (
    echo [OK] Virtual environment already exists
)

REM --- Install PyTorch CPU-only ---
echo [..] Installing PyTorch (CPU-only)...
uv pip install --python .venv\Scripts\python.exe torch torchaudio --index-url https://download.pytorch.org/whl/cpu --quiet
if %errorlevel% neq 0 (
    echo ERROR: Failed to install PyTorch.
    exit /b 1
)
echo [OK] PyTorch installed

REM --- Install SCNet dependencies from requirements.txt ---
echo [..] Installing SCNet dependencies...
uv pip install --python .venv\Scripts\python.exe soundfile numpy pyyaml einops julius tqdm --quiet
if %errorlevel% neq 0 (
    echo ERROR: Failed to install SCNet dependencies.
    exit /b 1
)
echo [OK] SCNet dependencies installed

REM --- Install any additional deps from ext/SCNet/requirements.txt ---
if exist "ext\SCNet\requirements.txt" (
    echo [..] Installing additional SCNet requirements...
    uv pip install --python .venv\Scripts\python.exe -r ext\SCNet\requirements.txt --quiet 2>nul
    echo [OK] Additional requirements installed
)

REM --- Verify model checkpoint ---
if not exist "ext\SCNet\models\SCNet.th" (
    echo.
    echo WARNING: SCNet model checkpoint not found at ext\SCNet\models\SCNet.th
    echo Download the model and place it in ext\SCNet\models\
) else (
    echo [OK] SCNet model checkpoint found
)

echo.
echo ============================================================
echo  Setup complete! Stem separation is ready.
echo  Use the S key in classcii TUI to access stem separation.
echo ============================================================

endlocal
