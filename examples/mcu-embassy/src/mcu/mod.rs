// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

pub mod double_buffer;
pub mod hardware;
pub mod rcc_setup;

use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
pub static ALLOCATOR: Heap = Heap::empty();
