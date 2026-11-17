use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

pub static RATON_X: AtomicI32 = AtomicI32::new(400);
pub static RATON_Y: AtomicI32 = AtomicI32::new(300);
pub static CLIC_IZQUIERDO: AtomicBool = AtomicBool::new(false);

struct EstadoPaqueteRaton {
    ciclo: u8,
    bytes: [u8; 3],
}

static PAQUETE_RATON: Mutex<EstadoPaqueteRaton> = Mutex::new(EstadoPaqueteRaton {
    ciclo: 0,
    bytes: [0; 3],
});

unsafe fn esperar_escritura_8042() {
    let mut estado: Port<u8> = Port::new(0x64);
    for _ in 0..100_000 {
        if (estado.read() & 0x02) == 0 {
            return;
        }
    }
}

unsafe fn esperar_lectura_8042() {
    let mut estado: Port<u8> = Port::new(0x64);
    for _ in 0..100_000 {
        if (estado.read() & 0x01) != 0 {
            return;
        }
    }
}

unsafe fn enviar_comando_raton(cmd: u8) {
    let mut puerto_cmd: Port<u8> = Port::new(0x64);
    let mut puerto_datos: Port<u8> = Port::new(0x60);

    esperar_escritura_8042();
    puerto_cmd.write(0xD4);
    esperar_escritura_8042();
    puerto_datos.write(cmd);

    esperar_lectura_8042();
    let _ = puerto_datos.read();
}

pub fn inicializar_raton() {
    unsafe {
        let mut puerto_cmd: Port<u8> = Port::new(0x64);
        let mut puerto_datos: Port<u8> = Port::new(0x60);

        esperar_escritura_8042();
        puerto_cmd.write(0xA8);

        esperar_escritura_8042();
        puerto_cmd.write(0x20);
        esperar_lectura_8042();
        let mut config = puerto_datos.read();

        config |= 0x02;
        config &= !0x20;

        esperar_escritura_8042();
        puerto_cmd.write(0x60);
        esperar_escritura_8042();
        puerto_datos.write(config);

        enviar_comando_raton(0xF6);
        enviar_comando_raton(0xF4);
    }
}

pub fn procesar_byte_raton(dato: u8) -> Option<(usize, usize, bool)> {
    let mut pkt = PAQUETE_RATON.lock();

    match pkt.ciclo {
        0 => {
            if (dato & 0x08) != 0 {
                pkt.bytes.copy_from_slice(&[dato, 0, 0]);
                pkt.ciclo = 1;
            }
            None
        }
        1 => {
            pkt.bytes.as_mut()[1] = dato;
            pkt.ciclo = 2;
            None
        }
        _ => {
            pkt.bytes.as_mut()[2] = dato;
            pkt.ciclo = 0;

            let flags = *pkt.bytes.first().unwrap();
            if (flags & 0x40) != 0 || (flags & 0x80) != 0 {
                return None;
            }

            let mut dx = *pkt.bytes.get(1).unwrap() as i32;
            let mut dy = *pkt.bytes.get(2).unwrap() as i32;

            if (flags & 0x10) != 0 {
                dx |= !0xFF;
            }
            if (flags & 0x20) != 0 {
                dy |= !0xFF;
            }

            let nuevo_x = (RATON_X.load(Ordering::Relaxed) + dx).clamp(0, 788);
            let nuevo_y = (RATON_Y.load(Ordering::Relaxed) - dy).clamp(0, 588);

            RATON_X.store(nuevo_x, Ordering::Relaxed);
            RATON_Y.store(nuevo_y, Ordering::Relaxed);

            let clic_izq = (flags & 0x01) != 0;
            CLIC_IZQUIERDO.store(clic_izq, Ordering::Relaxed);

            Some((nuevo_x as usize, nuevo_y as usize, clic_izq))
        }
    }
}
