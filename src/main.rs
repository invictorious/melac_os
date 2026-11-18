#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod disco;
mod fs;
mod graficos;
mod hardware;
mod interrupciones;
mod memoria;
mod raton;
mod rtc;
mod shell;
mod tareas;
mod vga_buffer;

use alloc::string::String;
use bootloader::{entry_point, BootInfo};
use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU64, Ordering};
use vga_buffer::PANTALLA_GLOBAL;

pub static OFFSET_MEMORIA_FISICA: AtomicU64 = AtomicU64::new(0);

entry_point!(kernel_main);

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

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    OFFSET_MEMORIA_FISICA.store(boot_info.physical_memory_offset, Ordering::Relaxed);

    memoria::inicializar_heap();
    tareas::inicializar_tareas();
    fs::inicializar_ruta_base();
    disco::inicializar_bus_ata();
    raton::inicializar_raton();

    let estado_montaje = disco::montar_disco_instalado();

    {
        let mut pantalla = PANTALLA_GLOBAL.lock();
        pantalla.activar_cursor_hardware();
        pantalla.limpiar_desde_fila(1);
        pantalla.escribir_texto("==================================================\n");
        pantalla.escribir_texto("       BIENVENIDO A MELAC_OS SERVER v3.1          \n");
        pantalla.escribir_texto("==================================================\n\n");

        match estado_montaje {
            Ok(n) => {
                *fs::SISTEMA_INSTALADO.lock() = true;
                *fs::DIRECTORIO_ACTUAL.lock() = String::from("/home/usuario");
                pantalla.cambiar_color(vga_buffer::Color::CianClaro, vga_buffer::Color::Negro);
                let _ = writeln!(pantalla, "[OK] Disco duro montado en '/' ({} nodos activos).", n);
                pantalla.escribir_texto("[OK] Documentacion completa en ~/manual_comandos.txt\n\n");
            }
            Err(_) => {
                pantalla.cambiar_color(vga_buffer::Color::Amarillo, vga_buffer::Color::Negro);
                pantalla.escribir_texto("[MODO LIVE-CD] Ejecuta 'install' para instalar melac_os Server.\n\n");
            }
        }
    }
    shell::imprimir_prompt();
    shell::actualizar_barra_superior(0);

    interrupciones::inicializar_idt();
    unsafe {
        interrupciones::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::hlt();
    }
}
