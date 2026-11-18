# ============================================================
# Patch melac_os Server v3.0 - Añade 15 comandos nuevos
# ============================================================
import os

# ---------- 1. Crear src/rtc.rs (reloj CMOS real) ----------
with open('src/rtc.rs', 'w') as f:
    f.write(r'''use x86_64::instructions::port::Port;

unsafe fn leer_cmos(reg: u8) -> u8 {
    let mut idx: Port<u8> = Port::new(0x70);
    let mut datos: Port<u8> = Port::new(0x71);
    idx.write(reg);
    datos.read()
}

fn bcd_a_binario(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

/// Devuelve (hora, min, seg, dia, mes, anio_2dig) del RTC CMOS.
pub fn leer_fecha_hora() -> (u8, u8, u8, u8, u8, u8) {
    unsafe {
        while leer_cmos(0x0A) & 0x80 != 0 {}
        let reg_b = leer_cmos(0x0B);
        let bcd = (reg_b & 0x04) == 0;
        let es_12h = (reg_b & 0x02) == 0;
        let mut seg = leer_cmos(0x00);
        let mut min = leer_cmos(0x02);
        let mut hora_raw = leer_cmos(0x04);
        let mut dia = leer_cmos(0x07);
        let mut mes = leer_cmos(0x08);
        let mut anio = leer_cmos(0x09);
        let pm = es_12h && (hora_raw & 0x80) != 0;
        hora_raw &= 0x7F;
        if bcd {
            seg = bcd_a_binario(seg);
            min = bcd_a_binario(min);
            hora_raw = bcd_a_binario(hora_raw);
            dia = bcd_a_binario(dia);
            mes = bcd_a_binario(mes);
            anio = bcd_a_binario(anio);
        }
        let mut hora = hora_raw;
        if es_12h {
            if pm && hora < 12 { hora += 12; }
            if !pm && hora == 12 { hora = 0; }
        }
        (hora, min, seg, dia, mes, anio)
    }
}

pub fn dia_semana(dia: u8, mes: u8, anio_2dig: u8) -> &'static str {
    if mes < 1 || mes > 12 { return "?"; }
    let anio = anio_2dig as i32 + 2000;
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if mes < 3 { anio - 1 } else { anio };
    let idx = ((y + y/4 - y/100 + y/400 + t[(mes as usize) - 1] + dia as i32) % 7) as usize;
    ["Domingo", "Lunes", "Martes", "Miercoles", "Jueves", "Viernes", "Sabado"][idx]
}
''')
print('[OK] src/rtc.rs creado')

# ---------- 2. Ampliar src/fs.rs con helpers ----------
with open('src/fs.rs', 'r') as f:
    s = f.read()

if 'pub fn obtener_nodo' not in s:
    s += r'''
// ========= HELPERS PARA COMANDOS AVANZADOS =========
pub fn obtener_nodo(ruta_entrada: &str) -> Option<NodoFS> {
    let ruta = resolver_ruta(ruta_entrada);
    SISTEMA_ARCHIVOS.lock().iter().find(|n| n.ruta == ruta).cloned()
}

pub fn listar_hijos(dir: &str) -> Vec<NodoFS> {
    let dir = resolver_ruta(dir);
    let mut hijos: Vec<NodoFS> = SISTEMA_ARCHIVOS.lock().iter()
        .filter(|n| n.ruta != "/" && carpeta_padre(&n.ruta) == dir)
        .cloned()
        .collect();
    hijos.sort_by(|a, b| match (a.tipo, b.tipo) {
        (TipoNodo::Directorio, TipoNodo::Archivo) => core::cmp::Ordering::Less,
        (TipoNodo::Archivo, TipoNodo::Directorio) => core::cmp::Ordering::Greater,
        _ => a.ruta.cmp(&b.ruta),
    });
    hijos
}

pub fn tamano_recursivo(ruta_entrada: &str) -> usize {
    let ruta = resolver_ruta(ruta_entrada);
    let nodos = SISTEMA_ARCHIVOS.lock();
    let prefijo = if ruta == "/" { String::from("/") } else { format!("{}/", ruta) };
    let mut total = 0;
    for n in nodos.iter() {
        if n.tipo == TipoNodo::Archivo && (n.ruta == ruta || n.ruta.starts_with(&prefijo)) {
            total += n.contenido.len();
        }
    }
    total
}

pub fn buscar_por_nombre(patron: &str) -> Vec<NodoFS> {
    SISTEMA_ARCHIVOS.lock().iter()
        .filter(|n| n.ruta != "/" && nombre_base(&n.ruta).contains(patron))
        .cloned()
        .collect()
}

pub fn buscar_en_archivos(patron: &str) -> Vec<(String, usize, String)> {
    let nodos = SISTEMA_ARCHIVOS.lock();
    let mut resultados = Vec::new();
    for n in nodos.iter() {
        if n.tipo == TipoNodo::Archivo {
            for (num, l) in n.contenido.lines().enumerate() {
                if l.contains(patron) {
                    resultados.push((n.ruta.clone(), num + 1, String::from(l)));
                }
            }
        }
    }
    resultados
}
'''
    with open('src/fs.rs', 'w') as f:
        f.write(s)
    print('[OK] src/fs.rs ampliado')
else:
    print('[SKIP] fs.rs ya tiene helpers')

# ---------- 3. mod rtc en src/main.rs ----------
with open('src/main.rs', 'r') as f:
    s = f.read()
if 'mod rtc;' not in s:
    s = s.replace('mod raton;\nmod shell;', 'mod raton;\nmod rtc;\nmod shell;')
    with open('src/main.rs', 'w') as f:
        f.write(s)
    print('[OK] mod rtc añadido a main.rs')
else:
    print('[SKIP] mod rtc ya presente')

# ---------- 4. src/shell.rs: imports + nuevos comandos ----------
with open('src/shell.rs', 'r') as f:
    s = f.read()

# 4a. Ampliar imports
old = '''use crate::fs::{
    anadir_a_archivo, borrar_nodo, cambiar_directorio, carpeta_padre, copiar_archivo,
    crear_archivo, crear_directorio, estadisticas_disco, instalar_en_disco_duro, leer_archivo,
    memoria_fs_bytes, mover_nodo, nombre_base, resolver_ruta, TipoNodo, DIRECTORIO_ACTUAL,
    KERNEL_LOGS, SISTEMA_ARCHIVOS, SISTEMA_INSTALADO,
};'''
new = '''use crate::fs::{
    anadir_a_archivo, borrar_nodo, buscar_en_archivos, buscar_por_nombre, cambiar_directorio,
    carpeta_padre, copiar_archivo, crear_archivo, crear_directorio, estadisticas_disco,
    instalar_en_disco_duro, leer_archivo, listar_hijos, memoria_fs_bytes, mover_nodo,
    nombre_base, obtener_nodo, resolver_ruta, tamano_recursivo, TipoNodo, DIRECTORIO_ACTUAL,
    KERNEL_LOGS, SISTEMA_ARCHIVOS, SISTEMA_INSTALADO,
};
use crate::rtc;'''
if old in s:
    s = s.replace(old, new)
    print('[OK] imports fs ampliados + rtc')

# 4b. Ampliar autocompletar
s = s.replace(
    '"grep", "echo", "dmesg", "whoami", "hostname", "env", "df", "gui", "clear",\n                "reboot", "shutdown",',
    '"grep", "echo", "dmesg", "whoami", "hostname", "env", "df", "gui", "clear",\n                "reboot", "shutdown", "tree", "find", "stat", "file", "hexdump",\n                "date", "time", "free", "lscpu", "history", "du", "cowsay", "fortune", "nano",'
)

# 4c. Actualizar el help principal (más compacto y completo)
old_help = '''p.escribir_texto("  Archivos : ls, cd, pwd, mkdir, touch, edit, cat, head, tail, wc\\n");
                    p.escribir_texto("             cp, mv, rm, grep, echo\\n");
                    p.escribir_texto("  Sistema  : view_resources, dmesg, df, env, install, reboot\\n");'''
new_help = '''p.escribir_texto("  Archivos : ls, cd, pwd, mkdir, touch, edit, cat, head, tail, wc, cp, mv, rm\\n");
                    p.escribir_texto("             grep [-r], echo, find, tree, stat, file, hexdump, du\\n");
                    p.escribir_texto("  Sistema  : view_resources, dmesg, df, free, lscpu, date, time, env\\n");
                    p.escribir_texto("  Extras   : history [-c], nano, cowsay, fortune, install, reboot, shutdown\\n");'''
if old_help in s:
    s = s.replace(old_help, new_help)
    print('[OK] help actualizado')

# 4d. Mejorar grep con -r
old_grep = '''"grep" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 3 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: grep <texto> <archivo>\\n");
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
                                p.escribir_texto("(Sin coincidencias)\\n");
                            }
                        }
                        None => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "grep: No existe '{}'", ruta);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }'''
new_grep = '''"grep" => {
                let mut p = PANTALLA_GLOBAL.lock();
                let (patron, ruta_opt, recursivo) = if partes.len() >= 2 && partes[1] == "-r" {
                    if partes.len() < 3 {
                        p.cambiar_color(Color::RojoClaro, Color::Negro);
                        p.escribir_texto("Uso: grep -r <texto>\\n");
                        p.cambiar_color(Color::VerdeClaro, Color::Negro);
                        return;
                    }
                    (partes[2], None, true)
                } else if partes.len() >= 3 {
                    (partes[1], Some(partes[2]), false)
                } else {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: grep <texto> <archivo> | grep -r <texto>\\n");
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                    return;
                };
                if recursivo {
                    let resultados = buscar_en_archivos(patron);
                    if resultados.is_empty() {
                        p.cambiar_color(Color::GrisClaro, Color::Negro);
                        p.escribir_texto("(Sin coincidencias)\\n");
                    } else {
                        for (ruta, num, linea) in resultados.iter() {
                            p.cambiar_color(Color::CianClaro, Color::Negro);
                            let _ = write!(p, "{}:", ruta);
                            p.cambiar_color(Color::Amarillo, Color::Negro);
                            let _ = write!(p, "{}:", num);
                            p.cambiar_color(Color::Blanco, Color::Negro);
                            let _ = writeln!(p, "{}", linea);
                        }
                    }
                } else if let Some(ruta) = ruta_opt {
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
                                p.escribir_texto("(Sin coincidencias)\\n");
                            }
                        }
                        None => {
                            p.cambiar_color(Color::RojoClaro, Color::Negro);
                            let _ = writeln!(p, "grep: No existe '{}'", ruta);
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }'''
if old_grep in s:
    s = s.replace(old_grep, new_grep)
    print('[OK] grep -r añadido')

# 4e. Añadir 15 comandos nuevos antes del fallback otro
old_otro = '''otro => {
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::RojoClaro, Color::Negro);
                let _ = writeln!(p, "Comando desconocido: '{}'. Escribe 'help'.", otro);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }'''
new_cmds = r'''"tree" => {
                let ruta = if partes.len() >= 2 { partes[1] } else { "." };
                let base = resolver_ruta(ruta);
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::AzulClaro, Color::Negro);
                let _ = writeln!(p, "{}", base);
                p.cambiar_color(Color::Blanco, Color::Negro);
                let mut contador: usize = 0;
                dibujar_arbol(&mut *p, &base, "", &mut contador);
                p.cambiar_color(Color::Amarillo, Color::Negro);
                let _ = writeln!(p, "\n{} entradas encontradas.", contador);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "find" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: find <nombre_o_parte>\n");
                } else {
                    let encontrados = buscar_por_nombre(partes[1]);
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    if encontrados.is_empty() {
                        p.escribir_texto("(Sin coincidencias)\n");
                    } else {
                        let _ = writeln!(p, "{} resultados:", encontrados.len());
                        for n in encontrados.iter() {
                            if n.tipo == TipoNodo::Directorio {
                                p.cambiar_color(Color::AzulClaro, Color::Negro);
                                let _ = writeln!(p, "  [DIR]  {}", n.ruta);
                            } else {
                                p.cambiar_color(Color::Blanco, Color::Negro);
                                let _ = writeln!(p, "  [FILE] {} ({} B)", n.ruta, n.contenido.len());
                            }
                        }
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "stat" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: stat <ruta>\n");
                } else if let Some(nodo) = obtener_nodo(partes[1]) {
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(p, "  Archivo:  {}", nodo.ruta);
                    let tipo = if nodo.tipo == TipoNodo::Directorio { "directorio" } else { "archivo regular" };
                    let _ = writeln!(p, "  Tipo:     {}", tipo);
                    let tam = if nodo.tipo == TipoNodo::Directorio { tamano_recursivo(&nodo.ruta) } else { nodo.contenido.len() };
                    let _ = writeln!(p, "  Tamano:   {} bytes", tam);
                    let _ = writeln!(p, "  Permisos: rw-r--r--");
                    let _ = writeln!(p, "  Owner:    usuario:usuario");
                } else {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "stat: No existe '{}'", partes[1]);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "file" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: file <ruta>\n");
                } else if let Some(nodo) = obtener_nodo(partes[1]) {
                    let desc = if nodo.tipo == TipoNodo::Directorio {
                        String::from("directory")
                    } else if nodo.contenido.starts_with("ELF") {
                        String::from("ELF 64-bit kernel binary")
                    } else if nodo.contenido.is_empty() {
                        String::from("empty file")
                    } else if nodo.contenido.chars().all(|c| c.is_ascii()) {
                        String::from("ASCII text")
                    } else {
                        String::from("data / binary")
                    };
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(p, "{}: {}", nodo.ruta, desc);
                } else {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "file: No existe '{}'", partes[1]);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "hexdump" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: hexdump <archivo>\n");
                } else if let Some(nodo) = obtener_nodo(partes[1]) {
                    if nodo.tipo == TipoNodo::Directorio {
                        p.cambiar_color(Color::RojoClaro, Color::Negro);
                        p.escribir_texto("hexdump: es un directorio\n");
                    } else {
                        p.cambiar_color(Color::GrisClaro, Color::Negro);
                        let bytes = nodo.contenido.as_bytes();
                        let limite = bytes.len().min(256);
                        for (i, chunk) in bytes[..limite].chunks(16).enumerate() {
                            let offset = i * 16;
                            let mut hex = String::new();
                            let mut ascii = String::new();
                            for (j, b) in chunk.iter().enumerate() {
                                if j == 8 { hex.push(' '); }
                                hex.push_str(&format!("{:02X} ", b));
                                if *b >= 32 && *b < 127 { ascii.push(*b as char); } else { ascii.push('.'); }
                            }
                            let _ = writeln!(p, "{:08X}  {: <48} |{}|", offset, hex, ascii);
                        }
                        if bytes.len() > limite {
                            let _ = writeln!(p, "... ({} bytes totales)", bytes.len());
                        }
                    }
                } else {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    let _ = writeln!(p, "hexdump: No existe '{}'", partes[1]);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "date" => {
                let (h, m, s, dia, mes, anio) = rtc::leer_fecha_hora();
                let ds = rtc::dia_semana(dia, mes, anio);
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                let _ = writeln!(p, "{} {} {} {}  {:02}:{:02}:{:02} UTC", ds, dia, mes, 2000 + anio as u16, h, m, s);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "time" => {
                let (h, m, s, _, _, _) = rtc::leer_fecha_hora();
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                let _ = writeln!(p, "{:02}:{:02}:{:02} UTC", h, m, s);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "free" => {
                let total_kb = TAMANO_HEAP / 1024;
                let fs_kb = memoria_fs_bytes() / 1024;
                let base_kb = 48u64;
                let usado_kb = (base_kb + fs_kb as u64).min(total_kb as u64);
                let libre_kb = (total_kb as u64).saturating_sub(usado_kb);
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                p.escribir_texto("              total        used        free\n");
                p.cambiar_color(Color::Blanco, Color::Negro);
                let _ = writeln!(p, "Mem:     {:>8} KiB {:>9} KiB {:>9} KiB", total_kb, usado_kb, libre_kb);
                let _ = writeln!(p, "Swap:             0 KiB         0 KiB         0 KiB");
                p.cambiar_color(Color::GrisClaro, Color::Negro);
                let _ = writeln!(p, "Heap dinamico: {} KiB | FS/RAM: {} KiB", total_kb, fs_kb);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "lscpu" => {
                let vendor_str = unsafe {
                    let base = core::arch::x86_64::__cpuid(0);
                    let mut v = [0u8; 12];
                    v[0..4].copy_from_slice(&base.ebx.to_le_bytes());
                    v[4..8].copy_from_slice(&base.edx.to_le_bytes());
                    v[8..12].copy_from_slice(&base.ecx.to_le_bytes());
                    let s: String = v.iter().map(|&b| b as char).collect();
                    s
                };
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::CianClaro, Color::Negro);
                let _ = writeln!(p, "Architecture:        x86_64");
                let _ = writeln!(p, "CPU op-mode(s):      32-bit, 64-bit");
                let _ = writeln!(p, "Byte Order:          Little Endian");
                let _ = writeln!(p, "CPU(s):              1");
                let _ = writeln!(p, "Vendor ID:           {}", vendor_str);
                let _ = writeln!(p, "Model name:          melac_os Virtual CPU (QEMU/KVM)");
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "history" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() >= 2 && partes[1] == "-c" {
                    self.historial.clear();
                    self.indice_historial = 0;
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    p.escribir_texto("Historial limpiado.\n");
                } else if self.historial.is_empty() {
                    p.cambiar_color(Color::GrisClaro, Color::Negro);
                    p.escribir_texto("(Sin historial todavia)\n");
                } else {
                    p.cambiar_color(Color::Blanco, Color::Negro);
                    for (i, h) in self.historial.iter().enumerate() {
                        let _ = writeln!(p, "  {:>3}  {}", i + 1, h);
                    }
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "du" => {
                let mut p = PANTALLA_GLOBAL.lock();
                if partes.len() < 2 {
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: du [-sh] <ruta>\n");
                } else {
                    let arg = partes[1];
                    let arg_ruta = if arg == "-sh" || arg == "-s" || arg == "-h" {
                        if partes.len() >= 3 { partes[2] } else { "." }
                    } else { arg };
                    let tam = tamano_recursivo(arg_ruta);
                    p.cambiar_color(Color::CianClaro, Color::Negro);
                    let _ = writeln!(p, "{}  {}", tam, arg_ruta);
                }
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "nano" => {
                if partes.len() < 2 {
                    let mut p = PANTALLA_GLOBAL.lock();
                    p.cambiar_color(Color::RojoClaro, Color::Negro);
                    p.escribir_texto("Uso: nano <archivo>\n");
                    p.cambiar_color(Color::VerdeClaro, Color::Negro);
                } else {
                    self.abrir_editor(partes[1]);
                }
            }
            "cowsay" => {
                let texto = if partes.len() >= 2 {
                    let mut s = String::new();
                    for (i, part) in partes[1..].iter().enumerate() {
                        if i > 0 { s.push(' '); }
                        s.push_str(*part);
                    }
                    s
                } else {
                    String::from("Hola desde melac_os!")
                };
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::Amarillo, Color::Negro);
                let _ = writeln!(p, " ______________________");
                let _ = writeln!(p, "< {} >", texto);
                let _ = writeln!(p, " ----------------------");
                let _ = writeln!(p, "        \\   ^__^");
                let _ = writeln!(p, "         \\  (oo)\\_______");
                let _ = writeln!(p, "            (__)\\       )\\/\\");
                let _ = writeln!(p, "                ||----w |");
                let _ = writeln!(p, "                ||     ||");
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            "fortune" => {
                let tics = TICS_RELOJ.load(Ordering::Relaxed);
                let frases = [
                    "Un kernel sin panico es como un dia sin sol.",
                    "El mejor sistema operativo es el que construyes tu mismo.",
                    "Unsafe es un contrato: cumplirlo es tu responsabilidad.",
                    "Los bugs son solo features que aun no entendemos.",
                    "Recuerda: Ctrl+L limpia la pantalla, Ctrl+C cancela.",
                    "El Heap esta vivo mientras el Mutex lo proteja.",
                ];
                let idx = (tics as usize) % frases.len();
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::Magenta, Color::Negro);
                let _ = writeln!(p, "\"{}\"", frases[idx]);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }
            otro => {
                let mut p = PANTALLA_GLOBAL.lock();
                p.cambiar_color(Color::RojoClaro, Color::Negro);
                let _ = writeln!(p, "Comando desconocido: '{}'. Escribe 'help'.", otro);
                p.cambiar_color(Color::VerdeClaro, Color::Negro);
            }'''
if old_otro in s:
    s = s.replace(old_otro, new_cmds)
    print('[OK] 15 comandos nuevos añadidos a shell.rs')
else:
    print('[ERROR] no se encontró el fallback otro => en shell.rs')

# 4f. Añadir dibujar_arbol al final del archivo
if 'fn dibujar_arbol' not in s:
    s += r'''
// ========= HELPER: arbol recursivo para el comando tree =========
fn dibujar_arbol(
    p: &mut crate::vga_buffer::EscritorVGA,
    base: &str,
    prefijo: &str,
    contador: &mut usize,
) {
    let hijos = listar_hijos(base);
    let n = hijos.len();
    for (i, nodo) in hijos.iter().enumerate() {
        let ultimo = i == n - 1;
        let conector = if ultimo { "└── " } else { "├── " };
        let nombre = nombre_base(&nodo.ruta);
        match nodo.tipo {
            TipoNodo::Directorio => {
                p.cambiar_color(Color::AzulClaro, Color::Negro);
                let _ = writeln!(p, "{}{}{}/", prefijo, conector, nombre);
                let extension = if ultimo { "    " } else { "│   " };
                let nuevo_prefijo = format!("{}{}", prefijo, extension);
                dibujar_arbol(p, &nodo.ruta, &nuevo_prefijo, contador);
            }
            TipoNodo::Archivo => {
                p.cambiar_color(Color::Blanco, Color::Negro);
                let _ = writeln!(p, "{}{}{}", prefijo, conector, nombre);
            }
        }
        *contador += 1;
    }
}
'''
    print('[OK] dibujar_arbol añadido')

with open('src/shell.rs', 'w') as f:
    f.write(s)

print('\n[OK] Patch v3.0 aplicado completo.')
print('Ejecuta: ./arrancar.sh')
