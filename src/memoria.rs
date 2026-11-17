use linked_list_allocator::LockedHeap;

// Ampliamos el Heap a 1 MiB (1,048,576 bytes) para archivos, historial y procesos
pub const TAMANO_HEAP: usize = 1024 * 1024;

#[repr(align(16))]
struct EspacioHeap([u8; TAMANO_HEAP]);

static mut MEMORIA_HEAP: EspacioHeap = EspacioHeap([0; TAMANO_HEAP]);

#[global_allocator]
static ASIGNADOR_GLOBAL: LockedHeap = LockedHeap::empty();

#[allow(static_mut_refs)]
pub fn inicializar_heap() {
    unsafe {
        let inicio_heap = MEMORIA_HEAP.0.as_mut_ptr();
        ASIGNADOR_GLOBAL.lock().init(inicio_heap, TAMANO_HEAP);
    }
}
