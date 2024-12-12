pub mod double_buffer;
pub mod hardware;
pub mod rcc_setup;

use embedded_alloc::Heap;

#[global_allocator]
pub static ALLOCATOR: Heap = Heap::empty();
