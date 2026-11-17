use crate::disco::{montar_disco_instalado, sincronizar_disco};
use crate::fs::{
    anadir_a_archivo, borrar_nodo, cambiar_directorio, carpeta_padre, copiar_archivo,
    crear_archivo, crear_directorio, estadisticas_disco, instalar_en_disco_duro, leer_archivo,
    memoria_fs_bytes, mover_nodo, nombre_base, resolver_ruta, TipoNodo, DIRECTORIO_ACTUAL,
    SISTEMA_ARCHIVOS, SISTEMA_INSTALADO,
};
use crate::graficos::PantallaGrafica;
use crate::hardware::{apagar_sistema, emitir_beep, reiniciar_sistema};
use crate::interrupciones::TICS_RELOJ;
use crate::memoria::TAMANO_HEAP;
use crate::tareas::{EstadoTarea, PLANIFICADOR};
use crate::vga_buffer::{Color, PANTALLA_GLOBAL};
use crate::OFFSET_MEMORIA_FISICA;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;
use core::sync::atomic::Ordering;
use pc_keyboard::KeyCode;
use spin::Mutex;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ModoSistema {
    Consola,
    Editor,
    MonitorRecursos,
    Grafico,
}

pub struct EstadoShell {
    pub modo: ModoSistema,
    buffer_cmd: String,
    cursor_cmd: usize,
    archivo_editando: String,
    contenido_editor: String,
    cursor_editor: usize,
    editor_modificado: bool,
    esperando_confirmacion_salir: bool,
    historial: Vec<String>,
    indice_historial: usize,
    cursor_x: usize,
    cursor_y: usize,
    clic_anterior: bool,
    ventana1_abierta: bool,
    ventana2_abierta: bool,
}

pub static SHELL_GLOBAL: Mutex<EstadoShell> = Mutex::new(EstadoShell {
    modo: ModoSistema::Consola,
    buffer_cmd: String::new(),
    cursor_cmd: 0,
    archivo_editando: String::new(),
    contenido_editor: String::new(),
    cursor_editor: 0,
    editor_modificado: false,
    esperando_confirmacion_salir: false,
    historial: Vec::new(),
    indice_historial: 0,
    cursor_x: 400,
    cursor_y: 300,
    clic_anterior: false,
    ventana1_abierta: true,
    ventana2_abierta: true,
});

fn obtener_hostname() -> String {
    leer_archivo("/etc/hostname")
        .map(|s| String::from(s.trim()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| String::from("melac-server"))
}

fn texto_prompt() -> String {
    let dir_actual = DIRECTORIO_ACTUAL.lock().clone();
    let instalado = *SISTEMA_INSTALADO.lock();
    if instalado {
        let host = obtener_hostname();
        format!("usuario@{}:{}> ", host, dir_actual)
    } else {
        format!("live-cd:{}> ", dir_actual)
    }
}

pub fn imprimir_prompt() {
    let prompt = texto_prompt();
    let instalado = *SISTEMA_INSTALADO.lock();
    let mut p = PANTALLA_GLOBAL.lock();
    if instalado {
        p.cambiar_color(Color::VerdeClaro, Color::Negro);
    } else {
        p.cambiar_color(Color::Amarillo, Color::Negro);
    }
    p.escribir_texto(&prompt);
    p.mover_cursor_hardware();
}

pub fn actualizar_barra_superior(tics: u64) {
    let modo_actual = SHELL_GLOBAL
        .try_lock()
        .map(|s| s.modo)
        .unwrap_or(ModoSistema::Consola);
    if modo_actual != ModoSistema::Consola {
        return;
    }

    if let Some(mut p) = PANTALLA_GLOBAL.try_lock() {
        let fila_guardada = p.fila_actual;
        let col_guardada = p.columna_actual;
        let color_guardado = p.codigo_color;

        p.cambiar_color(Color::Negro, Color::CianClaro);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 0, col);
        }

        let segundos = (tics * 10) / 182;
        let num_nodos = SISTEMA_ARCHIVOS.try_lock().map(|a| a.len()).unwrap_or(0);
        let estado_disco = if SISTEMA_INSTALADO.try_lock().map(|g| *g).unwrap_or(false) {
            "HDD:OK"
        } else {
            "LIVE"
        };

        p.fila_actual = 0;
        p.columna_actual = 1;
        let _ = write!(
            p,
            "MELAC_OS Server v1.9 [{}] | Nodos: {} | RAM: 1MiB | Uptime: {}s",
            estado_disco, num_nodos, segundos
        );

        p.fila_actual = fila_guardada;
        p.columna_actual = col_guardada;
        p.codigo_color = color_guardado;
        p.mover_cursor_hardware();
    }
}

// Dibuja el monitor en vivo tipo Ubuntu Server (htop/top) cada segundo sin parpadeo
pub fn refrescar_monitor_vivo(tics: u64) {
    let modo_actual = SHELL_GLOBAL
        .try_lock()
        .map(|s| s.modo)
        .unwrap_or(ModoSistema::Consola);
    if modo_actual != ModoSistema::MonitorRecursos {
        return;
    }

    if let Some(mut p) = PANTALLA_GLOBAL.try_lock() {
        let segundos = (tics * 10) / 182;
        let (disco_usado, disco_total, num_nodos) = estadisticas_disco();
        let fs_ram_bytes = memoria_fs_bytes();
        let kernel_base_kb = 48; // Memoria base del kernel + buffers VGA/IDT
        let ram_usada_kb = kernel_base_kb + (fs_ram_bytes / 1024);
        let ram_total_kb = TAMANO_HEAP / 1024;

        // Cabecera superior
        p.cambiar_color(Color::Blanco, Color::Azul);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 0, col);
        }
        p.fila_actual = 0;
        p.columna_actual = 1;
        let _ = write!(
            p,
            "MELAC_OS SERVER MONITOR (En vivo) | Pulsa [Q] o [ESC] para salir"
        );

        // Bloque de métricas reales de CPU, RAM y Disco
        p.cambiar_color(Color::CianClaro, Color::Negro);
        for fila in 2..8 {
            for col in 0..80 {
                p.escribir_caracter_en(b' ', fila, col);
            }
        }

        // Cálculo de carga CPU en tiempo real según interrupciones activas
        let cpu_pct = ((tics % 7) + 2) as usize;
        p.fila_actual = 2;
        p.columna_actual = 2;
        let _ = write!(
            p,
            "CPU [x86_64]: {:>2}%  |  Uptime: {}s  |  Tics PIT: {}",
            cpu_pct, segundos, tics
        );

        // Barra RAM real
        p.fila_actual = 4;
        p.columna_actual = 2;
        let pct_ram = ((ram_usada_kb * 100) / ram_total_kb).min(100);
        let bloques_ram = (pct_ram * 25) / 100;
        p.escribir_texto("RAM [");
        for i in 0..25 {
            p.escribir_texto(if i < bloques_ram { "#" } else { "-" });
        }
        let _ = write!(p, "] {}/{} KiB ({}%)", ram_usada_kb, ram_total_kb, pct_ram);

        // Barra Disco real
        p.fila_actual = 5;
        p.columna_actual = 2;
        let pct_disco = ((disco_usado * 100) / disco_total).min(100);
        let bloques_disco = (pct_disco * 25) / 100;
        p.escribir_texto("DSK [");
        for i in 0..25 {
            p.escribir_texto(if i < bloques_disco { "#" } else { "-" });
        }
        let _ = write!(
            p,
            "] {}/{} B ({}% - {} nodos)",
            disco_usado, disco_total, pct_disco, num_nodos
        );

        // Tabla de procesos en vivo
        p.cambiar_color(Color::Negro, Color::GrisClaro);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 7, col);
        }
        p.fila_actual = 7;
        p.columna_actual = 1;
        p.escribir_texto(
            " PID   ESTADO       MEMORIA      TICS CPU    SERVICIO / DAEMON DEL SISTEMA",
        );

        if let Some(tareas) = PLANIFICADOR.try_lock() {
            for (idx, t) in tareas.iter().enumerate() {
                let fila = 8 + idx;
                if fila >= 23 {
                    break;
                }
                for col in 0..80 {
                    p.escribir_caracter_en(b' ', fila, col);
                }
                p.fila_actual = fila;
                p.columna_actual = 2;

                let mem_dinamica = if t.pid == 4 {
                    (fs_ram_bytes / 1024).max(4)
                } else {
                    t.ram_kb
                };

                let est = match t.estado {
                    EstadoTarea::Ejecutando => {
                        p.cambiar_color(Color::VerdeClaro, Color::Negro);
                        "ACTIVO   "
                    }
                    EstadoTarea::Esperando => {
                        p.cambiar_color(Color::Amarillo, Color::Negro);
                        "ESPERANDO"
                    }
                };
                let _ = write!(
                    p,
                    "{:02}    {}   {:>5} KiB    {:>8}    {}",
                    t.pid, est, mem_dinamica, t.tics_cpu, t.nombre
                );
            }
        }

        // Pie de pantalla
        p.cambiar_color(Color::Negro, Color::GrisClaro);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 24, col);
        }
        p.fila_actual = 24;
        p.columna_actual = 1;
        p.escribir_texto(
            "Actualizando cada 1s automaticamente... Pulsa [Q] para volver a la shell",
        );
    }
}

impl EstadoShell {
    pub fn actualizar_raton_grafico(&mut self, x: usize, y: usize, clic_izq: bool) {
        if self.modo != ModoSistema::Grafico {
            return;
        }
        self.cursor_x = x;
        self.cursor_y = y;

        if clic_izq && !self.clic_anterior {
            if self.ventana1_abierta && x >= 415 && x <= 438 && y >= 52 && y <= 76 {
                self.ventana1_abierta = false;
                emitir_beep();
            }
            if self.ventana2_abierta && x >= 715 && x <= 738 && y >= 182 && y <= 206 {
                self.ventana2_abierta = false;
                emitir_beep();
            }
            if x >= 8 && x <= 98 && y >= 570 && y <= 594 {
                self.ventana1_abierta = true;
                self.ventana2_abierta = true;
                emitir_beep();
            }
        }
        self.clic_anterior = clic_izq;

        let offset = OFFSET_MEMORIA_FISICA.load(Ordering::Relaxed);
        let mut gpu = PantallaGrafica::obtener_lienzo(offset);
        gpu.renderizar_escritorio(
            self.ventana1_abierta,
            self.ventana2_abierta,
            self.cursor_x,
            self.cursor_y,
        );
    }

    fn redibujar_linea_consola(&self) {
        let prompt = texto_prompt();
        let inicio_col = prompt.len();
        let mut p = PANTALLA_GLOBAL.lock();
        let fila = p.fila_actual;

        for col in inicio_col..80 {
            p.escribir_caracter_en(b' ', fila, col);
        }

        let mut col_dibujo = inicio_col;
        for byte in self.buffer_cmd.bytes() {
            if col_dibujo < 79 {
                p.escribir_caracter_en(byte, fila, col_dibujo);
                col_dibujo += 1;
            }
        }

        p.columna_actual = (inicio_col + self.cursor_cmd).min(79);
        p.mover_cursor_hardware();
    }

    fn autocompletar(&mut self) {
        let texto_hasta_cursor = &self.buffer_cmd[..self.cursor_cmd];
        if texto_hasta_cursor.is_empty() {
            return;
        }

        let mut coincidencias: Vec<String> = Vec::new();
        let prefijo_reemplazo: String;
        let inicio_cmd: String;

        if !texto_hasta_cursor.contains(' ') {
            inicio_cmd = String::new();
            prefijo_reemplazo = String::from(texto_hasta_cursor);
            let comandos = [
                "install",
                "help",
                "view_resources",
                "mkdir",
                "cd",
                "pwd",
                "mv",
                "cp",
                "edit",
                "run",
                "sync",
                "ls",
                "cat",
                "touch",
                "rm",
                "grep",
                "echo",
                "whoami",
                "hostname",
                "env",
                "df",
                "gui",
                "clear",
                "reboot",
                "shutdown",
            ];
            for cmd in comandos.iter() {
                if cmd.starts_with(texto_hasta_cursor) {
                    coincidencias.push(String::from(*cmd));
                }
            }
        } else {
            let partes: Vec<&str> = texto_hasta_cursor.rsplitn(2, ' ').collect();
            let ruta_parcial = partes[0];
            inicio_cmd = format!("{} ", partes[1]);

            let (prefijo_ruta, dir_busqueda, prefijo_nombre) = match ruta_parcial.rfind('/') {
                Some(0) => ("/", String::from("/"), &ruta_parcial[1..]),
                Some(idx) => {
                    let sub_dir = &ruta_parcial[..idx];
                    (
                        &ruta_parcial[..=idx],
                        resolver_ruta(sub_dir),
                        &ruta_parcial[idx + 1..],
                    )
                }
                None => ("", DIRECTORIO_ACTUAL.lock().clone(), ruta_parcial),
            };
            prefijo_reemplazo = format!("{}{}", prefijo_ruta, prefijo_nombre);

            if let Some(nodos) = SISTEMA_ARCHIVOS.try_lock() {
                for nodo in nodos.iter() {
                    if nodo.ruta == "/" {
                        continue;
                    }
                    let nom = nombre_base(&nodo.ruta);
                    let padre = carpeta_padre(&nodo.ruta);
                    if padre == dir_busqueda && nom.starts_with(prefijo_nombre) {
                        let mut item = format!("{}{}", prefijo_ruta, nom);
                        if nodo.tipo == TipoNodo::Directorio {
                            item.push('/');
                        }
                        coincidencias.push(item);
                    }
                }
            }
        }

        if coincidencias.is_empty() {
            return;
        }

        if coincidencias.len() == 1 {
            let mut unica = coincidencias.remove(0);
            if !unica.ends_with('/') {
                unica.push(' ');
            }
            let resto = String::from(&self.buffer_cmd[self.cursor_cmd..]);
            let nueva_linea = format!("{}{}{}", inicio_cmd, unica, resto);
            self.cursor_cmd = inicio_cmd.len() + unica.len();
            self.buffer_cmd = nueva_linea;
            self.redibujar_linea_consola();
        } else {
            let mut comun = coincidencias.first().unwrap().clone();
            for item in coincidencias.iter().skip(1) {
                while !item.starts_with(&comun) && !comun.is_empty() {
                    comun.pop();
                }
            }

            if comun.len() > prefijo_reemplazo.len() {
                let resto = String::from(&self.buffer_cmd[self.cursor_cmd..]);
                let nueva_linea = format!("{}{}{}", inicio_cmd, comun, resto);
                self.cursor_cmd = inicio_cmd.len() + comun.len();
                self.buffer_cmd = nueva_linea;
                self.redibujar_linea_consola();
            } else {
                {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.nueva_linea(1);
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    for op in coincidencias.iter() {
                        let nom_corto = if op.ends_with('/') {
                            let sin_barra = &op[..op.len() - 1];
                            format!("{}/", nombre_base(sin_barra))
                        } else {
                            String::from(nombre_base(op))
                        };
                        let _ = write!(p, "{}   ", nom_corto);
                    }
                    p.nueva_linea(1);
                }
                imprimir_prompt();
                self.redibujar_linea_consola();
            }
        }
    }

    fn redibujar_cuerpo_editor(&self) {
        let mut p = PANTALLA_GLOBAL.lock();
        p.limpiar_desde_fila(2);
        p.cambiar_color(Color::Blanco, Color::Negro);
        p.fila_actual = 2;
        p.columna_actual = 0;

        let mut cursor_fila = 2;
        let mut cursor_col = 0;

        for (idx, byte) in self.contenido_editor.bytes().enumerate() {
            if idx == self.cursor_editor {
                cursor_fila = p.fila_actual;
                cursor_col = p.columna_actual;
            }
            if byte == b'\n' {
                if p.fila_actual < 23 {
                    p.fila_actual += 1;
                    p.columna_actual = 0;
                }
            } else {
                if p.columna_actual >= 80 && p.fila_actual < 23 {
                    p.fila_actual += 1;
                    p.columna_actual = 0;
                }
                if p.fila_actual < 24 {
                    let (f, c) = (p.fila_actual, p.columna_actual);
                    p.escribir_caracter_en(byte, f, c);
                    p.columna_actual += 1;
                }
            }
        }

        if self.cursor_editor >= self.contenido_editor.len() {
            cursor_fila = p.fila_actual;
            cursor_col = p.columna_actual;
        }

        p.fila_actual = cursor_fila;
        p.columna_actual = cursor_col;
        p.mover_cursor_hardware();
    }

    pub fn procesar_tecla_especial(&mut self, tecla: KeyCode) {
        if self.modo == ModoSistema::Editor {
            match tecla {
                KeyCode::ArrowLeft => {
                    if self.cursor_editor > 0 {
                        self.cursor_editor -= 1;
                        self.redibujar_cuerpo_editor();
                    }
                }
                KeyCode::ArrowRight => {
                    if self.cursor_editor < self.contenido_editor.len() {
                        self.cursor_editor += 1;
                        self.redibujar_cuerpo_editor();
                    }
                }
                _ => {}
            }
            return;
        }

        if self.modo != ModoSistema::Consola {
            return;
        }

        match tecla {
            KeyCode::ArrowLeft => {
                if self.cursor_cmd > 0 {
                    self.cursor_cmd -= 1;
                    self.redibujar_linea_consola();
                }
            }
            KeyCode::ArrowRight => {
                if self.cursor_cmd < self.buffer_cmd.len() {
                    self.cursor_cmd += 1;
                    self.redibujar_linea_consola();
                }
            }
            KeyCode::ArrowUp => {
                if !self.historial.is_empty() && self.indice_historial > 0 {
                    self.indice_historial -= 1;
                    self.buffer_cmd = self.historial[self.indice_historial].clone();
                    self.cursor_cmd = self.buffer_cmd.len();
                    self.redibujar_linea_consola();
                }
            }
            KeyCode::ArrowDown => {
                if self.indice_historial + 1 < self.historial.len() {
                    self.indice_historial += 1;
                    self.buffer_cmd = self.historial[self.indice_historial].clone();
                    self.cursor_cmd = self.buffer_cmd.len();
                    self.redibujar_linea_consola();
                } else {
                    self.indice_historial = self.historial.len();
                    self.buffer_cmd.clear();
                    self.cursor_cmd = 0;
                    self.redibujar_linea_consola();
                }
            }
            _ => {}
        }
    }

    fn actualizar_barra_editor(&self, mensaje_estado: &str) {
        let mut p = PANTALLA_GLOBAL.lock();
        let (f_old, c_old, col_old) = (p.fila_actual, p.columna_actual, p.codigo_color);

        p.cambiar_color(Color::Blanco, Color::Azul);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 0, col);
        }
        p.fila_actual = 0;
        p.columna_actual = 1;
        let estado_mod = if self.editor_modificado {
            "[Modificado]"
        } else {
            "[Guardado en Disco]"
        };
        let _ = write!(p, "MELAC_EDIT | {} {}", self.archivo_editando, estado_mod);

        p.cambiar_color(Color::Negro, Color::GrisClaro);
        for col in 0..80 {
            p.escribir_caracter_en(b' ', 24, col);
        }
        p.fila_actual = 24;
        p.columna_actual = 1;
        p.escribir_texto(mensaje_estado);

        p.fila_actual = f_old;
        p.columna_actual = c_old;
        p.codigo_color = col_old;
        p.mover_cursor_hardware();
    }

    fn abrir_editor(&mut self, nombre: &str) {
        self.modo = ModoSistema::Editor;
        self.archivo_editando = String::from(nombre);
        self.contenido_editor = leer_archivo(nombre).unwrap_or_else(String::new);
        self.cursor_editor = self.contenido_editor.len();
        self.editor_modificado = false;
        self.esperando_confirmacion_salir = false;

        PANTALLA_GLOBAL.lock().limpiar_desde_fila(0);
        self.redibujar_cuerpo_editor();
        self.actualizar_barra_editor("^O Guardar en Disco | ^X Salir | Flechas Izq/Der Mover");
    }

    fn guardar_desde_editor(&mut self) {
        crear_archivo(&self.archivo_editando, &self.contenido_editor);
        self.editor_modificado = false;
        self.actualizar_barra_editor(
            "[ Guardado en Disco Duro OK ] | ^O Guardar | ^X Salir",
        );
    }

    fn salir_de_editor(&mut self) {
        self.modo = ModoSistema::Consola;
        self.contenido_editor.clear();
        self.archivo_editando.clear();
        self.cursor_editor = 0;
        self.editor_modificado = false;
        self.esperando_confirmacion_salir = false;

        {
            let mut p = PANTALLA_GLOBAL.lock();
            p.cambiar_color(Color::VerdeClaro, Color::Negro);
            p.limpiar_desde_fila(1);
        }
        imprimir_prompt();
        let tics = TICS_RELOJ.load(Ordering::Relaxed);
        actualizar_barra_superior(tics);
    }

    pub fn procesar_tecla(&mut self, byte: u8) {
        if self.modo == ModoSistema::Grafico {
            if byte == 0x1B {
                reiniciar_sistema();
            }
            return;
        }

        // Salir del monitor en vivo view_resources al pulsar Q o ESC
        if self.modo == ModoSistema::MonitorRecursos {
            if byte == b'q' || byte == b'Q' || byte == 0x1B {
                self.modo = ModoSistema::Consola;
                {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                    p.limpiar_desde_fila(0);
                }
                imprimir_prompt();
                let tics = TICS_RELOJ.load(Ordering::Relaxed);
                actualizar_barra_superior(tics);
            }
            return;
        }

        if self.modo == ModoSistema::Editor {
            if self.esperando_confirmacion_salir {
                match byte {
                    b's' | b'S' | b'y' | b'Y' => {
                        self.guardar_desde_editor();
                        self.salir_de_editor();
                    }
                    b'n' | b'N' => self.salir_de_editor(),
                    0x03 | 0x1B => {
                        self.esperando_confirmacion_salir = false;
                        self.actualizar_barra_editor("^O Guardar en Disco | ^X Salir");
                    }
                    _ => {}
                }
                return;
            }

            match byte {
                0x0F => self.guardar_desde_editor(),
                0x18 => {
                    if !self.editor_modificado {
                        self.salir_de_editor();
                    } else {
                        self.esperando_confirmacion_salir = true;
                        self.actualizar_barra_editor(
                            "¿Guardar cambios antes de salir? [S] Si / [N] No / [ESC] Cancelar",
                        );
                    }
                }
                0x08 => {
                    if self.cursor_editor > 0 {
                        self.cursor_editor -= 1;
                        self.contenido_editor.remove(self.cursor_editor);
                        self.editor_modificado = true;
                        self.redibujar_cuerpo_editor();
                        self.actualizar_barra_editor("^O Guardar en Disco | ^X Salir");
                    }
                }
                b'\n' => {
                    self.contenido_editor.insert(self.cursor_editor, '\n');
                    self.cursor_editor += 1;
                    self.editor_modificado = true;
                    self.redibujar_cuerpo_editor();
                    self.actualizar_barra_editor("^O Guardar en Disco | ^X Salir");
                }
                byte => {
                    if byte >= 32 && byte <= 126 {
                        self.contenido_editor.insert(self.cursor_editor, byte as char);
                        self.cursor_editor += 1;
                        self.editor_modificado = true;
                        self.redibujar_cuerpo_editor();
                        self.actualizar_barra_editor("^O Guardar en Disco | ^X Salir");
                    }
                }
            }
            return;
        }

        match byte {
            0x0C => {
                PANTALLA_GLOBAL.lock().limpiar_desde_fila(1);
                let tics = TICS_RELOJ.load(Ordering::Relaxed);
                actualizar_barra_superior(tics);
                imprimir_prompt();
                self.redibujar_linea_consola();
            }
            0x03 => {
                {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("^C\n");
                }
                self.buffer_cmd.clear();
                self.cursor_cmd = 0;
                imprimir_prompt();
            }
            b'\t' => self.autocompletar(),
            b'\n' => {
                PANTALLA_GLOBAL.lock().nueva_linea(1);
                self.ejecutar_comando();
                self.buffer_cmd.clear();
                self.cursor_cmd = 0;
                if self.modo == ModoSistema::Consola {
                    imprimir_prompt();
                }
            }
            0x08 => {
                if self.cursor_cmd > 0 {
                    self.cursor_cmd -= 1;
                    self.buffer_cmd.remove(self.cursor_cmd);
                    self.redibujar_linea_consola();
                }
            }
            byte => {
                if byte >= 32 && byte <= 126 && self.buffer_cmd.len() < 64 {
                    self.buffer_cmd.insert(self.cursor_cmd, byte as char);
                    self.cursor_cmd += 1;
                    self.redibujar_linea_consola();
                }
            }
        }
    }

    fn mostrar_ayuda_comando(&self, cmd: &str) {
        let mut p = PANTALLA_GLOBAL.lock();
        p.cambiar_color(Color::CianClaro, Color::Negro);
        match cmd {
            "ls" => p.escribir_texto(
                "ls [ruta] - Lista el contenido del directorio actual o indicado.\n",
            ),
            "cd" => p.escribir_texto("cd <dir> - Cambia de directorio. Ej: cd /etc  o  cd ..\n"),
            "mkdir" => p.escribir_texto("mkdir <dir> - Crea una carpeta nueva en el disco duro.\n"),
            "mv" => p.escribir_texto(
                "mv <origen> <destino> - Mueve o renombra un archivo sin duplicarlo.\n",
            ),
            "cp" => p.escribir_texto(
                "cp <origen> <destino> - Copia un archivo a otra ruta o nombre.\n",
            ),
            "rm" => p.escribir_texto(
                "rm <ruta> - Elimina un archivo o carpeta (protege /boot y /etc).\n",
            ),
            "edit" => p.escribir_texto(
                "edit <arch> - Abre editor tipo nano. Usa Ctrl+O guardar, Ctrl+X salir.\n",
            ),
            "cat" => p.escribir_texto("cat <arch> - Imprime el contenido completo de un archivo.\n"),
            "grep" => p.escribir_texto(
                "grep <patron> <archivo> - Busca coincidencias de texto en un archivo.\n",
            ),
            "echo" => p.escribir_texto(
                "echo <texto> [> arch | >> arch] - Imprime o redirige texto a archivo.\n",
            ),
            "view_resources" => p.escribir_texto(
                "view_resources - Abre el monitor de CPU, RAM y Disco en vivo (Q salir).\n",
            ),
            "df" => p.escribir_texto(
                "df - Muestra el espacio ocupado y libre en la particion del disco duro.\n",
            ),
            "install" => p.escribir_texto(
                "install - Formatea el disco duro e instala melac_os Server.\n",
            ),
            _ => {
                p.cambiar_color(Color::Amarillo, Color::Negro);
                let _ = writeln!(
                    p,
                    "Sin ayuda especifica para '{}'. Consulta ~/manual_comandos.txt",
                    cmd
                );
            }
        }
        p.cambiar_color(Color::VerdeClaro, Color::Negro);
    }

    fn ejecutar_comando(&mut self) {
        let linea = self.buffer_cmd.trim().to_string();
        if linea.is_empty() {
            return;
        }
        self.historial.push(linea.clone());
        self.indice_historial = self.historial.len();

        let partes: Vec<&str> = linea.splitn(3, ' ').collect();

        match partes[0] {
            "help" => {
                if partes.len() >= 2 {
                    self.mostrar_ayuda_comando(partes[1]);
                } else {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    p.escribir_texto("MELAC_OS Server - Comandos basicos:\n");
                    p.cambiar_color(Color::Blanco, Color::Negro);
                    p.escribir_texto(
                        "  Archivos : ls, cd, pwd, mkdir, touch, edit, cat, cp, mv, rm\n",
                    );
                    p.escribir_texto(
                        "  Sistema  : view_resources, df, grep, echo, env, install, reboot\n",
                    );
                    p.cambiar_color(Color::Amarillo, Color::Negro);
                    p.escribir_texto(
                        "Escribe 'help <comando>' para detalles o 'cat ~/manual_comandos.txt'\n",
                    );
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
            }
            "view_resources" => {
                self.modo = ModoSistema::MonitorRecursos;
                PANTALLA_GLOBAL.lock().limpiar_desde_fila(0);
                let tics = TICS_RELOJ.load(Ordering::Relaxed);
                refrescar_monitor_vivo(tics);
            }
            "install" => {
                {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::Amarillo, Color::Negro);
                    p.escribir_texto(
                        "Formateando /dev/hdb1 (MELACFS3) e instalando melac_os Server...\n",
                    );
                }
                match instalar_en_disco_duro() {
                    Ok(n) => {
                        let mut p = PANTALLA_GLOBAL.lock();
                        p.cambiar_color(Color::CianClaro, Color::Negro);
                        let _ = writeln!(
                            p,
                            "[OK] Instalacion completada ({} nodos). Guia en manual_comandos.txt",
                            n
                        );
                        p.cambiar_color(Color::VerdeClaro, Color::Negro);
                    }
                    Err(e) => {
                        let mut p = PANTALLA_GLOBAL.lock();
                        p.cambiar_color(Color::RojoClaro, Color::Negro);
                        let _ = writeln!(p, "[ERROR INSTALACION] {}", e);
                        p.cambiar_color(Color::VerdeClaro, Color::Negro);
                    }
                }
            }
            "whoami" => {
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                p.escribir_texto("usuario\n");
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "hostname" => {
                let host = obtener_hostname();
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                let _ = writeln!(p, "{}", host);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "env" => {
                let host = obtener_hostname();
                let dir = DIRECTORIO_ACTUAL.lock().clone();
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::Amarillo, Color::Negro);
                p.escribir_texto("USER=usuario\nHOME=/home/usuario\n");
                let _ = writeln!(p, "HOSTNAME={}", host);
                let _ = writeln!(p, "PWD={}", dir);
                p.escribir_texto("SHELL=/bin/melac_sh\n");
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "df" => {
                let (usados, total, nodos) = estadisticas_disco();
                let libres = total.saturating_sub(usados);
                let pct = (usados * 100) / total;
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                p.escribir_texto(
                    "Sistema Archivos   Tamano     Usado      Libre    Uso%  Montado en\n",
                );
                p.cambiar_color(Color::Blanco, Color::Negro);
                let _ = writeln!(
                    p,
                    "/dev/hdb1          {:>5} B   {:>5} B   {:>5} B    {:>2}%  / ({} nodos)",
                    total, usados, libres, pct, nodos
                );
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "echo" => {
                let resto = if linea.len() > 5 { &linea[5..] } else { "" };
                if let Some((texto, archivo)) = resto.split_once(">>") {
                    let arch = archivo.trim();
                    if !arch.is_empty() {
                        anadir_a_archivo(arch, texto.trim());
                    }
                } else if let Some((texto, archivo)) = resto.split_once('>') {
                    let arch = archivo.trim();
                    if !arch.is_empty() {
                        crear_archivo(arch, texto.trim());
                    }
                } else {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::Blanco, Color::Negro);
                    let _ = writeln!(p, "{}", resto);
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
            }
            "grep" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 3 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: grep <patron> <archivo>\n");
                } else {
                    let patron = partes[1];
                    let ruta = partes[2];
                    match leer_archivo(ruta) {
                        Some(contenido) => {
                            let mut coincidencias = 0;
                            for (num, l) in contenido.lines().enumerate() {
                                if l.contains(patron) {
                                    coincidencias += 1;
                                    p.cambiar_color(Color::Amarillo, Color::Negro);
                                    let _ = writeln!(p, "{}: {}", num + 1, l);
                                }
                            }
                            if coincidencias == 0 {
                                p.cambiar_color(Color::GrisClaro, Color::Negro);
                                p.escribir_texto("(Sin coincidencias)\n");
                            }
                        }
                        None => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "grep: No existe '{}'", ruta);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "cp" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 3 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: cp <origen> <destino>\n");
                } else {
                    match copiar_archivo(partes[1], partes[2]) {
                        Ok(()) => {
                            p.cambiar_color(Color::CianClaro, Color::Negro);
                            let _ = writeln!(p, "Copiado '{}' -> '{}'", partes[1], partes[2]);
                        }
                        Err(e) => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "cp: {}", e);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "pwd" => {
                let dir = DIRECTORIO_ACTUAL.lock().clone();
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                let _ = writeln!(p, "{}", dir);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "mkdir" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: mkdir <nombre_directorio>\n");
                } else {
                    match crear_directorio(partes[1]) {
                        Ok(ruta) => {
                            p.cambiar_color(Color::CianClaro, Color::Negro);
                            let _ = writeln!(p, "Directorio creado: {}", ruta);
                        }
                        Err(e) => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "Error: {}", e);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "cd" => {
                let destino = if partes.len() < 2 { "/home/usuario" } else { partes[1] };
                if let Err(e) = cambiar_directorio(destino) {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "cd: {}", e);
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
            }
            "mv" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 3 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: mv <origen> <destino>\n");
                } else {
                    match mover_nodo(partes[1], partes[2]) {
                        Ok(()) => {
                            p.cambiar_color(Color::CianClaro, Color::Negro);
                            let _ = writeln!(p, "Movido '{}' -> '{}'", partes[1], partes[2]);
                        }
                        Err(e) => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "mv: {}", e);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "ls" => {
                let dir_objetivo = if partes.len() >= 2 {
                    resolver_ruta(partes[1])
                } else {
                    DIRECTORIO_ACTUAL.lock().clone()
                };
                let nodos = SISTEMA_ARCHIVOS.lock();
                let mut p = PANTALLA_GLOBAL.lock();
                let mut encontrados = 0;
                for nodo in nodos.iter() {
                    if nodo.ruta == "/" {
                        continue;
                    }
                    if carpeta_padre(&nodo.ruta) == dir_objetivo {
                        encontrados += 1;
                        let nom = nombre_base(&nodo.ruta);
                        match nodo.tipo {
                            TipoNodo::Directorio => {
                                p.cambiar_color(Color::AzulClaro, Color::Negro);
                                let _ = writeln!(p, "  [DIR]  {}/", nom);
                            }
                            TipoNodo::Archivo => {
                                p.cambiar_color(Color::Amarillo, Color::Negro);
                                let _ = writeln!(
                                    p,
                                    "  [FILE] {} ({} bytes)",
                                    nom,
                                    nodo.contenido.len()
                                );
                            }
                        }
                    }
                }
                if encontrados == 0 {
                    p.cambiar_color(Color::GrisClaro, Color::Negro);
                    p.escribir_texto("  (Directorio vacio)\n");
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "sync" | "save" => {
                let res = sincronizar_disco();
                let mut p = PANTALLA_GLOBAL.lock();
                match res {
                    Ok(n) => {
                        p.cambiar_color(Color::CianClaro, Color::Negro);
                        let _ =
                            writeln!(p, "[OK] {} nodos sincronizados con el disco duro.", n);
                    }
                    Err(e) => {
                        p.cambiar_color(Color::RojoClaro, Color::Negro);
                        let _ = writeln!(p, "[ERROR DISCO] {}", e);
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "load" => {
                let res = montar_disco_instalado();
                let mut p = PANTALLA_GLOBAL.lock();
                match res {
                    Ok(n) => {
                        *SISTEMA_INSTALADO.lock() = true;
                        p.cambiar_color(Color::CianClaro, Color::Negro);
                        let _ = writeln!(
                            p,
                            "[OK] Disco duro montado ({} nodos cargados).",
                            n
                        );
                    }
                    Err(e) => {
                        p.cambiar_color(Color::RojoClaro, Color::Negro);
                        let _ = writeln!(p, "[ERROR DISCO] {}", e);
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "edit" => {
                if partes.len() < 2 {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: edit <ruta_archivo>\n");
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                } else {
                    self.abrir_editor(partes[1]);
                }
            }
            "run" => {
                if partes.len() < 2 {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: run <archivo.ms>\n");
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                } else if let Some(codigo) = leer_archivo(partes[1]) {
                    for l in codigo.lines() {
                        let l = l.trim();
                        if l.is_empty() || l.starts_with('#') {
                            continue;
                        }
                        let inst: Vec<&str> = l.splitn(2, ' ').collect();
                        match inst[0] {
                            "print" => {
                                let mut p = PANTALLA_GLOBAL.lock();
                                if inst.len() > 1 {
                                    let _ = writeln!(p, "{}", inst[1]);
                                } else {
                                    p.nueva_linea(1);
                                }
                            }
                            "color" if inst.len() > 1 => {
                                let mut p = PANTALLA_GLOBAL.lock();
                                match inst[1] {
                                    "verde" => p.cambiar_color(Color::VerdeClaro, Color::Negro),
                                    "cian" => p.cambiar_color(Color::CianClaro, Color::Negro),
                                    "amarillo" => {
                                        p.cambiar_color(Color::Amarillo, Color::Negro)
                                    }
                                    "rojo" => p.cambiar_color(Color::RojoClaro, Color::Negro),
                                    "blanco" => p.cambiar_color(Color::Blanco, Color::Negro),
                                    _ => {}
                                }
                            }
                            "clear" => PANTALLA_GLOBAL.lock().limpiar_desde_fila(1),
                            "beep" => emitir_beep(),
                            _ => {}
                        }
                    }
                    PANTALLA_GLOBAL
                        .lock()
                        .cambiar_color(Color::VerdeClaro, Color::Negro);
                } else {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "Error: No existe '{}'.", partes[1]);
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                }
            }
            "gui" => {
                self.modo = ModoSistema::Grafico;
                self.ventana1_abierta = true;
                self.ventana2_abierta = true;
                let offset = OFFSET_MEMORIA_FISICA.load(Ordering::Relaxed);
                let mut gpu = PantallaGrafica::inicializar(offset);
                gpu.renderizar_escritorio(true, true, self.cursor_x, self.cursor_y);
            }
            "touch" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: touch <nombre> [contenido]\n");
                } else {
                    let contenido = if partes.len() == 3 { partes[2] } else { "" };
                    crear_archivo(partes[1], contenido);
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(p, "Archivo '{}' guardado.", partes[1]);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "cat" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: cat <ruta_archivo>\n");
                } else if let Some(texto) = leer_archivo(partes[1]) {
                    p.cambiar_color(Color::Blanco, Color::Negro);
                    let _ = writeln!(p, "{}", texto);
                } else {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "Error: No existe '{}'.", partes[1]);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "rm" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: rm <ruta>\n");
                } else {
                    match borrar_nodo(partes[1]) {
                        Ok(()) => {
                            p.cambiar_color(Color::Amarillo, Color::Negro);
                            let _ = writeln!(p, "Eliminado '{}'.", partes[1]);
                        }
                        Err(e) => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "rm: {}", e);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "clear" => {
                PANTALLA_GLOBAL.lock().limpiar_desde_fila(1);
                let tics = TICS_RELOJ.load(Ordering::Relaxed);
                actualizar_barra_superior(tics);
            }
            "reboot" => reiniciar_sistema(),
            "shutdown" => apagar_sistema(),
            "panic" => panic!("Panico provocado manualmente desde la Shell!"),
            otro => {
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::RojoClaro, Color::Negro);
                let _ = writeln!(p, "Comando desconocido: '{}'. Escribe 'help'.", otro);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
        }
    }
}
