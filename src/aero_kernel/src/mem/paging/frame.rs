/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use bit_field::BitField;
use spin::Once;
use stivale_boot::v2::StivaleMemoryMapEntry;
use stivale_boot::v2::{StivaleMemoryMapEntryType, StivaleMemoryMapIter, StivaleMemoryMapTag};

use super::mapper::*;
use super::page::*;

use super::addr::PhysAddr;

use crate::mem::paging::align_up;
use crate::utils::sync::Mutex;

const BUDDY_BITS: u64 = (core::mem::size_of::<usize>() * 8) as u64;

static BUDDY_SIZE: [u64; 3] = [Size4KiB::SIZE, Size4KiB::SIZE * 2, Size2MiB::SIZE];

pub struct LockedFrameAllocator(Once<Mutex<GlobalFrameAllocator>>);

impl LockedFrameAllocator {
    /// Constructs a new uninitialized and locked version of the global frame
    /// allocator.
    pub(super) const fn new_uninit() -> Self {
        Self(Once::new())
    }

    /// Initializes the inner locked global frame allocator.
    pub(super) fn init(
        &self,
        memory_map: &'static StivaleMemoryMapTag,
        kernel_base: PhysAddr,
        kernel_end: PhysAddr,
    ) {
        self.0.call_once(|| {
            Mutex::new(GlobalFrameAllocator::new(
                memory_map,
                kernel_base,
                kernel_end,
            ))
        });
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(0)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
    }
}

unsafe impl FrameAllocator<Size2MiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(1)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
    }
}

pub struct RangeMemoryIter {
    iter: StivaleMemoryMapIter<'static>,

    kernel_base: PhysAddr,
    kernel_end: PhysAddr,

    cursor_base: PhysAddr,
    cursor_end: PhysAddr,
}

impl RangeMemoryIter {
    /// Helper function that returns [`true`] if the provided range intersects
    /// between the current cursor base and the cursor end addresses.
    fn intersects_in_range(&self, base: PhysAddr, end: PhysAddr) -> bool {
        (self.cursor_base <= end && self.cursor_end <= end)
            && (base <= self.cursor_base && base <= self.cursor_end)
    }

    fn cursor_align_up(&mut self) -> PhysAddr {
        if self.cursor_base < self.kernel_base {
            self.cursor_base = self.kernel_base;
        }

        if self.intersects_in_range(self.kernel_base, self.kernel_end) {
            self.kernel_end
        } else {
            self.cursor_base
        }
    }

    fn cursor_align_down(&self) -> Option<PhysAddr> {
        if self.cursor_base >= self.cursor_end {
            return None;
        }

        if self.kernel_end < self.cursor_end && self.kernel_base >= self.cursor_base {
            Some(self.kernel_base)
        } else {
            Some(self.cursor_end)
        }
    }
}

impl Iterator for RangeMemoryIter {
    type Item = (PhysAddr, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor_base >= self.cursor_end {
            if let Some(entry) = loop {
                // We need to find out the next useable memory range from
                // the memory map and set the cursor to the start of it.
                let next = self.iter.next()?;

                if next.entry_type == StivaleMemoryMapEntryType::Usable {
                    break Some(next);
                }
            } {
                self.cursor_base = PhysAddr::new(entry.base).align_up(Size4KiB::SIZE);
                self.cursor_end = PhysAddr::new(entry.base + entry.length);
            } else {
                // We reached the end of the memory map.
                return None;
            }
        }

        self.cursor_base = self.cursor_align_up().align_up(Size4KiB::SIZE);

        if let Some(end) = self.cursor_align_down() {
            let range = Some((self.cursor_base, end - self.cursor_base));

            self.cursor_base = end.align_up(Size4KiB::SIZE);
            range
        } else {
            self.next()
        }
    }
}

#[derive(Copy, Clone)]
#[repr(usize)]
pub enum BuddyOrdering {
    Size4KiB = 0,
    Size8KiB = 1,
}

pub fn pmm_alloc(ordering: BuddyOrdering) -> PhysAddr {
    let ordering = ordering as usize;
    debug_assert!(ordering <= BUDDY_SIZE.len());

    unsafe {
        let addr = super::FRAME_ALLOCATOR
            .0
            .get()
            .map(|m| m.lock().allocate_frame_inner(ordering).map(|f| f))
            .expect("pmm: out of memory")
            .expect("pmm: frame allocator not initialized");

        let virt = crate::PHYSICAL_MEMORY_OFFSET + addr.as_u64();
        let fill_size = BUDDY_SIZE[ordering] as usize;
        let slice = core::slice::from_raw_parts_mut(virt.as_mut_ptr::<u8>(), fill_size);

        // We always zero out memory for security reasons.
        slice.fill(0x00);

        addr
    }
}

#[derive(Debug)]
struct MemoryRange {
    addr: PhysAddr,
    size: u64,
}

struct BootAllocator {
    memory_ranges: &'static mut [MemoryRange],
}

impl BootAllocator {
    fn new(memory_ranges: &'static mut [MemoryRange]) -> Self {
        Self { memory_ranges }
    }

    fn allocate(&mut self, size: usize) -> *mut u8 {
        let size = align_up(size as u64, Size4KiB::SIZE);

        for range in self.memory_ranges.iter_mut() {
            if range.size >= size {
                let addr = range.addr;

                range.addr += size;
                range.size -= size;

                return unsafe { crate::PHYSICAL_MEMORY_OFFSET + addr.as_u64() }.as_mut_ptr();
            }
        }

        unreachable!("pmm: bootstrap allocator is out of memory")
    }
}

pub struct GlobalFrameAllocator {
    buddies: [&'static mut [u64]; 3],
    free: [usize; 3],

    base: PhysAddr,
    end: PhysAddr,
}

impl GlobalFrameAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    fn new(
        memory_map: &'static StivaleMemoryMapTag,
        kernel_base: PhysAddr,
        kernel_end: PhysAddr,
    ) -> Self {
        let mut iter = memory_map.iter();
        let cursor = iter
            .next()
            .expect("stivale2: unexpected end of the memory map");

        // Find a memory map entry that is big enough to fit all of the items in
        // range memory iter.
        let requested_size = core::mem::size_of::<MemoryRange>() as u64 * memory_map.entries_len;
        let mut region = None;

        for i in 0..memory_map.entries_len {
            let entry = &memory_map.as_slice()[i as usize];

            if entry.length >= requested_size && entry.base > kernel_end.as_u64() {
                // Found a big enough memory map entry.
                //
                // SAFETY: Its safe for us to mutate the memory map entry & life is ment to be
                // unsafe. We use the power of holy transmutes here.
                let entry_mut = unsafe { &mut *(entry as *const _ as *mut StivaleMemoryMapEntry) };
                let base = entry_mut.base;

                entry_mut.base += requested_size;
                entry_mut.length -= requested_size;

                region = Some(PhysAddr::new(base));

                break;
            }
        }

        let ranges = unsafe {
            let phys_addr = region.expect("stivale2: out of memory").as_u64();
            let virt_addr = crate::PHYSICAL_MEMORY_OFFSET + phys_addr;

            core::slice::from_raw_parts_mut::<MemoryRange>(
                virt_addr.as_mut_ptr(),
                requested_size as usize,
            )
        };

        let range_iter = RangeMemoryIter {
            iter,

            kernel_base,
            kernel_end,

            cursor_base: PhysAddr::new(cursor.base),
            cursor_end: PhysAddr::new(cursor.base + cursor.length),
        };

        // Lets goo! Now lets initialize the bootstrap allocator so we can initialize
        // our efficient buddy allocator. We need a seperate allocator since some computers
        // such as Macs have a shitload of memory map entries so, we cannt assume the amount
        // of maximum mmap entries and allocate space for it on the stack instead. God damn it.
        let mut i = 0;

        for (addr, size) in range_iter {
            ranges[i] = MemoryRange { addr, size };
            i += 1;
        }

        let base = ranges[0].addr;
        let end = ranges[i - 1].addr + ranges[i - 1].size;

        let mut bootstrapper = BootAllocator::new(&mut ranges[..i]);

        let mut this = Self {
            base,
            end,

            buddies: [&mut [], &mut [], &mut []],
            free: [0; 3],
        };

        let size = this.end - this.base;

        // Allocate the buddies using prealloc:
        for (i, bsize) in BUDDY_SIZE.iter().enumerate() {
            let chunk = ((size / bsize) + BUDDY_BITS - 1) / BUDDY_BITS;
            let chunk_size = chunk * 8;

            let chunk_ptr = bootstrapper.allocate(chunk_size as usize) as *mut u64;
            let chunk_slice = unsafe { core::slice::from_raw_parts_mut(chunk_ptr, chunk as usize) };

            chunk_slice.fill(0x00);
            this.buddies[i] = chunk_slice;
        }

        for region in bootstrapper.memory_ranges.iter() {
            this.insert_range(region.addr, region.addr + region.size);
        }

        this
    }

    /// Find the perfect buddy ordering for the provided address range.
    fn find_ordering(&self, address: PhysAddr, chunk_size: u64) -> usize {
        for ordering in (0..BUDDY_SIZE.len()).rev() {
            let size = BUDDY_SIZE[ordering];

            // Too big...
            if size > chunk_size {
                continue;
            }

            let mask = BUDDY_SIZE[ordering] - 1;

            if mask & address.as_u64() != 0 {
                continue;
            } else {
                return ordering;
            }
        }

        return 0;
    }

    /// Helper function that translates a address to it's part in the map. This
    /// function returns a tuple of (index, bit) where index is the index on the
    /// `u64` array and `bit` is the bit over the `u64`.
    fn get_byte_bit(&self, addr: PhysAddr, order: usize) -> (u64, u64) {
        let offset = addr - self.base;
        let id = offset / BUDDY_SIZE[order];

        (id / BUDDY_BITS, id % BUDDY_BITS)
    }

    fn set_bit(&mut self, address: PhysAddr, ordering: usize) -> bool {
        let (byte, bit) = self.get_byte_bit(address, ordering);

        let chunk = &mut self.buddies[ordering][byte as usize];
        let change = (*chunk).get_bit(bit as usize) == false;

        if change {
            (*chunk).set_bit(bit as usize, true);
            self.free[ordering] += 1;
        }

        change
    }

    /// Inserts the provided memory range.
    fn insert_range(&mut self, base: PhysAddr, end: PhysAddr) {
        let mut remaning = end - base;
        let mut current = base;

        while remaning > 0 {
            let ordering = self.find_ordering(current, remaning);
            let size = BUDDY_SIZE[ordering];

            self.set_bit(current, ordering);

            current += size;
            remaning -= size;
        }
    }

    /// Finds a free chunk with the provided `ordering`.
    fn find_free(&mut self, ordering: usize) -> Option<PhysAddr> {
        for (i, chunk) in self.buddies[ordering].iter_mut().enumerate() {
            let mut chunk_value = *chunk;

            if chunk_value != 0 {
                let mut bit = 0;

                while !chunk_value.get_bit(0) {
                    chunk_value >>= 1;
                    bit += 1;
                }

                (*chunk).set_bit(bit, false);
                self.free[ordering] -= 1;

                return Some(
                    self.base.align_up(BUDDY_SIZE[ordering])
                        + (BUDDY_SIZE[ordering] * BUDDY_BITS * i as u64)
                        + BUDDY_SIZE[ordering] * bit as u64,
                );
            }
        }

        None
    }

    fn allocate_frame_inner(&mut self, ordering: usize) -> Option<PhysAddr> {
        let size = BUDDY_SIZE[ordering];

        // Loop through the list of buddies until we can find one that can give us
        // the requested memory.
        for (i, &bsize) in BUDDY_SIZE[ordering..].iter().enumerate() {
            let i = i + ordering;

            if self.free[i] > 0 {
                let result = self.find_free(i)?;
                let mut remaning = bsize - size;

                if remaning > 0 {
                    for j in (0..=i).rev() {
                        let sizee = BUDDY_SIZE[j];

                        if remaning >= sizee {
                            self.set_bit(result + (remaning - sizee) + size, j);
                            remaning -= sizee;
                        }
                    }
                }

                return Some(result);
            }
        }

        None
    }
}
