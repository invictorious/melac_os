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
    // Búfer de hasta 64 letras para guardar el comando que el usuario está escribiendo
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

    // Procesa cada tecla pulsada por el usuario
    pub fn procesar_tecla(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                // El usuario presionó ENTER: bajamos de línea y ejecutamos el comando
                self.nueva_linea();
                self.ejecutar_comando();
                self.longitud_comando = 0; // Vaciamos el búfer para el siguiente comando
                self.escribir_texto("melac_os> ");
            }
            0x08 => {
                // Tecla BACKSPACE: borramos en pantalla y quitamos 1 letra del búfer
                if self.longitud_comando > 0 {
                    self.longitud_comando -= 1;
                    self.columna_actual -= 1;
                    self.escribir_caracter_en(b' ', self.fila_actual, self.columna_actual);
                }
            }
            byte => {
                // Guardamos la letra si cabe en los 64 espacios y la dibujamos
                if self.longitud_comando < 64 && self.columna_actual < 79 {
                    self.linea_comando[self.longitud_comando] = byte;
                    self.longitud_comando += 1;
                    self.escribir_caracter_en(byte, self.fila_actual, self.columna_actual);
                    self.columna_actual += 1;
                }
            }
        }
    }

    // El cerebro de nuestra Shell: compara los bytes escritos con comandos conocidos
        fn ejecutar_comando(&mut self) {
        // Hacemos una copia local de los 64 bytes para no mantener prestado 'self'
        let copia_buffer = self.linea_comando;
        let slice = &copia_buffer[..self.longitud_comando];

        if let Ok(comando) = core::str::from_utf8(slice) {
            match comando {
                "" => {}
                "help" => {
                    self.cambiar_color(Color::CianClaro, Color::Negro);
                    self.escribir_texto("Comandos disponibles en melac_os:\n");
                    self.escribir_texto("  help  - Muestra esta lista de ayuda\n");
                    self.escribir_texto("  info  - Muestra informacion del kernel y hardware\n");
                    self.escribir_texto("  clear - Limpia la pantalla\n");
                    self.escribir_texto("  panic - Simula un error critico del sistema\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "info" => {
                    self.cambiar_color(Color::Amarillo, Color::Negro);
                    self.escribir_texto("Sistema: melac_os v0.3 (MKernel)\n");
                    self.escribir_texto("Arquitectura: x86_64 Bare Metal (Sin SO anfitrion)\n");
                    self.escribir_texto("Memoria Video: VGA Text Buffer 80x25 (0xb8000)\n");
                    self.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
                "clear" => {
                    self.limpiar_pantalla();
                }
                "panic" => {
                    panic!("Pánico provocado manualmente desde la Shell!");
                }
                desconocido => {
                    self.cambiar_color(Color::RojoClaro, Color::Negro);
                    self.escribir_texto("Comando desconocido: '");
                    self.escribir_texto(desconocido);
                    self.escribir_texto("'. Escribe 'help'.\n");
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
            // Si llegamos abajo del todo, subimos todas las filas 1 espacio (Scroll)
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
            // Limpiamos la última fila (fila 24)
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
