use core::fmt;
use spin::Mutex;
use x86_64::instructions::port::Port;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Negro = 0,
    Azul = 1,
    Verde = 2,
    Cian = 3,
    Rojo = 4,
    Magenta = 5,
    Marron = 6,
    GrisClaro = 7,
    GrisOscuro = 8,
    AzulClaro = 9,
    VerdeClaro = 10,
    CianClaro = 11,
    RojoClaro = 12,
    Rosa = 13,
    Amarillo = 14,
    Blanco = 15,
}

pub struct EscritorVGA {
    pub columna_actual: usize,
    pub fila_actual: usize,
    pub codigo_color: u8,
    pub buffer: *mut u8,
}

unsafe impl Send for EscritorVGA {}

pub static PANTALLA_GLOBAL: Mutex<EscritorVGA> = Mutex::new(EscritorVGA {
    columna_actual: 0,
    fila_actual: 1,
    codigo_color: ((Color::Negro as u8) << 4) | (Color::VerdeClaro as u8),
    buffer: 0xb8000 as *mut u8,
});

impl EscritorVGA {
    pub fn activar_cursor_hardware(&self) {
        unsafe {
            let mut idx: Port<u8> = Port::new(0x3D4);
            let mut dat: Port<u8> = Port::new(0x3D5);
            idx.write(0x0A);
            let inicio = dat.read() & 0xC0;
            dat.write(inicio | 13); // Cursor línea inferior gruesa (líneas 13..15)
            idx.write(0x0B);
            let fin = dat.read() & 0xE0;
            dat.write(fin | 15);
        }
    }

    pub fn mover_cursor_hardware(&self) {
        let pos = (self.fila_actual * 80 + self.columna_actual) as u16;
        unsafe {
            let mut idx: Port<u8> = Port::new(0x3D4);
            let mut dat: Port<u8> = Port::new(0x3D5);
            idx.write(0x0F);
            dat.write((pos & 0xFF) as u8);
            idx.write(0x0E);
            dat.write(((pos >> 8) & 0xFF) as u8);
        }
    }

    pub fn cambiar_color(&mut self, texto: Color, fondo: Color) {
        self.codigo_color = ((fondo as u8) << 4) | (texto as u8);
    }

    pub fn limpiar_desde_fila(&mut self, fila_inicio: usize) {
        for fila in fila_inicio..25 {
            for col in 0..80 {
                self.escribir_caracter_en(b' ', fila, col);
            }
        }
        self.fila_actual = fila_inicio;
        self.columna_actual = 0;
        self.mover_cursor_hardware();
    }

    pub fn escribir_texto(&mut self, texto: &str) {
        for byte in texto.bytes() {
            match byte {
                b'\n' => self.nueva_linea(1),
                byte => {
                    if self.columna_actual >= 80 {
                        self.nueva_linea(1);
                    }
                    self.escribir_caracter_en(byte, self.fila_actual, self.columna_actual);
                    self.columna_actual += 1;
                }
            }
        }
        self.mover_cursor_hardware();
    }

    pub fn nueva_linea(&mut self, fila_protegida: usize) {
        self.columna_actual = 0;
        if self.fila_actual < 24 {
            self.fila_actual += 1;
        } else {
            self.hacer_scroll(fila_protegida);
        }
        self.mover_cursor_hardware();
    }

    pub fn hacer_scroll(&mut self, fila_inicio: usize) {
        unsafe {
            for fila in (fila_inicio + 1)..25 {
                for col in 0..80 {
                    let origen = (fila * 80 + col) * 2;
                    let destino = ((fila - 1) * 80 + col) * 2;
                    *self.buffer.add(destino) = *self.buffer.add(origen);
                    *self.buffer.add(destino + 1) = *self.buffer.add(origen + 1);
                }
            }
            for col in 0..80 {
                self.escribir_caracter_en(b' ', 24, col);
            }
        }
    }

    pub fn escribir_caracter_en(&mut self, byte: u8, fila: usize, col: usize) {
        let posicion = (fila * 80 + col) * 2;
        unsafe {
            *self.buffer.add(posicion) = byte;
            *self.buffer.add(posicion + 1) = self.codigo_color;
        }
    }
}

impl fmt::Write for EscritorVGA {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.escribir_texto(s);
        Ok(())
    }
}
