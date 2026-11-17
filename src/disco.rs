use crate::fs::{NodoFS, TipoNodo, SISTEMA_ARCHIVOS};
use alloc::string::String;
use alloc::vec::Vec;
use x86_64::instructions::port::Port;

const ATA_DATA: u16 = 0x1F0;
const ATA_SECTOR_COUNT: u16 = 0x1F2;
const ATA_LBA_LOW: u16 = 0x1F3;
const ATA_LBA_MID: u16 = 0x1F4;
const ATA_LBA_HIGH: u16 = 0x1F5;
const ATA_DRIVE_HEAD: u16 = 0x1F6;
const ATA_STATUS_CMD: u16 = 0x1F7;
const ATA_CONTROL: u16 = 0x3F6;

const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;

// Guardamos la partición raíz '/' en el Disco Duro Real (Esclavo 0xF0 -> disco_instalado.img)
const SECTOR_INICIO_FS: u32 = 1;
const CANTIDAD_SECTORES_FS: u32 = 64; // 64 * 512 = 32 KiB en disco duro

pub fn inicializar_bus_ata() {
    unsafe {
        let mut ctrl: Port<u8> = Port::new(ATA_CONTROL);
        ctrl.write(0x02);
    }
}

unsafe fn esperar_bsy_libre() -> Result<(), &'static str> {
    let mut estado_port: Port<u8> = Port::new(ATA_STATUS_CMD);
    for _ in 0..4 {
        let _ = estado_port.read();
    }
    let mut timeout = 1_000_000;
    while timeout > 0 {
        let st = estado_port.read();
        if st == 0xFF || st == 0x00 {
            return Err("No hay disco duro (disco_instalado.img) conectado");
        }
        if (st & 0x80) == 0 {
            return Ok(());
        }
        timeout -= 1;
    }
    Err("Timeout esperando BSY=0 en disco")
}

unsafe fn esperar_drq_listo() -> Result<(), &'static str> {
    let mut estado_port: Port<u8> = Port::new(ATA_STATUS_CMD);
    let mut timeout = 1_000_000;
    while timeout > 0 {
        let st = estado_port.read();
        if (st & 0x01) != 0 {
            return Err("Error fisico en sector ATA");
        }
        if (st & 0x80) == 0 && (st & 0x08) != 0 {
            return Ok(());
        }
        timeout -= 1;
    }
    Err("Timeout esperando DRQ=1 en disco")
}

pub fn leer_sector(lba: u32, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
    unsafe {
        let mut drive_head: Port<u8> = Port::new(ATA_DRIVE_HEAD);
        let mut sector_count: Port<u8> = Port::new(ATA_SECTOR_COUNT);
        let mut lba_low: Port<u8> = Port::new(ATA_LBA_LOW);
        let mut lba_mid: Port<u8> = Port::new(ATA_LBA_MID);
        let mut lba_high: Port<u8> = Port::new(ATA_LBA_HIGH);
        let mut comando: Port<u8> = Port::new(ATA_STATUS_CMD);
        let mut datos: Port<u16> = Port::new(ATA_DATA);

        // Seleccionamos Disco Esclavo (0xF0 = disco_instalado.img)
        drive_head.write(0xF0 | ((lba >> 24) & 0x0F) as u8);
        esperar_bsy_libre()?;

        sector_count.write(1);
        lba_low.write((lba & 0xFF) as u8);
        lba_mid.write(((lba >> 8) & 0xFF) as u8);
        lba_high.write(((lba >> 16) & 0xFF) as u8);
        comando.write(ATA_CMD_READ_PIO);

        esperar_drq_listo()?;

        for i in 0..256 {
            let palabra = datos.read();
            buffer[i * 2] = (palabra & 0xFF) as u8;
            buffer[i * 2 + 1] = (palabra >> 8) as u8;
        }
    }
    Ok(())
}

pub fn escribir_sector(lba: u32, buffer: &[u8; 512]) -> Result<(), &'static str> {
    unsafe {
        let mut drive_head: Port<u8> = Port::new(ATA_DRIVE_HEAD);
        let mut sector_count: Port<u8> = Port::new(ATA_SECTOR_COUNT);
        let mut lba_low: Port<u8> = Port::new(ATA_LBA_LOW);
        let mut lba_mid: Port<u8> = Port::new(ATA_LBA_MID);
        let mut lba_high: Port<u8> = Port::new(ATA_LBA_HIGH);
        let mut comando: Port<u8> = Port::new(ATA_STATUS_CMD);
        let mut datos: Port<u16> = Port::new(ATA_DATA);

        // Seleccionamos Disco Esclavo (0xF0 = disco_instalado.img)
        drive_head.write(0xF0 | ((lba >> 24) & 0x0F) as u8);
        esperar_bsy_libre()?;

        sector_count.write(1);
        lba_low.write((lba & 0xFF) as u8);
        lba_mid.write(((lba >> 8) & 0xFF) as u8);
        lba_high.write(((lba >> 16) & 0xFF) as u8);
        comando.write(ATA_CMD_WRITE_PIO);

        esperar_drq_listo()?;

        for i in 0..256 {
            let palabra = (buffer[i * 2] as u16) | ((buffer[i * 2 + 1] as u16) << 8);
            datos.write(palabra);
        }

        comando.write(ATA_CMD_CACHE_FLUSH);
        esperar_bsy_libre()?;
    }
    Ok(())
}

// Sincroniza automáticamente todo el sistema de archivos al disco duro real
pub fn sincronizar_disco() -> Result<usize, &'static str> {
    let nodos = SISTEMA_ARCHIVOS.lock();
    let mut datos_serializados: Vec<u8> = Vec::new();

    datos_serializados.extend_from_slice(b"MELACFS3");
    let cantidad = nodos.len().min(255) as u8;
    datos_serializados.push(cantidad);

    for nodo in nodos.iter() {
        let tipo_byte = if nodo.tipo == TipoNodo::Directorio { 1u8 } else { 0u8 };
        let ruta_b = nodo.ruta.as_bytes();
        let cont_b = nodo.contenido.as_bytes();

        if datos_serializados.len() + 4 + ruta_b.len() + cont_b.len() > (CANTIDAD_SECTORES_FS as usize * 512) {
            break;
        }

        datos_serializados.push(tipo_byte);
        datos_serializados.push(ruta_b.len() as u8);
        datos_serializados.extend_from_slice(ruta_b);

        let len_cont = cont_b.len() as u16;
        datos_serializados.push((len_cont & 0xFF) as u8);
        datos_serializados.push((len_cont >> 8) as u8);
        datos_serializados.extend_from_slice(cont_b);
    }

    datos_serializados.resize(CANTIDAD_SECTORES_FS as usize * 512, 0);

    for s in 0..CANTIDAD_SECTORES_FS {
        let mut sector = [0u8; 512];
        let inicio = (s as usize) * 512;
        sector.copy_from_slice(&datos_serializados[inicio..inicio + 512]);
        escribir_sector(SECTOR_INICIO_FS + s, &sector)?;
    }

    Ok(cantidad as usize)
}

pub fn montar_disco_instalado() -> Result<usize, &'static str> {
    let mut datos_crudos: Vec<u8> = Vec::new();

    // Leemos primero el sector 1 para comprobar si está instalado
    let mut primer_sector = [0u8; 512];
    leer_sector(SECTOR_INICIO_FS, &mut primer_sector)?;
    if &primer_sector[0..8] != b"MELACFS3" {
        return Err("Disco duro virgen: ejecuta 'install' para instalar melac_os");
    }
    datos_crudos.extend_from_slice(&primer_sector);

    for s in 1..CANTIDAD_SECTORES_FS {
        let mut sector = [0u8; 512];
        leer_sector(SECTOR_INICIO_FS + s, &mut sector)?;
        datos_crudos.extend_from_slice(&sector);
    }

    let cantidad = datos_crudos[8] as usize;
    let mut pos = 9;
    let mut nuevos_nodos: Vec<NodoFS> = Vec::new();

    for _ in 0..cantidad {
        if pos + 4 > datos_crudos.len() {
            break;
        }
        let tipo = if datos_crudos[pos] == 1 { TipoNodo::Directorio } else { TipoNodo::Archivo };
        pos += 1;

        let len_ruta = datos_crudos[pos] as usize;
        pos += 1;
        if pos + len_ruta + 2 > datos_crudos.len() {
            break;
        }
        let ruta = String::from(core::str::from_utf8(&datos_crudos[pos..pos + len_ruta]).unwrap_or(""));
        pos += len_ruta;

        let len_cont = (datos_crudos[pos] as usize) | ((datos_crudos[pos + 1] as usize) << 8);
        pos += 2;
        if pos + len_cont > datos_crudos.len() {
            break;
        }
        let contenido = String::from(core::str::from_utf8(&datos_crudos[pos..pos + len_cont]).unwrap_or(""));
        pos += len_cont;

        if !ruta.is_empty() {
            nuevos_nodos.push(NodoFS { ruta, tipo, contenido });
        }
    }

    let cargados = nuevos_nodos.len();
    *SISTEMA_ARCHIVOS.lock() = nuevos_nodos;
    Ok(cargados)
}
