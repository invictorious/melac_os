use linked_list_allocator::LockedHeap;

// Reservamos 100 KiB (102,400 bytes) de memoria RAM para asignaciones dinámicas
pub const TAMANO_HEAP: usize = 100 * 1024;

// Creamos el bloque físico alineado a 16 bytes donde vivirá el Heap
#[repr(align(16))]
struct EspacioHeap([u8; TAMANO_HEAP]);

static mut MEMORIA_HEAP: EspacioHeap = EspacioHeap([0; TAMANO_HEAP]);

// Le decimos a Rust que este es el repartidor oficial de memoria de todo el kernel
#[global_allocator]
static ASIGNADOR_GLOBAL: LockedHeap = LockedHeap::empty();

#[allow(static_mut_refs)]
pub fn inicializar_heap() {
    unsafe {
        let inicio_heap = MEMORIA_HEAP.0.as_mut_ptr();
        ASIGNADOR_GLOBAL.lock().init(inicio_heap, TAMANO_HEAP);
    }
}
