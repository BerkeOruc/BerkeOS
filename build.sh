#!/usr/bin/env bash
# ============================================================
#  BerkeOS — Build Script
#  Custom x86_64 OS
# ============================================================
set -e

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${GREEN}==>${NC} ${BOLD}$*${NC}"; }
step() { echo -e "  ${CYAN}->${NC} $*"; }
warn() { echo -e "  ${YELLOW}!!${NC} $*"; }
err()  { echo -e "  ${RED}ERROR:${NC} $*"; exit 1; }

echo ""
echo -e "${GREEN}${BOLD}  ██████╗ ███████╗██████╗ ██╗  ██╗███████╗ ██████╗ ███████╗${NC}"
echo -e "${GREEN}${BOLD}  ██╔══██╗██╔════╝██╔══██╗██║ ██╔╝██╔════╝██╔═══██╗██╔════╝${NC}"
echo -e "${GREEN}${BOLD}  ██████╔╝█████╗  ██████╔╝█████╔╝ █████╗  ██║   ██║███████╗${NC}"
echo -e "${GREEN}${BOLD}  ██╔══██╗██╔══╝  ██╔══██╗██╔═██╗ ██╔══╝  ██║   ██║╚════██║${NC}"
echo -e "${GREEN}${BOLD}  ██████╔╝███████╗██║  ██║██║  ██╗███████╗╚██████╔╝███████║${NC}"
echo -e "${GREEN}${BOLD}  ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝${NC}"
echo ""
log "BerkeOS Build"
echo ""

# ── Dependency check ──────────────────────────────────────────────────────────
log "Checking dependencies..."
need() {
    command -v "$1" &>/dev/null || err "'$1' not found. Install: sudo pacman -S $2"
    step "$1 ... OK"
}
need rustup        "rust"
need nasm          "nasm"
need grub-mkrescue "grub xorriso"
need xorriso       "xorriso"
need ld            "binutils"

# ── Rust toolchain ────────────────────────────────────────────────────────────
step "Setting Rust toolchain to nightly..."
rustup override set nightly 2>/dev/null || true
step "Checking rust-src component..."
rustup component add rust-src --toolchain nightly 2>/dev/null || true
NIGHTLY_VER=$(rustup show active-toolchain 2>/dev/null | awk '{print $1}')
step "Active toolchain: $NIGHTLY_VER"

# ── Clean ─────────────────────────────────────────────────────────────────────
log "Cleaning stale artifacts..."
rm -rf build target
step "Cleaned."

# ── Directories ───────────────────────────────────────────────────────────────
log "Preparing build directories..."
mkdir -p build/isofiles/boot/grub
step "build/ ... ready"

# ── Step 1: Assemble boot shim ────────────────────────────────────────────────
log "Assembling boot shim (boot.asm)..."
nasm -f elf64 src/boot/boot.asm -o build/boot.o -w-all
step "boot.o ... OK"

# ── Step 2: Build Rust kernel as staticlib ────────────────────────────────────
log "Building Rust kernel (staticlib)..."
step "Target: x86_64-unknown-none (built-in bare-metal target)"
step "Cargo produces libberkeos.a — no linking by Cargo"

RUSTFLAGS="\
  -C target-feature=-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx,-avx2 \
  -C relocation-model=static \
  -C code-model=kernel \
  -C no-redzone=yes \
" \
cargo +nightly build \
    --release \
    --lib \
    --target x86_64-unknown-none \
    -Z build-std=core,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem \
    2>&1 | sed 's/^/    /'

LIB="target/x86_64-unknown-none/release/libberkeos.a"
[ -f "$LIB" ] || err "Static library not found at $LIB — check cargo output above."
step "Static library: $LIB ... OK"

# ── Step 3: Link ──────────────────────────────────────────────────────────────
log "Linking boot.o + Rust static library..."
ld \
    -n \
    --gc-sections \
    -T linker.ld \
    -o build/berkeos.bin \
    build/boot.o \
    --whole-archive "$LIB" --no-whole-archive \
    2>&1 | grep -v "RWX" || true

[ -f "build/berkeos.bin" ] || err "Link failed — berkeos.bin not produced."
step "build/berkeos.bin ... OK"
file build/berkeos.bin | grep -q "ELF" && step "ELF format: OK" || warn "Not ELF?"

# ── Step 4: GRUB config (BIOS) ─────────────────────────────────────────────
log "Writing GRUB config (Silent Boot)..."
mkdir -p build/isofiles/boot/grub
cat > build/isofiles/boot/grub/grub.cfg << 'GRUBEOF'
# BerkeOS - Silent Boot
set timeout=0
set default=0

menuentry "BerkeOS" {
    insmod all_video
    insmod gfxterm
    insmod multiboot2
    set gfxpayload=1024x768x32
    multiboot2 /boot/berkeos.bin
    boot
}
GRUBEOF
step "grub.cfg ... OK (Silent Boot)"

# ── Step 4b: EFI Boot files ───────────────────────────────────────────────
log "Setting up EFI boot..."
mkdir -p build/isofiles/efi64/EFI/BOOT

# Create EFI boot entry - point to grub
cat > build/isofiles/efi64/shell.cfg << 'EFICFG'
\EFI\BOOT\BOOTX64.EFI
EFICFG

# Copy kernel to EFI location for direct boot (as fallback)
cp build/berkeos.bin build/isofiles/boot/berkeos.bin

# Build proper EFI-enabled ISO using grub-mkrescue
# This creates a hybrid ISO that works in both BIOS and UEFI mode
grub-mkrescue -o build/berkeos.iso build/isofiles --verbose 2>&1 | head -10 || true

# Fallback: if hybrid ISO fails, try with --embedded-boot for EFI
if [ ! -f build/berkeos.iso ] || [ ! -s build/berkeos.iso ]; then
    step "Creating EFI-enabled ISO..."
    grub-mkrescue --output=build/berkeos.iso build/isofiles 2>&1 | head -5 || true
fi

[ -f "build/berkeos.iso" ] || err "ISO not created."
step "build/berkeos.iso ... OK (BIOS + UEFI)"

echo ""
echo -e "${GREEN}${BOLD}  ✓ Build complete!${NC}"
echo -e "    ISO : ${CYAN}build/berkeos.iso${NC}"
echo -e "    Run : ${YELLOW}./run.sh${NC}"
echo ""
