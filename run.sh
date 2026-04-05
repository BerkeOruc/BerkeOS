#!/usr/bin/env bash
# BerkeOS — QEMU Launch Script

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

ISO="build/berkeos.iso"

NOGRAPHIC=false
UEFI_MODE=false

for arg in "$@"; do
    case $arg in
        -n|--nographic|--headless|-h)
            NOGRAPHIC=true
            ;;
        --uefi|-uefi)
            UEFI_MODE=true
            ;;
    esac
done

if [ -f "/usr/share/qemu/ovmf-x86_64.bin" ] || [ -f "/usr/share/edk2/x64/OVMF.fd" ]; then
    if [ "$UEFI_MODE" = false ]; then
        UEFI_AUTO="auto"
    else
        UEFI_AUTO="uefi"
    fi
else
    UEFI_AUTO="bios"
fi

[ -f "$ISO" ] || {
    echo -e "${RED}ERROR:${NC} $ISO not found. Run ${YELLOW}./build.sh${NC} first."
    exit 1
}

command -v qemu-system-x86_64 &>/dev/null || {
    echo -e "${RED}ERROR:${NC} qemu-system-x86_64 not found."
    echo "Install: sudo pacman -S qemu-full"
    exit 1
}

if [ "$NOGRAPHIC" = false ]; then
    echo ""
    echo -e "${GREEN}${BOLD}==> BerkeOS — Launching in QEMU${NC}"
    echo -e "    ISO      : ${CYAN}$ISO${NC}"
    echo -e "    Arch     : x86_64  |  RAM: 256 MiB  |  Boot: ${CYAN}$UEFI_AUTO${NC}"
    echo -e "    Display  : ${CYAN}1024x768 32bpp pixel framebuffer${NC}"
    echo -e "    Drives   : ${CYAN}Alpha (ide0) | Beta (ide1)${NC}"
    echo -e "    Input    : ${CYAN}PS/2 Keyboard — click QEMU window to type${NC}"
    echo ""
    echo -e "    ${YELLOW}Click the QEMU window to capture keyboard input${NC}"
    echo -e "    ${YELLOW}Press Ctrl+Alt+G to release mouse from QEMU${NC}"
    echo ""
fi

DISK1="build/berkeos_disk.img"
DISK2="build/berkeos_disk2.img"

if [ ! -f "$DISK1" ]; then
    [ "$NOGRAPHIC" = false ] && echo -e "  ${CYAN}->  Alpha disk olusturuluyor...${NC}"
    dd if=/dev/zero of="$DISK1" bs=1M count=128 2>/dev/null
fi

if [ ! -f "$DISK2" ]; then
    [ "$NOGRAPHIC" = false ] && echo -e "  ${CYAN}->  Beta disk olusturuluyor...${NC}"
    dd if=/dev/zero of="$DISK2" bs=1M count=256 2>/dev/null
fi

UEFI_BIOS=""
UEFI_FORCE=""
if [ -f "/usr/share/qemu/ovmf-x86_64.bin" ]; then
    UEFI_BIOS="-bios /usr/share/qemu/ovmf-x86_64.bin"
    if [ "$UEFI_MODE" = true ]; then
        UEFI_FORCE="-bios /usr/share/qemu/ovmf-x86_64.bin"
    fi
elif [ -f "/usr/share/edk2/x64/OVMF.fd" ]; then
    UEFI_BIOS="-bios /usr/share/edk2/x64/OVMF.fd"
    if [ "$UEFI_MODE" = true ]; then
        UEFI_FORCE="-bios /usr/share/edk2/x64/OVMF.fd"
    fi
fi

BOOT_OPTS="-boot d"
if [ "$UEFI_AUTO" = "bios" ]; then
    BOOT_OPTS="-boot d"
else
    BOOT_OPTS="-boot order=c,menu=off"
fi

if [ "$NOGRAPHIC" = true ]; then
    qemu-system-x86_64 \
        -m            256M           \
        -cdrom        "$ISO"         \
        -drive        file="$DISK1",format=raw,if=ide,index=0,media=disk \
        -drive        file="$DISK2",format=raw,if=ide,index=1,media=disk \
        $BOOT_OPTS   \
        -nographic                  \
        -serial       null          \
        $UEFI_FORCE                \
        -D            build/qemu.log \
        "$@"
else
    qemu-system-x86_64 \
        -m            256M           \
        -cdrom        "$ISO"         \
        -drive        file="$DISK1",format=raw,if=ide,index=0,media=disk \
        -drive        file="$DISK2",format=raw,if=ide,index=1,media=disk \
        $BOOT_OPTS   \
        -vga          std            \
        -serial       stdio           \
        $UEFI_FORCE                \
        -D            build/qemu.log \
        "$@"
fi

if [ "$NOGRAPHIC" = false ]; then
    echo ""
    echo -e "${GREEN}==> QEMU exited.${NC}"
    echo -e "    Log: ${CYAN}build/qemu.log${NC}"
fi
