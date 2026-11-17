// 1. Definimos los 16 colores estándar del hardware VGA mediante un Enum
#[allow(dead_code)] // Evita avisos amarillos si aún no usamos todos los colores
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)] // Cada color ocupa exactamente 1 byte (u8)
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

// 2. El Escritor (Writer) controla en qué columna estamos y qué color usamos
pub struct EscritorVGA {
    columna_actual: usize,
    fila_actual: usize,
    codigo_color: u8,
    buffer: *mut u8,
}

// 'impl' nos permite darle habilidades (métodos) a nuestro struct EscritorVGA
impl EscritorVGA {
    // Crea un escritor nuevo apuntando a la dirección física 0xb8000
    pub fn nuevo(texto: Color, fondo: Color) -> EscritorVGA {
        // Combinamos el color de fondo (4 bits altos) y el de texto (4 bits bajos)
        let color_combinado = ((fondo as u8) << 4) | (texto as u8);
        EscritorVGA {
            columna_actual: 0,
            fila_actual: 0,
            codigo_color: color_combinado,
            buffer: 0xb8000 as *mut u8,
        }
    }

    // Borra toda la pantalla (la pantalla VGA mide 80 columnas x 25 filas)
    pub fn limpiar_pantalla(&mut self) {
        for fila in 0..25 {
            for col in 0..80 {
                self.escribir_caracter_en(b' ', fila, col);
            }
        }
        self.fila_actual = 0;
        self.columna_actual = 0;
    }

    // Escribe un texto completo letra por letra
    pub fn escribir_texto(&mut self, texto: &str) {
        for byte in texto.bytes() {
            match byte {
                // Si encuentra un salto de línea '\n', baja a la siguiente fila
                b'\n' => self.nueva_linea(),
                // Si es cualquier otro carácter imprimible, lo dibuja en pantalla
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

    // Salta a la fila de abajo y vuelve a la columna 0
    fn nueva_linea(&mut self) {
        self.columna_actual = 0;
        if self.fila_actual < 24 {
            self.fila_actual += 1;
        }
    }

    // Escribe un único byte y su color en la coordenada exacta (fila, columna)
    fn escribir_caracter_en(&mut self, byte: u8, fila: usize, col: usize) {
        let posicion = (fila * 80 + col) * 2;
        unsafe {
            *self.buffer.add(posicion) = byte;
            *self.buffer.add(posicion + 1) = self.codigo_color;
        }
    }
}
