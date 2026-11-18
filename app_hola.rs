#![no_std]
#![no_main]
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! { loop {} }

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga = 0xb8000 as *mut u16;
    let texto = b"[OK] HOLA DESDE EL BINARIO ELF REAL!";
    for (i, &b) in texto.iter().enumerate() {
        unsafe { *vga.add((5 * 80) + i) = 0x0A00 | (b as u16) }; 
    }
    loop {}
}
