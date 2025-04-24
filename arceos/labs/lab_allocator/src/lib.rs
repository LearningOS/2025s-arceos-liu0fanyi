//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;
// use slab_allocator::Heap;
use buddy_system_allocator::Heap;
use rlsf::Tlsf;

pub struct LabByteAllocator {
    // inner: Heap<32>,
    inner: Tlsf<'static, u32, u32, 28, 32>, // max pool size: 32 * 2^28 = 8G
    total_bytes: usize,
    used_bytes: usize,

    bump_start: usize,
    bump_end: usize,
    bump_current: usize,

    last_size: usize,
    counter: usize,
    who_need_memory: bool,
}

impl LabByteAllocator {
    pub const fn new() -> Self {
        Self {
            // inner: Heap::<32>::new(),
            inner: Tlsf::new(),
            total_bytes: 0,
            used_bytes: 0,
            bump_start: 0,
            bump_end: 0,
            bump_current: 0,
            counter: 0,
            last_size: 0,
            who_need_memory: false,
        }
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        let buddy_size = size - 1; // 分配一半给buddy
        let bump_size = size - buddy_size; // 剩余一半给bump

        // unsafe { self.inner.init(start, buddy_size) };
        //
        unsafe {
            let pool = core::slice::from_raw_parts_mut(start as *mut u8, buddy_size);
            self.inner
                .insert_free_block_ptr(NonNull::new(pool).unwrap())
                .unwrap();
        }
        self.total_bytes = buddy_size;

        self.bump_start = start + buddy_size;
        self.bump_end = self.bump_start + bump_size;
        self.bump_current = self.bump_start;

        // if DEBUG {
        axlog::ax_println!("{}start:{} size:{}{}", GREEN, start, size, RESET);
        // }
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        if DEBUG {
            axlog::ax_println!(
                "{}add_memory bump_end:{} start:{} size:{}{}, whoneed:{}",
                BLUE,
                self.bump_end,
                start,
                size,
                RESET,
                self.who_need_memory
            );
        }

        if self.who_need_memory {
            unsafe {
                let pool = core::slice::from_raw_parts_mut(start as *mut u8, size);
                self.inner
                    .insert_free_block_ptr(NonNull::new(pool).unwrap())
                    .ok_or(AllocError::InvalidParam)?;
            }
            self.total_bytes += size;
        } else {
            // 自己从新的位置开始
            // 多余的给buddy
            // unsafe { self.inner.add_to_heap(self.bump_current, self.bump_end) };

            if self.bump_end != start {
                self.bump_start = start;
                self.bump_end = self.bump_start + size;
                self.bump_current = self.bump_start;
            } else {
                if DEBUG {
                    axlog::ax_println!("just add");
                }
                self.bump_end += size;
            }
        }

        Ok(())
    }
}

const RED: &str = "\x1B[31m";
const GREEN: &str = "\x1B[32m";
const BLUE: &str = "\x1B[34m";
const RESET: &str = "\x1B[0m";
const DEBUG: bool = false;

fn is_64_family(x: usize) -> bool {
    let tz = x.trailing_zeros();
    tz % 2 != 1
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();
        if align == 1 && size < 524288 && self.last_size >= 524288 {
            self.counter += 1;
            if DEBUG {
                axlog::ax_println!(
                    "{}counter:{}{}-{}size:{}{}",
                    RED,
                    self.counter,
                    RESET,
                    GREEN,
                    size,
                    RESET
                );
            }
        }
        if align == 1 {
            self.last_size = size;
            // 这些是永恒不释放的
            if is_64_family(size - self.counter) {
                // 使用bump分配器
                if DEBUG {
                    axlog::ax_println!("{}size-counter:{}{}", RED, size - self.counter, RESET);
                }

                // 对齐 bump_current 向上（正向 bump 的关键步骤）
                let align_mask = align - 1;
                let aligned_current = (self.bump_current + align_mask) & !align_mask;

                let new_current = aligned_current + size;

                if new_current > self.bump_end {
                    self.who_need_memory = false;
                    return Err(AllocError::NoMemory);
                }

                let result = aligned_current;

                self.bump_current = new_current;

                return Ok(unsafe { NonNull::new_unchecked(result as *mut u8) });
            }
        }

        let pos = self.inner.allocate(layout).ok_or(AllocError::NoMemory);
        // Ok(ptr)
        // 正常走buddy分配器
        // let pos = self.inner.alloc(layout).map_err(|_| AllocError::NoMemory);
        if pos.is_err() {
            self.who_need_memory = true;
        } else {
            self.used_bytes += layout.size();
        }
        if DEBUG {
            axlog::ax_println!(
                "pos: {:?}, alloc: layout:{:?}, align:{:?}",
                pos,
                layout,
                align
            );
        }
        // axlog::ax_println!("{}counter:{}{}-{}size:{}{}", RED, self.counter, RESET, GREEN, size, RESET);
        pos
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        // axlog::ax_println!("dealloc: layout:{:?}", layout);
        if DEBUG {
            axlog::ax_println!(
                "dealloc: {}layout:{:?}{}, pos:{:?}",
                RED,
                layout,
                RESET,
                pos
            );
        }
        // self.inner.dealloc(pos, layout)
        unsafe { self.inner.deallocate(pos, layout.align()) }
        self.used_bytes -= layout.size();
    }

    fn total_bytes(&self) -> usize {
        // axlog::ax_println!("total size:{:?}", self.total_bytes);
        // self.inner.stats_total_bytes()
        self.total_bytes
    }

    fn used_bytes(&self) -> usize {
        // self.inner.stats_alloc_actual()
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        // self.inner.stats_total_bytes() - self.inner.stats_alloc_actual()
        self.total_bytes - self.used_bytes
    }
}
