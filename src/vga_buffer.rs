use crate::fs::{borrar_archivo, crear_archivo, leer_archivo, SISTEMA_ARCHIVOS};
use crate::hardware::{apagar_sistema, emitir_beep, reiniciar_sistema};
use crate::interrupciones::TICS_RELOJ;
use alloc::string::String;
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

#[derive(PartialEq, Eq)]
pub enum ModoSistema {
    Consola,
    Editor,
}

pub struct EscritorVGA {
    columna_actual: usize,
    fila_actual: usize,
    codigo_color: u8,
    buffer: *mut u8,
    linea_comando: [u8; 64],
    longitud_comando: usize,
    modo: ModoSistema,
    archivo_editando: String,
    contenido_editor: String,
}

unsafe impl Send for EscritorVGA {}

pub static PANTALLA_GLOBAL: Mutex<EscritorVGA> = Mutex::new(EscritorVGA {
    columna_actual: 0,
    fila_actual: 0,
    codigo_color: ((Color::Negro as u8) << 4) | (Color::VerdeClaro as u8),
    buffer: 0xb8000 as *mut u8,
    linea_comando: [0; 64],
    longitud_comando: 0,
    modo: ModoSistema::Consola,
    archivo_editando: String::new(),
    contenido_editor: String::new(),
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

    fn abrir_editor(&mut self, nombre: &str) {
        self.modo = ModoSistema::Editor;
        self.archivo_editando = String::from(nombre);
        self.contenido_editor = leer_archivo(nombre).unwrap_or_else(String::new);

        self.limpiar_pantalla();

        self.cambiar_color(Color::Blanco, Color::Azul);
        for col in 0..80 {
            self.escribir_caracter_en(b' ', 0, col);
        }
        self.fila_actual = 0;
        self.columna_actual = 1;
        let _ = write!(
            self,
            "MELAC_EDIT v0.1 | Archivo: {} | Pulsa [ESC] para Guardar",
            nombre
        );

        self.cambiar_color(Color::Blanco, Color::Negro);
        self.fila_actual = 2;
        self.columna_actual = 0;
        let texto_previo = self.contenido_editor.clone();
        self.escribir_texto(&texto_previo);
    }

    fn cerrar_y_guardar_editor(&mut self) {
        crear_archivo(&self.archivo_editando, &self.contenido_editor);
        let nombre_guardado = self.archivo_editando.clone();

        self.modo = ModoSistema::Consola;
        self.contenido_editor.clear();
        self.archivo_editando.clear();

        self.cambiar_color(Color::VerdeClaro, Color::Negro);
        self.limpiar_pantalla();
        self.cambiar_color(Color::CianClaro, Color::Negro);
        let _ = writeln!(self, "[OK] Archivo '{}' guardado en RAM.\n", nombre_guardado);
        self.cambiar_color(Color::VerdeClaro, Color::Negro);
        self.escribir_texto("melac_os> ");
    }

    pub fn procesar_tecla(&mut self, byte: u8) {
        if self.modo == ModoSistema::Editor {
            match byte {
                0x1B => self.cerrar_y_guardar_editor(),
                0x08 => {
                    if !self.contenido_editor.is_empty() {
                        let ultimo = self.contenido_editor.pop();
                        if ultimo == Some('\n') {
                            let nombre = self.archivo_editando.clone();
                            let respaldo = self.contenido_editor.clone();
                            self.abrir_editor(&nombre);
                            self.contenido_editor = respaldo.clone();
                            self.fila_actual = 2;
                            self.columna_actual = 0;
                            self.escribir_texto(&respaldo);
                        } else if self.columna_actual > 0 {
                            self.columna_actual -= 1;
                            self.escribir_caracter_en(b' ', self.fila_actual, self.columna_actual);
                        }
                    }
                }
                b'\n' => {
                    self.contenido_editor.push('\n');
                    self.nueva_linea();
                }
                byte => {
                    if byte >= 32 && byte <= 126 {
                        self.contenido_editor.push(byte as char);
                        if self.columna_actual >= 80 {
                            self.nueva_linea();
                        }
                        self.escribir_caracter_en(byte, self.fila_actual, self.columna_actual);
                        self.columna_actual += 1;
                    }
                }
            }
            return;
        }

        match byte {
            b'\n' => {
                self.nueva_linea();
                self.ejecutar_comando();
                self.longitud_comando = 0;
                if self.modo == ModoSistema::Consola {
                    self.escribir_texto("melac_os> ");
                }
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

    // Intérprete MelacScript: ejecuta instrucciones línea por línea desde un archivo
    fn ejecutar_script(&mut self, codigo: &str) {
        for linea in codigo.lines() {
            let linea = linea.trim();
            if linea.is_empty() || linea.starts_with('#') {
                continue;
            }
            let partes: Vec<&str> = linea.splitn(2, ' ').collect();
            match partes[0] {
                "print" => {
                    if partes.len() > 1 {
                        let _ = writeln!(self, "{}", partes[1]);
                    } else {
                        self.nueva_linea();
                    }
                }
                "color" => {
                    if partes.len() > 1 {
                        match partes[1] {
                            "verde" => self.cambiar_color(Color::VerdeClaro, Color::Negro),
                            "cian" => self.cambiar_color(Color::CianClaro, Color::Negro),
                            "amarillo" => self.cambiar_color(Color::Amarillo, Color::Negro),
                            "rojo" => self.cambiar_color(Color::RojoClaro, Color::Negro),
                            "blanco" => self.cambiar_color(Color::Blanco, Color::Negro),
                            _ => {}
                        }
                    }
                }
                "clear" => self.limpiar_pantalla(),
                "beep" => emitir_beep(),
                otra => {
                    self.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(self, "[Script Error] Instruccion desconocida: '{}'", otra);
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
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
                    self.escribir_texto("Comandos de melac_os v0.8:\n");
                    self.escribir_texto("  edit <nom>        - Abre el editor de texto (ESC guarda)\n");
                    self.escribir_texto("  run <nom>         - Ejecuta un script MelacScript (.ms)\n");
                    self.escribir_texto("  ls / cat / touch / rm - Gestion de archivos en RAM\n");
                    self.escribir_texto("  beep / uptime / info / clear\n");
                    self.escribir_texto("  reboot / shutdown / panic - Control de hardware\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "edit" => {
                    if partes.len() < 2 {
                        self.cambiar_color(Color::RojoClaro, Color::Negro);
                        self.escribir_texto("Uso: edit <nombre_archivo>\n");
                        self.cambiar_color(Color::VerdeClaro, Color::Negro);
                    } else {
                        self.abrir_editor(partes[1]);
                    }
                }
                "run" => {
                    if partes.len() < 2 {
                        self.cambiar_color(Color::RojoClaro, Color::Negro);
                        self.escribir_texto("Uso: run <archivo.ms>\n");
                    } else {
                        match leer_archivo(partes[1]) {
                            Some(codigo) => self.ejecutar_script(&codigo),
                            None => {
                                self.cambiar_color(Color::RojoClaro, Color::Negro);
                                let _ = writeln!(self, "Error: No existe el script '{}'.", partes[1]);
                            }
                        }
                    }
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "beep" => {
                    emitir_beep();
                    self.escribir_texto("BEEP (Altavoz PC Speaker activado)\n");
                }
                "reboot" => {
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    self.escribir_texto("Reiniciando procesador...\n");
                    reiniciar_sistema();
                }
                "shutdown" => {
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    self.escribir_texto("Apagando sistema ACPI...\n");
                    apagar_sistema();
                }
                "info" => {
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    self.escribir_texto("Sistema: melac_os v0.8 (MKernel + RamFS + MelacScript)\n");
                    self.escribir_texto("Arquitectura: x86_64 Bare Metal | Heap: 100 KiB\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "uptime" => {
                    let tics = TICS_RELOJ.load(Ordering::Relaxed);
                    let segundos = (tics * 10) / 182;
                    self.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(
                        self,
                        "Tiempo encendido: {} segundos ({} tics)",
                        segundos, tics
                    );
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "ls" => {
                    let archivos = SISTEMA_ARCHIVOS.lock();
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    if archivos.is_empty() {
                        self.escribir_texto("(Directorio vacio)\n");
                    } else {
                        for arch in archivos.iter() {
                            let _ = writeln!(
                                self,
                                "  - {} ({} bytes)",
                                arch.nombre,
                                arch.contenido.len()
                            );
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
                    let _ = writeln!(
                        self,
                        "Comando desconocido: '{}'. Escribe 'help'.",
                        desconocido
                    );
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
            let inicio_fila = if self.modo == ModoSistema::Editor { 3 } else { 1 };
            for fila in inicio_fila..25 {
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
