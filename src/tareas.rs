use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EstadoTarea {
    Ejecutando,
    Esperando,
}

pub struct TareaKernel {
    pub pid: u32,
    pub nombre: String,
    pub estado: EstadoTarea,
    pub ram_kb: usize,
    pub tics_cpu: u64,
}

// Tabla dinámica global de tareas/procesos del Kernel
pub static PLANIFICADOR: Mutex<Vec<TareaKernel>> = Mutex::new(Vec::new());

// Inicializa los procesos base del sistema operativo
pub fn inicializar_tareas() {
    let mut tareas = PLANIFICADOR.lock();
    tareas.clear();

    tareas.push(TareaKernel {
        pid: 1,
        nombre: String::from("melac_kernel::core"),
        estado: EstadoTarea::Ejecutando,
        ram_kb: 24,
        tics_cpu: 10,
    });
    tareas.push(TareaKernel {
        pid: 2,
        nombre: String::from("drivers::vga_tty"),
        estado: EstadoTarea::Ejecutando,
        ram_kb: 16,
        tics_cpu: 5,
    });
    tareas.push(TareaKernel {
        pid: 3,
        nombre: String::from("drivers::ps2_keyboard"),
        estado: EstadoTarea::Esperando,
        ram_kb: 8,
        tics_cpu: 2,
    });
    tareas.push(TareaKernel {
        pid: 4,
        nombre: String::from("fs::ramfs_daemon"),
        estado: EstadoTarea::Esperando,
        ram_kb: 32,
        tics_cpu: 1,
    });
    tareas.push(TareaKernel {
        pid: 5,
        nombre: String::from("sys::clock_scheduler"),
        estado: EstadoTarea::Ejecutando,
        ram_kb: 4,
        tics_cpu: 1,
    });
}

// Esta función es llamada en segundo plano por el reloj del hardware para repartir CPU
pub fn tic_planificador(tics_totales: u64) {
    // Usamos try_lock() para nunca bloquear una interrupción si el usuario está mirando htop
    if let Some(mut tareas) = PLANIFICADOR.try_lock() {
        for tarea in tareas.iter_mut() {
            if tarea.estado == EstadoTarea::Ejecutando {
                tarea.tics_cpu += 1;
            }
        }
        // Cada 18 tics (~1 segundo), el demonio RamFS hace una comprobación rápida en segundo plano
        if tics_totales % 18 == 0 {
            if let Some(ramfs) = tareas.iter_mut().find(|t| t.pid == 4) {
                ramfs.tics_cpu += 1;
            }
        }
    }
}
