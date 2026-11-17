use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

pub struct Archivo {
    pub nombre: String,
    pub contenido: String,
}

// Lista dinámica global de archivos almacenados en el Heap de melac_os
pub static SISTEMA_ARCHIVOS: Mutex<Vec<Archivo>> = Mutex::new(Vec::new());

// Crea o sobrescribe un archivo en memoria RAM
pub fn crear_archivo(nombre: &str, contenido: &str) {
    let mut archivos = SISTEMA_ARCHIVOS.lock();
    
    // Si ya existe un archivo con ese nombre, actualizamos su contenido
    for archivo in archivos.iter_mut() {
        if archivo.nombre == nombre {
            archivo.contenido = contenido.to_string();
            return;
        }
    }

    // Si es nuevo, lo añadimos al Vector en el Heap
    archivos.push(Archivo {
        nombre: nombre.to_string(),
        contenido: contenido.to_string(),
    });
}

// Busca un archivo por nombre y devuelve una copia de su contenido
pub fn leer_archivo(nombre: &str) -> Option<String> {
    let archivos = SISTEMA_ARCHIVOS.lock();
    for archivo in archivos.iter() {
        if archivo.nombre == nombre {
            return Some(archivo.contenido.clone());
        }
    }
    None
}

// Elimina un archivo y libera su memoria en el Heap
pub fn borrar_archivo(nombre: &str) -> bool {
    let mut archivos = SISTEMA_ARCHIVOS.lock();
    if let Some(pos) = archivos.iter().position(|a| a.nombre == nombre) {
        archivos.remove(pos);
        return true;
    }
    false
}
