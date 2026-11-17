use core::sync::atomic::{AtomicU64, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::tareas::tic_planificador;
use crate::vga_buffer::PANTALLA_GLOBAL;

pub static TICS_RELOJ: AtomicU64 = AtomicU64::new(0);

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

static TECLADO: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
    Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore)
);

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[allow(static_mut_refs)]
pub fn inicializar_idt() {
    unsafe {
        IDT[32].set_handler_fn(manejador_reloj);
        IDT[33].set_handler_fn(manejador_teclado);
        IDT.load();
    }
}

extern "x86-interrupt" fn manejador_reloj(_stack_frame: InterruptStackFrame) {
    let tics = TICS_RELOJ.fetch_add(1, Ordering::Relaxed) + 1;

    tic_planificador(tics);

    if tics % 18 == 0 {
        if let Some(mut pantalla) = PANTALLA_GLOBAL.try_lock() {
            pantalla.dibujar_barra_estado(tics);
        }
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(32);
    }
}

extern "x86-interrupt" fn manejador_teclado(_stack_frame: InterruptStackFrame) {
    let mut puerto_teclado = Port::new(0x60);
    let mut teclado = TECLADO.lock();

    let scancode: u8 = unsafe { puerto_teclado.read() };

    if let Ok(Some(evento_tecla)) = teclado.add_byte(scancode) {
        if let Some(tecla) = teclado.process_keyevent(evento_tecla) {
            match tecla {
                DecodedKey::Unicode(caracter) => {
                    PANTALLA_GLOBAL.lock().procesar_tecla(caracter as u8);
                }
                DecodedKey::RawKey(_tecla_especial) => {}
            }
        }
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(33);
    }
}
