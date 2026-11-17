use crate::disco::sincronizar_disco;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TipoNodo {
    Archivo,
    Directorio,
}

#[derive(Clone)]
pub struct NodoFS {
    pub ruta: String,
    pub tipo: TipoNodo,
    pub contenido: String,
}

pub static SISTEMA_ARCHIVOS: Mutex<Vec<NodoFS>> = Mutex::new(Vec::new());
pub static DIRECTORIO_ACTUAL: Mutex<String> = Mutex::new(String::new());
pub static SISTEMA_INSTALADO: Mutex<bool> = Mutex::new(false);

pub fn inicializar_ruta_base() {
    *DIRECTORIO_ACTUAL.lock() = String::from("/");
}

pub fn instalar_en_disco_duro() -> Result<usize, &'static str> {
    {
        let mut nodos = SISTEMA_ARCHIVOS.lock();
        nodos.clear();
        nodos.push(NodoFS {
            ruta: String::from("/"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
        nodos.push(NodoFS {
            ruta: String::from("/boot"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
        nodos.push(NodoFS {
            ruta: String::from("/bin"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
        nodos.push(NodoFS {
            ruta: String::from("/etc"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
        nodos.push(NodoFS {
            ruta: String::from("/home"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
        nodos.push(NodoFS {
            ruta: String::from("/home/usuario"),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });

        let bytes_reales_kernel = unsafe { core::slice::from_raw_parts(0x200000 as *const u8, 64) };
        let mut volcado_hex = String::from("ELF64_KERNEL_HDR: ");
        for b in bytes_reales_kernel.iter() {
            volcado_hex.push_str(&format!("{:02X} ", b));
        }

        nodos.push(NodoFS {
            ruta: String::from("/boot/melac_kernel.bin"),
            tipo: TipoNodo::Archivo,
            contenido: volcado_hex,
        });
        nodos.push(NodoFS {
            ruta: String::from("/etc/hostname"),
            tipo: TipoNodo::Archivo,
            contenido: String::from("melac-server"),
        });
        nodos.push(NodoFS {
            ruta: String::from("/etc/fstab"),
            tipo: TipoNodo::Archivo,
            contenido: String::from("/dev/hdb1  /  melacfs3  rw,sync  0 1"),
        });
        nodos.push(NodoFS {
            ruta: String::from("/home/usuario/bienvenida.txt"),
            tipo: TipoNodo::Archivo,
            contenido: String::from(
                "Bienvenido a melac_os Server v1.9! Lee manual_comandos.txt para ver toda la guia.",
            ),
        });

        let manual = concat!(
            "=== MANUAL OFICIAL DE COMANDOS MELAC_OS SERVER ===\n",
            "1. ARCHIVOS Y CARPETAS:\n",
            "   ls [ruta]           - Lista archivos y carpetas\n",
            "   cd <ruta>           - Cambia de directorio (ej: cd .., cd /etc)\n",
            "   pwd                 - Muestra la ruta actual\n",
            "   mkdir <dir>         - Crea una carpeta nueva\n",
            "   touch <arch> [txt]  - Crea un archivo rapido\n",
            "   edit <arch>         - Editor estilo nano (^O Guardar, ^X Salir)\n",
            "   cat <arch>          - Muestra el contenido de un archivo\n",
            "   cp <orig> <dest>    - Copia un archivo\n",
            "   mv <orig> <dest>    - Mueve o renombra un archivo\n",
            "   rm <ruta>           - Elimina un archivo o carpeta\n",
            "   grep <txt> <arch>   - Busca texto dentro de un archivo\n",
            "   echo <txt> > <arch> - Escribe o anade (>>) texto a un archivo\n",
            "2. SISTEMA Y MONITORIZACION:\n",
            "   view_resources      - Monitor en tiempo real (CPU, RAM, Disco). Pulsa Q para salir\n",
            "   df                  - Muestra espacio usado y libre en disco\n",
            "   whoami / hostname / env / uptime\n",
            "   install / sync      - Instala o sincroniza el disco duro\n",
            "   clear (^L) / reboot / shutdown / gui\n"
        );

        nodos.push(NodoFS {
            ruta: String::from("/home/usuario/manual_comandos.txt"),
            tipo: TipoNodo::Archivo,
            contenido: String::from(manual),
        });
    }

    *SISTEMA_INSTALADO.lock() = true;
    *DIRECTORIO_ACTUAL.lock() = String::from("/home/usuario");
    sincronizar_disco()
}

fn auto_guardar() {
    if *SISTEMA_INSTALADO.lock() {
        let _ = sincronizar_disco();
    }
}

pub fn estadisticas_disco() -> (usize, usize, usize) {
    let nodos = SISTEMA_ARCHIVOS.lock();
    let total_bytes = 64 * 512;
    let mut usados = 9;
    for n in nodos.iter() {
        usados += 4 + n.ruta.len() + n.contenido.len();
    }
    (usados, total_bytes, nodos.len())
}

// Calcula la memoria Heap real ocupada por el sistema de archivos y estructuras
pub fn memoria_fs_bytes() -> usize {
    let nodos = SISTEMA_ARCHIVOS.lock();
    let mut total = nodos.capacity() * core::mem::size_of::<NodoFS>();
    for n in nodos.iter() {
        total += n.ruta.capacity() + n.contenido.capacity();
    }
    total
}

pub fn resolver_ruta(entrada: &str) -> String {
    let actual = DIRECTORIO_ACTUAL.lock().clone();
    let base = if entrada.starts_with('/') {
        String::from(entrada)
    } else if actual == "/" {
        format!("/{}", entrada)
    } else {
        format!("{}/{}", actual, entrada)
    };

    let mut partes_limpias: Vec<&str> = Vec::new();
    for parte in base.split('/') {
        match parte {
            "" | "." => {}
            ".." => {
                partes_limpias.pop();
            }
            p => partes_limpias.push(p),
        }
    }

    if partes_limpias.is_empty() {
        String::from("/")
    } else {
        format!("/{}", partes_limpias.join("/"))
    }
}

pub fn nombre_base(ruta: &str) -> &str {
    if ruta == "/" {
        return "/";
    }
    ruta.rsplit('/').next().unwrap_or(ruta)
}

pub fn carpeta_padre(ruta: &str) -> String {
    if ruta == "/" {
        return String::from("/");
    }
    match ruta.rfind('/') {
        Some(0) | None => String::from("/"),
        Some(idx) => String::from(&ruta[..idx]),
    }
}

pub fn crear_directorio(ruta_entrada: &str) -> Result<String, &'static str> {
    let ruta = resolver_ruta(ruta_entrada);
    {
        let mut nodos = SISTEMA_ARCHIVOS.lock();
        if nodos.iter().any(|n| n.ruta == ruta) {
            return Err("Ya existe un archivo o directorio con ese nombre");
        }
        let padre = carpeta_padre(&ruta);
        if !nodos.iter().any(|n| n.ruta == padre && n.tipo == TipoNodo::Directorio) {
            return Err("El directorio padre no existe");
        }
        nodos.push(NodoFS {
            ruta: ruta.clone(),
            tipo: TipoNodo::Directorio,
            contenido: String::new(),
        });
    }
    auto_guardar();
    Ok(ruta)
}

pub fn cambiar_directorio(ruta_entrada: &str) -> Result<String, &'static str> {
    let ruta = resolver_ruta(ruta_entrada);
    let nodos = SISTEMA_ARCHIVOS.lock();

    if nodos.iter().any(|n| n.ruta == ruta && n.tipo == TipoNodo::Directorio) {
        *DIRECTORIO_ACTUAL.lock() = ruta.clone();
        Ok(ruta)
    } else {
        Err("Directorio no encontrado")
    }
}

pub fn crear_archivo(ruta_entrada: &str, contenido: &str) {
    let ruta = resolver_ruta(ruta_entrada);
    {
        let mut nodos = SISTEMA_ARCHIVOS.lock();
        // Eliminamos duplicados previos si existieran y actualizamos limpio
        let mut actualizado = false;
        for nodo in nodos.iter_mut() {
            if nodo.ruta == ruta && nodo.tipo == TipoNodo::Archivo {
                nodo.contenido = contenido.to_string();
                actualizado = true;
                break;
            }
        }
        if !actualizado {
            nodos.push(NodoFS {
                ruta,
                tipo: TipoNodo::Archivo,
                contenido: contenido.to_string(),
            });
        }
    }
    auto_guardar();
}

pub fn anadir_a_archivo(ruta_entrada: &str, texto_extra: &str) {
    let mut actual = leer_archivo(ruta_entrada).unwrap_or_else(String::new);
    if !actual.is_empty() && !actual.ends_with('\n') {
        actual.push('\n');
    }
    actual.push_str(texto_extra);
    crear_archivo(ruta_entrada, &actual);
}

pub fn leer_archivo(ruta_entrada: &str) -> Option<String> {
    let ruta = resolver_ruta(ruta_entrada);
    let nodos = SISTEMA_ARCHIVOS.lock();
    for nodo in nodos.iter() {
        if nodo.ruta == ruta && nodo.tipo == TipoNodo::Archivo {
            return Some(nodo.contenido.clone());
        }
    }
    None
}

pub fn copiar_archivo(origen_in: &str, destino_in: &str) -> Result<(), &'static str> {
    let origen = resolver_ruta(origen_in);
    let mut destino = resolver_ruta(destino_in);

    if origen == destino {
        return Err("El origen y el destino son el mismo archivo");
    }

    let contenido = leer_archivo(&origen).ok_or("El archivo de origen no existe")?;

    {
        let nodos = SISTEMA_ARCHIVOS.lock();
        if nodos.iter().any(|n| n.ruta == destino && n.tipo == TipoNodo::Directorio) {
            let nom = nombre_base(&origen);
            destino = if destino == "/" {
                format!("/{}", nom)
            } else {
                format!("{}/{}", destino, nom)
            };
        }
    }

    if origen == destino {
        return Err("El archivo ya se encuentra en ese directorio");
    }

    crear_archivo(&destino, &contenido);
    Ok(())
}

pub fn mover_nodo(origen_in: &str, destino_in: &str) -> Result<(), &'static str> {
    let origen = resolver_ruta(origen_in);
    let mut destino = resolver_ruta(destino_in);

    if origen == "/" || origen == "/boot" || origen == "/bin" || origen == "/etc" {
        return Err("No puedes mover directorios criticos del sistema");
    }

    {
        let mut nodos = SISTEMA_ARCHIVOS.lock();
        if nodos.iter().any(|n| n.ruta == destino && n.tipo == TipoNodo::Directorio) {
            let nom = nombre_base(&origen);
            destino = if destino == "/" {
                format!("/{}", nom)
            } else {
                format!("{}/{}", destino, nom)
            };
        }
        if origen == destino {
            return Err("El origen y el destino son iguales");
        }
        // Si ya existía un archivo en el destino, lo eliminamos antes para no crear duplicados
        if let Some(pos_dup) = nodos
            .iter()
            .position(|n| n.ruta == destino && n.tipo == TipoNodo::Archivo)
        {
            nodos.remove(pos_dup);
        }
        if let Some(pos) = nodos.iter().position(|n| n.ruta == origen) {
            nodos[pos].ruta = destino;
        } else {
            return Err("El archivo o ruta de origen no existe");
        }
    }
    auto_guardar();
    Ok(())
}

pub fn borrar_nodo(ruta_entrada: &str) -> Result<(), &'static str> {
    let ruta = resolver_ruta(ruta_entrada);
    if ruta == "/" || ruta == "/boot" || ruta == "/bin" || ruta == "/etc" || ruta == "/home" {
        return Err("No se pueden borrar directorios protegidos del sistema");
    }
    {
        let mut nodos = SISTEMA_ARCHIVOS.lock();
        if let Some(pos) = nodos.iter().position(|n| n.ruta == ruta) {
            nodos.remove(pos);
        } else {
            return Err("No existe el archivo o directorio");
        }
    }
    auto_guardar();
    Ok(())
}
