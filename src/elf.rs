use core::mem;
#[allow(dead_code)]

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[allow(dead_code)]
pub fn cargar_y_ejecutar(datos: &[u8]) -> Result<(), &'static str> {
    if datos.len() < mem::size_of::<Elf64Ehdr>() {
        return Err("Archivo demasiado pequeño para ser ELF");
    }

    let cabecera: &Elf64Ehdr = unsafe { &*(datos.as_ptr() as *const Elf64Ehdr) };

    // Verificar la firma mágica (0x7F 'E' 'L' 'F')
    if cabecera.e_ident[0..4] != [0x7F, b'E', b'L', b'F'] {
        return Err("Firma magica ELF no valida");
    }
    if cabecera.e_ident[4] != 2 {
        return Err("No es un binario ELF de 64 bits");
    }

    // Leer los Program Headers
    let ph_offset = cabecera.e_phoff as usize;
    let ph_num = cabecera.e_phnum as usize;

    for i in 0..ph_num {
        let offset_actual = ph_offset + i * mem::size_of::<Elf64Phdr>();
        if offset_actual + mem::size_of::<Elf64Phdr>() > datos.len() {
            return Err("Program header fuera de los limites");
        }
        
        let phdr: &Elf64Phdr = unsafe { &*(datos.as_ptr().add(offset_actual) as *const Elf64Phdr) };

        // PT_LOAD = 1 (Segmento que debe ser cargado en memoria)
        if phdr.p_type == 1 {
            let destino_fisico = phdr.p_paddr as *mut u8;
            let origen = unsafe { datos.as_ptr().add(phdr.p_offset as usize) };
            
            unsafe {
                // Copiamos el segmento del archivo a la dirección de memoria física
                core::ptr::copy_nonoverlapping(origen, destino_fisico, phdr.p_filesz as usize);
                
                // Si el tamaño en memoria es mayor (BSS), lo llenamos con ceros
                if phdr.p_memsz > phdr.p_filesz {
                    let bss_dest = destino_fisico.add(phdr.p_filesz as usize);
                    core::ptr::write_bytes(bss_dest, 0, (phdr.p_memsz - phdr.p_filesz) as usize);
                }
            }
        }
    }

    // ¡SALTO AL CÓDIGO!
    unsafe {
        let punto_entrada: extern "C" fn() -> ! = mem::transmute(cabecera.e_entry as usize);
        punto_entrada();
    }
}
