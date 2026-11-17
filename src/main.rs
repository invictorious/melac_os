#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)] // Activamos el soporte de interrupciones de procesador x86

mod interrupciones;
mod vga_buffer;

use bootloader as _;
use core::panic::PanicInfo;
use vga_buffer::PANTALLA_GLOBAL;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 1. Apagamos interrupciones para que nadie más interrumpa
    x86_64::instructions::interrupts::disable();

    // 2. Como estamos en emergencia, rompemos cualquier candado bloqueado de la pantalla
    unsafe {
        PANTALLA_GLOBAL.force_unlock();
    }

    let mut pantalla = PANTALLA_GLOBAL.lock();
    pantalla.cambiar_color(vga_buffer::Color::Blanco, vga_buffer::Color::Rojo);
    pantalla.escribir_texto("\n[!!!] KERNEL PANIC EN MELAC_OS [!!!]\n");
    
    // Si el panic trae un mensaje de texto, también lo mostramos
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
    {
        let mut pantalla = PANTALLA_GLOBAL.lock();
        pantalla.limpiar_pantalla();
        pantalla.escribir_texto("========================================\n");
        pantalla.escribir_texto("       BIENVENIDO A MELAC_OS v0.2       \n");
        pantalla.escribir_texto("========================================\n\n");
        pantalla.escribir_texto("[OK] Tabla de Interrupciones (IDT) activa.\n");
        pantalla.escribir_texto("[OK] Driver de Teclado PS/2 listo.\n\n");
        pantalla.escribir_texto("melac_os> ");
    }

    // Inicializamos la IDT y encendemos los chips PIC
    interrupciones::inicializar_idt();
    unsafe {
        interrupciones::PICS.lock().initialize();
    }
    // ¡Ordenamos a la CPU empezar a escuchar interrupciones físicas!
    x86_64::instructions::interrupts::enable();

    // Bucle de bajo consumo: la CPU duerme hasta que pulses una tecla
    loop {
        x86_64::instructions::hlt();
    }
}

