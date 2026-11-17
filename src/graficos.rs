use x86_64::instructions::port::Port;

const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

const VBE_DISPI_INDEX_XRES: u16 = 0x1;
const VBE_DISPI_INDEX_YRES: u16 = 0x2;
const VBE_DISPI_INDEX_BPP: u16 = 0x3;
const VBE_DISPI_INDEX_ENABLE: u16 = 0x4;

const VBE_DISPI_DISABLED: u16 = 0x00;
const VBE_DISPI_ENABLED: u16 = 0x01;
const VBE_DISPI_LFB_ENABLED: u16 = 0x40;

pub const ANCHO_PANTALLA: usize = 800;
pub const ALTO_PANTALLA: usize = 600;

unsafe fn escribir_registro_bga(indice: u16, valor: u16) {
    let mut puerto_idx: Port<u16> = Port::new(VBE_DISPI_IOPORT_INDEX);
    let mut puerto_dat: Port<u16> = Port::new(VBE_DISPI_IOPORT_DATA);
    puerto_idx.write(indice);
    puerto_dat.write(valor);
}

pub struct PantallaGrafica {
    pub framebuffer: *mut u32,
}

unsafe impl Send for PantallaGrafica {}

impl PantallaGrafica {
    pub fn inicializar(offset_memoria_fisica: u64) -> PantallaGrafica {
        unsafe {
            escribir_registro_bga(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
            escribir_registro_bga(VBE_DISPI_INDEX_XRES, ANCHO_PANTALLA as u16);
            escribir_registro_bga(VBE_DISPI_INDEX_YRES, ALTO_PANTALLA as u16);
            escribir_registro_bga(VBE_DISPI_INDEX_BPP, 32);
            escribir_registro_bga(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);
        }

        let direccion_virtual_fb = (offset_memoria_fisica + 0xFD00_0000) as *mut u32;
        PantallaGrafica {
            framebuffer: direccion_virtual_fb,
        }
    }

    pub fn obtener_lienzo(offset_memoria_fisica: u64) -> PantallaGrafica {
        PantallaGrafica {
            framebuffer: (offset_memoria_fisica + 0xFD00_0000) as *mut u32,
        }
    }

    #[inline]
    pub fn pintar_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < ANCHO_PANTALLA && y < ALTO_PANTALLA {
            let indice = y * ANCHO_PANTALLA + x;
            unsafe {
                self.framebuffer.add(indice).write_volatile(color);
            }
        }
    }

    pub fn dibujar_rectangulo(&mut self, x: usize, y: usize, ancho: usize, alto: usize, color: u32) {
        for dy in 0..alto {
            for dx in 0..ancho {
                self.pintar_pixel(x + dx, y + dy, color);
            }
        }
    }

    pub fn dibujar_ventana(&mut self, x: usize, y: usize, ancho: usize, alto: usize, color_barra: u32) {
        self.dibujar_rectangulo(x + 6, y + 6, ancho, alto, 0x00070B14);
        self.dibujar_rectangulo(x, y, ancho, alto, 0x001E293B);
        self.dibujar_rectangulo(x, y, ancho, 28, color_barra);
        self.dibujar_rectangulo(x + ancho - 22, y + 6, 16, 16, 0x00EF4444);
        for i in 4..12 {
            self.pintar_pixel(x + ancho - 22 + i, y + 6 + i, 0x00FFFFFF);
            self.pintar_pixel(x + ancho - 22 + (15 - i), y + 6 + i, 0x00FFFFFF);
        }
        self.dibujar_rectangulo(x, y, ancho, 2, 0x0064748B);
        self.dibujar_rectangulo(x, y + alto - 2, ancho, 2, 0x0064748B);
        self.dibujar_rectangulo(x, y, 2, alto, 0x0064748B);
        self.dibujar_rectangulo(x + ancho - 2, y, 2, alto, 0x0064748B);
    }

    pub fn dibujar_cursor(&mut self, cx: usize, cy: usize) {
        let flecha: [&str; 12] = [
            "X...........",
            "XX..........",
            "XOX.........",
            "XOOX........",
            "XOOOX.......",
            "XOOOOX......",
            "XOOOOOX.....",
            "XOOOOOOX....",
            "XOOOOXXXX...",
            "XOOXOOX.....",
            "X.X.XOOX....",
            ".....XX.....",
        ];

        for (dy, fila) in flecha.iter().enumerate() {
            for (dx, ch) in fila.chars().enumerate() {
                match ch {
                    'X' => self.pintar_pixel(cx + dx, cy + dy, 0x00000000),
                    'O' => self.pintar_pixel(cx + dx, cy + dy, 0x00FFFFFF),
                    _ => {}
                }
            }
        }
    }

    pub fn renderizar_escritorio(&mut self, v1_abierta: bool, v2_abierta: bool, cx: usize, cy: usize) {
        self.dibujar_rectangulo(0, 0, ANCHO_PANTALLA, ALTO_PANTALLA, 0x000F172A);

        if v1_abierta {
            self.dibujar_ventana(60, 50, 380, 260, 0x002563EB);
            self.dibujar_rectangulo(80, 100, 200, 14, 0x0038BDF8);
            self.dibujar_rectangulo(80, 130, 320, 12, 0x00475569);
            self.dibujar_rectangulo(80, 152, 260, 12, 0x00475569);
        }

        if v2_abierta {
            self.dibujar_ventana(340, 180, 400, 280, 0x00059669);
            self.dibujar_rectangulo(360, 225, 360, 210, 0x00090D16);
            self.dibujar_rectangulo(375, 245, 140, 10, 0x0010B981);
        }

        self.dibujar_rectangulo(0, ALTO_PANTALLA - 36, ANCHO_PANTALLA, 36, 0x001E293B);
        self.dibujar_rectangulo(8, ALTO_PANTALLA - 30, 90, 24, 0x003B82F6);

        self.dibujar_cursor(cx, cy);
    }
}
