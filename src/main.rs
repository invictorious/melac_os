#![no_std]
#![no_main]

mod vga_buffer; // Importamos el driver que acabamos de crear

use bootloader as _;
use core::panic::PanicInfo;
use vga_buffer::{Color, EscritorVGA};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Creamos nuestro escritor con texto CianClaro sobre fondo Negro
    let mut pantalla = EscritorVGA::nuevo(Color::CianClaro, Color::Negro);
    
    pantalla.limpiar_pantalla();
    pantalla.escribir_texto("========================================\n");
    pantalla.escribir_texto("       BIENVENIDO A MELAC_OS v0.1       \n");
    pantalla.escribir_texto("========================================\n\n");
    pantalla.escribir_texto("[OK] Kernel de 64 bits cargado.\n");
    pantalla.escribir_texto("[OK] Driver de pantalla VGA inicializado.\n");

    loop {}
}
