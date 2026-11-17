#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod fs;
mod hardware;
mod interrupciones;
mod memoria;
mod tareas;
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
    tareas::inicializar_tareas();

    fs::crear_archivo("readme.txt", "Bienvenido a melac_os v0.9! Escribe 'htop' para ver procesos.");
    fs::crear_archivo(
        "demo.ms",
        "color cian\nprint === Ejecutando programa en melac_os ===\ncolor amarillo\nprint Hola desde MelacScript!\nbeep\ncolor verde",
    );

    {
        let mut pantalla = PANTALLA_GLOBAL.lock();
        pantalla.limpiar_pantalla();
        pantalla.escribir_texto("========================================\n");
        pantalla.escribir_texto("       BIENVENIDO A MELAC_OS v0.9       \n");
        pantalla.escribir_texto("========================================\n\n");
        pantalla.escribir_texto("[OK] Planificador Multitarea y Barra en vivo activos.\n");
        pantalla.escribir_texto("[OK] Ejecuta 'htop' para ver el consumo de CPU y RAM.\n\n");
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
