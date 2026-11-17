use crate::fs::{borrar_archivo, crear_archivo, leer_archivo, SISTEMA_ARCHIVOS};
use crate::interrupciones::TICS_RELOJ;
use alloc::vec::Vec;
use core::fmt::{self, Write};
use core::sync::atomic::Ordering;
use spin::Mutex;

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
    columna_actual: usize,
    fila_actual: usize,
    codigo_color: u8,
    buffer: *mut u8,
    linea_comando: [u8; 64],
    longitud_comando: usize,
}

unsafe impl Send for EscritorVGA {}

pub static PANTALLA_GLOBAL: Mutex<EscritorVGA> = Mutex::new(EscritorVGA {
    columna_actual: 0,
    fila_actual: 0,
    codigo_color: ((Color::Negro as u8) << 4) | (Color::VerdeClaro as u8),
    buffer: 0xb8000 as *mut u8,
    linea_comando: [0; 64],
    longitud_comando: 0,
});

impl EscritorVGA {
    pub fn cambiar_color(&mut self, texto: Color, fondo: Color) {
        self.codigo_color = ((fondo as u8) << 4) | (texto as u8);
    }

    pub fn limpiar_pantalla(&mut self) {
        for fila in 0..25 {
            for col in 0..80 {
                self.escribir_caracter_en(b' ', fila, col);
            }
        }
        self.fila_actual = 0;
        self.columna_actual = 0;
    }

    pub fn escribir_texto(&mut self, texto: &str) {
        for byte in texto.bytes() {
            match byte {
                b'\n' => self.nueva_linea(),
                byte => {
                    if self.columna_actual >= 80 {
                        self.nueva_linea();
                    }
                    self.escribir_caracter_en(byte, self.fila_actual, self.columna_actual);
                    self.columna_actual += 1;
                }
            }
        }
    }

    pub fn procesar_tecla(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.nueva_linea();
                self.ejecutar_comando();
                self.longitud_comando = 0;
                self.escribir_texto("melac_os> ");
            }
            0x08 => {
                if self.longitud_comando > 0 {
                    self.longitud_comando -= 1;
                    self.columna_actual -= 1;
                    self.escribir_caracter_en(b' ', self.fila_actual, self.columna_actual);
                }
            }
            byte => {
                if self.longitud_comando < 64 && self.columna_actual < 79 {
                    self.linea_comando[self.longitud_comando] = byte;
                    self.longitud_comando += 1;
                    self.escribir_caracter_en(byte, self.fila_actual, self.columna_actual);
                    self.columna_actual += 1;
                }
            }
        }
    }

    fn ejecutar_comando(&mut self) {
        let copia_buffer = self.linea_comando;
        let slice = &copia_buffer[..self.longitud_comando];

        if let Ok(linea) = core::str::from_utf8(slice) {
            let linea = linea.trim();
            if linea.is_empty() {
                return;
            }

            let partes: Vec<&str> = linea.splitn(3, ' ').collect();
            let comando = partes[0];

            match comando {
                "help" => {
                    self.cambiar_color(Color::CianClaro, Color::Negro);
                    self.escribir_texto("Comandos de melac_os v0.6:\n");
                    self.escribir_texto("  ls                  - Lista archivos en RAM\n");
                    self.escribir_texto("  touch <nom> <txt>   - Crea un archivo con texto\n");
                    self.escribir_texto("  cat <nom>           - Lee el contenido de un archivo\n");
                    self.escribir_texto("  rm <nom>            - Borra un archivo de la RAM\n");
                    self.escribir_texto("  info / uptime / clear / panic\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "info" => {
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    self.escribir_texto("Sistema: melac_os v0.6 (MKernel + RamFS)\n");
                    self.escribir_texto("Arquitectura: x86_64 Bare Metal | Heap: 100 KiB\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "uptime" => {
                    let tics = TICS_RELOJ.load(Ordering::Relaxed);
                    let segundos = (tics * 10) / 182;
                    self.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(self, "Tiempo encendido: {} segundos ({} tics)", segundos, tics);
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "ls" => {
                    let archivos = SISTEMA_ARCHIVOS.lock();
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    if archivos.is_empty() {
                        self.escribir_texto("(Directorio vacio. Usa 'touch archivo.txt hola')\n");
                    } else {
                        for arch in archivos.iter() {
                            let _ = writeln!(self, "  - {} ({} bytes)", arch.nombre, arch.contenido.len());
                        }
                    }
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "touch" => {
                    if partes.len() < 2 {
                        self.cambiar_color(Color::RojoClaro, Color::Negro);
                        self.escribir_texto("Uso: touch <nombre> [contenido]\n");
                    } else {
                        let nombre = partes[1];
                        let contenido = if partes.len() == 3 { partes[2] } else { "" };
                        crear_archivo(nombre, contenido);
                        self.cambiar_color(Color::CianClaro, Color::Negro);
                        let _ = writeln!(self, "Archivo '{}' guardado en RAM.", nombre);
                    }
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "cat" => {
                    if partes.len() < 2 {
                        self.cambiar_color(Color::RojoClaro, Color::Negro);
                        self.escribir_texto("Uso: cat <nombre>\n");
                    } else {
                        let nombre = partes[1];
                        match leer_archivo(nombre) {
                            Some(texto) => {
                                self.cambiar_color(Color::Blanco, Color::Negro);
                                let _ = writeln!(self, "{}", texto);
                            }
                            None => {
                                self.cambiar_color(Color::RojoClaro, Color::Negro);
                                let _ = writeln!(self, "Error: El archivo '{}' no existe.", nombre);
                            }
                        }
                    }
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "rm" => {
                    if partes.len() < 2 {
                        self.cambiar_color(Color::RojoClaro, Color::Negro);
                        self.escribir_texto("Uso: rm <nombre>\n");
                    } else {
                        let nombre = partes[1];
                        if borrar_archivo(nombre) {
                            self.cambiar_color(Color::Amarillo, Color::Negro);
                            let _ = writeln!(self, "Archivo '{}' eliminado.", nombre);
                        } else {
                            self.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(self, "Error: No existe '{}'.", nombre);
                        }
                    }
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "clear" => {
                    self.limpiar_pantalla();
                }
                "panic" => {
                    panic!("Panico provocado manualmente desde la Shell!");
                }
                desconocido => {
                    self.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(self, "Comando desconocido: '{}'. Escribe 'help'.", desconocido);
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
            }
        }
    }

    fn nueva_linea(&mut self) {
        self.columna_actual = 0;
        if self.fila_actual < 24 {
            self.fila_actual += 1;
        } else {
            self.hacer_scroll();
        }
    }

    fn hacer_scroll(&mut self) {
        unsafe {
            for fila in 1..25 {
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

    fn escribir_caracter_en(&mut self, byte: u8, fila: usize, col: usize) {
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
