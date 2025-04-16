#![no_std]

use core::ptr::NonNull;

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
// const PAGE_SIZE: usize = 0x1000;

pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
    page_size: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        assert!(
            PAGE_SIZE.is_power_of_two(),
            "PAGE_SIZE must be a power of two"
        );
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
            page_size: 0,
        }
    }

    fn can_alloc_bytes(&self, size: usize, align: usize) -> bool {
        let aligned_pos = align_up(self.b_pos, align);
        aligned_pos + size <= self.p_pos
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;
        self.page_size = PAGE_SIZE;
    }

    // 不能变化大小，固定大小的bump
    fn add_memory(&mut self, start: usize, size: usize) -> allocator::AllocResult {
        Err(allocator::AllocError::NoMemory)
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let size = layout.size();
        let align = layout.align();

        if !self.can_alloc_bytes(size, align) {
            return Err(allocator::AllocError::NoMemory);
        }

        // 向上对齐
        let aligned_pos = align_up(self.b_pos, align);

        // 如果大于了p_pos，超了
        if aligned_pos + size > self.p_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.b_pos = aligned_pos + size;
        self.count += 1;

        unsafe { Ok(NonNull::new(aligned_pos as *mut u8).unwrap()) }
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        self.count -= 1;

        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        if align_pow2 % PAGE_SIZE != 0 {
            return Err(allocator::AllocError::InvalidParam);
        }

        let size = num_pages * Self::PAGE_SIZE;
        let aligned_pos = align_down(
            self.p_pos
                .checked_sub(size)
                .ok_or(allocator::AllocError::NoMemory)?,
            align_pow2,
        );

        if aligned_pos < self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.p_pos = aligned_pos;
        Ok(aligned_pos)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {}

    fn total_pages(&self) -> usize {
        (self.end - self.start) / PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / PAGE_SIZE
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}
