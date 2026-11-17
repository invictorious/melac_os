#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod fs;
mod interrupciones;
mod memoria;
mod vga_buffer;

use bootloader as _;
use core::panic::PanicInfo;
use vga_buffer::PANTALLA_GLOBAL;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    x86_64::instructions::interrupts::disable();
    unsafe {
        PANTALLA_GLOBAL.force_unlock();
    }

    let mut pantalla = PANTALLA_GLOBAL.lock();
    pantalla.cambiar_color(vga_buffer::Color::Blanco, vga_buffer::Color::Rojo);
    pantalla.escribir_texto("\n[!!!] KERNEL PANIC EN MELAC_OS [!!!]\n");

    if let Some(mensaje) = info.message().as_str() {
        pantalla.escribir_texto("Motivo: ");
        pantalla.escribir_texto(mensaje);
        pantalla.escribir_texto("\n");
    }

    loop {
        x86_64::instructions::hlt();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    memoria::inicializar_heap();

    // Creamos un archivo inicial en nuestro sistema de archivos en RAM
    fs::crear_archivo("readme.txt", "Bienvenido a melac_os! Este archivo vive en el Heap.");

    {
        let mut pantalla = PANTALLA_GLOBAL.lock();
        pantalla.limpiar_pantalla();
        pantalla.escribir_texto("========================================\n");
        pantalla.escribir_texto("       BIENVENIDO A MELAC_OS v0.6       \n");
        pantalla.escribir_texto("========================================\n\n");
        pantalla.escribir_texto("[OK] Heap Allocator (100 KiB) y RamFS listos.\n");
        pantalla.escribir_texto("[OK] Interrupciones (IDT) y Teclado activos.\n\n");
        pantalla.escribir_texto("melac_os> ");
    }

    interrupciones::inicializar_idt();
    unsafe {
        interrupciones::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::hlt();
    }
}
