use core::ops::IndexMut;
use core::sync::atomic::{AtomicU64, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::raton::procesar_byte_raton;
use crate::shell::{actualizar_barra_superior, refrescar_monitor_vivo, SHELL_GLOBAL};
use crate::tareas::tic_planificador;

pub static TICS_RELOJ: AtomicU64 = AtomicU64::new(0);

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

static TECLADO: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
    Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::MapLettersToUnicode)
);

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[allow(static_mut_refs)]
pub fn inicializar_idt() {
    unsafe {
        IDT.index_mut(32).set_handler_fn(manejador_reloj);
        IDT.index_mut(33).set_handler_fn(manejador_teclado);
        IDT.index_mut(44).set_handler_fn(manejador_raton);
        IDT.index_mut(46).set_handler_fn(manejador_disco);
        IDT.load();
    }
}

extern "x86-interrupt" fn manejador_reloj(_stack_frame: InterruptStackFrame) {
    let tics = TICS_RELOJ.fetch_add(1, Ordering::Relaxed) + 1;
    tic_planificador(tics);

    if tics % 18 == 0 {
        actualizar_barra_superior(tics);
        refrescar_monitor_vivo(tics);
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
                    if let Some(mut sh) = SHELL_GLOBAL.try_lock() {
                        sh.procesar_tecla(caracter as u8);
                    }
                }
                DecodedKey::RawKey(tecla_especial) => {
                    if let Some(mut sh) = SHELL_GLOBAL.try_lock() {
                        sh.procesar_tecla_especial(tecla_especial);
                    }
                }
            }
        }
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(33);
    }
}

extern "x86-interrupt" fn manejador_raton(_stack_frame: InterruptStackFrame) {
    let mut puerto_datos = Port::new(0x60);
    let dato: u8 = unsafe { puerto_datos.read() };

    if let Some((x, y, clic_izq)) = procesar_byte_raton(dato) {
        if let Some(mut sh) = SHELL_GLOBAL.try_lock() {
            sh.actualizar_raton_grafico(x, y, clic_izq);
        }
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(44);
    }
}

extern "x86-interrupt" fn manejador_disco(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(46);
    }
}
