#!/bin/bash
set -e
cargo bootimage
IMAGEN_LIVE="target/x86_64-melac_kernel/debug/bootimage-melac_kernel.bin"
DISPLAY=:0 qemu-system-x86_64 \
    -drive format=raw,file="$IMAGEN_LIVE",index=0,media=disk \
    -drive format=raw,file=disco_instalado.img,index=1,media=disk
