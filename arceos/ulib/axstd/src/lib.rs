//! # The ArceOS Standard Library
//!
//! The [ArceOS] Standard Library is a mini-std library, with an interface similar
//! to rust [std], but calling the functions directly in ArceOS modules, instead
//! of using libc and system calls.
//!
//! These features are exactly the same as those in [axfeat], they are used to
//! provide users with the selection of features in axfeat, without import
//! [axfeat] additionally:
//!
//! ## Cargo Features
//!
//! - CPU
//!     - `smp`: Enable SMP (symmetric multiprocessing) support.
//!     - `fp_simd`: Enable floating point and SIMD support.
//! - Interrupts:
//!     - `irq`: Enable interrupt handling support.
//! - Memory
//!     - `alloc`: Enable dynamic memory allocation.
//!     - `alloc-tlsf`: Use the TLSF allocator.
//!     - `alloc-slab`: Use the slab allocator.
//!     - `alloc-buddy`: Use the buddy system allocator.
//!     - `paging`: Enable page table manipulation.
//!     - `tls`: Enable thread-local storage.
//! - Task management
//!     - `multitask`: Enable multi-threading support.
//!     - `sched_fifo`: Use the FIFO cooperative scheduler.
//!     - `sched_rr`: Use the Round-robin preemptive scheduler.
//!     - `sched_cfs`: Use the Completely Fair Scheduler (CFS) preemptive scheduler.
//! - Upperlayer stacks
//!     - `fs`: Enable file system support.
//!     - `myfs`: Allow users to define their custom filesystems to override the default.
//!     - `net`: Enable networking support.
//!     - `dns`: Enable DNS lookup support.
//!     - `display`: Enable graphics support.
//! - Device drivers
//!     - `bus-mmio`: Use device tree to probe all MMIO devices.
//!     - `bus-pci`: Use PCI bus to probe all PCI devices.
//!     - `driver-ramdisk`: Use the RAM disk to emulate the block device.
//!     - `driver-ixgbe`: Enable the Intel 82599 10Gbit NIC driver.
//!     - `driver-bcm2835-sdhci`: Enable the BCM2835 SDHCI driver (Raspberry Pi SD card).
//! - Logging
//!     - `log-level-off`: Disable all logging.
//!     - `log-level-error`, `log-level-warn`, `log-level-info`, `log-level-debug`,
//!       `log-level-trace`: Keep logging only at the specified level or higher.
//!
//! [ArceOS]: https://github.com/arceos-org/arceos

#![cfg_attr(all(not(test), not(doc)), no_std)]
#![feature(doc_cfg)]
#![feature(doc_auto_cfg)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
#[doc(no_inline)]
pub use alloc::{boxed, format, string, vec, vec::Vec};

#[doc(no_inline)]
pub use core::{arch, cell, cmp, hint, marker, mem, ops, ptr, slice, str};

#[macro_use]
mod macros;

pub mod env;
pub mod io;
pub mod os;
pub mod process;
pub mod sync;
pub mod thread;
pub mod time;

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "net")]
pub mod net;

// make run A=exercises/support_hashmap
#[cfg(feature = "alloc")]
pub mod collections {
    pub use alloc::collections::*;
    use alloc::vec::Vec;
    use axhal::misc::random;
    use core::mem;

    const INITIAL_CAPACITY: usize = 8;
    const LOAD_FACTOR: f64 = 0.7;

    enum Bucket<K, V> {
        Empty,
        Occupied(K, V),
        Tombstone,
    }

    pub struct HashMap<K, V> {
        buckets: Vec<Bucket<K, V>>,
        len: usize,
        capacity: usize,
        seed: u128,
    }

    impl<K, V> HashMap<K, V>
    where
        K: Eq + Clone + AsRef<[u8]>,
        V: Default,
    {
        pub fn new() -> Self {
            let capacity = INITIAL_CAPACITY;
            let mut buckets = Vec::with_capacity(capacity);
            for _ in 0..capacity {
                buckets.push(Bucket::Empty);
            }

            Self {
                buckets,
                len: 0,
                capacity,
                seed: Self::gen_seed(),
            }
        }

        fn gen_seed() -> u128 {
            random()
        }

        fn hash(&self, key: &K) -> usize {
            let bytes = key.as_ref();
            let mut hash = self.seed as usize;

            for &byte in bytes {
                hash = hash.wrapping_mul(31).wrapping_add(byte as usize);
            }

            hash % self.capacity
        }

        pub fn insert(&mut self, key: K, value: V) -> Option<V> {
            if (self.len as f64 / self.capacity as f64) >= LOAD_FACTOR {
                self.resize();
            }

            let mut index = self.hash(&key);
            let mut first_tombstone = None;

            // 线性探测
            loop {
                match &self.buckets[index] {
                    Bucket::Occupied(k, _) if *k == key => {
                        let default_v: V = V::default();
                        // key已经存在，替换value
                        let old = mem::replace(
                            &mut self.buckets[index],
                            Bucket::Occupied(key.clone(), value),
                        );
                        if let Bucket::Occupied(_, v) = old {
                            return Some(v);
                        }
                        // 上一个return道理上必然返回，但需要先取出old，会导致value被认为已经move了，因而在这里加一个必然返回
                        return Some(default_v);
                    }
                    Bucket::Tombstone if first_tombstone.is_none() => {
                        first_tombstone = Some(index);
                    }
                    Bucket::Empty => {
                        let insert_pos = first_tombstone.unwrap_or(index);
                        self.buckets[insert_pos] = Bucket::Occupied(key.clone(), value);
                        self.len += 1;
                        return None;
                    }
                    _ => {}
                }
                index = (index + 1) % self.capacity;
            }
        }

        pub fn get(&self, key: &K) -> Option<&V> {
            let mut index = self.hash(key);

            loop {
                match &self.buckets[index] {
                    Bucket::Empty => return None,
                    Bucket::Occupied(k, v) if k == key => return Some(v),
                    _ => {}
                }
                index = (index + 1) % self.capacity;
            }
        }

        pub fn remove(&mut self, key: &K) -> Option<V> {
            let mut index = self.hash(key);

            loop {
                match &mut self.buckets[index] {
                    Bucket::Occupied(k, _) if k == key => {
                        let old = mem::replace(&mut self.buckets[index], Bucket::Tombstone);
                        self.len -= 1;
                        if let Bucket::Occupied(_, v) = old {
                            return Some(v);
                        }
                    }
                    Bucket::Empty => return None,
                    _ => {}
                }
                index = (index + 1) % self.capacity;
            }
        }

        fn resize(&mut self) {
            let new_capacity = self.capacity * 2;
            let mut new_map = HashMap {
                buckets: Vec::with_capacity(new_capacity),
                len: 0,
                capacity: new_capacity,
                seed: random(),
            };

            for _ in 0..new_capacity {
                new_map.buckets.push(Bucket::Empty);
            }

            for bucket in mem::take(&mut self.buckets) {
                if let Bucket::Occupied(k, v) = bucket {
                    new_map.insert(k, v);
                }
            }

            *self = new_map;
        }

        pub fn iter(&self) -> Iter<'_, K, V> {
            Iter {
                inner: self.buckets.iter(),
            }
        }
    }

    pub struct Iter<'a, K, V> {
        inner: core::slice::Iter<'a, Bucket<K, V>>,
    }

    impl<'a, K, V> Iterator for Iter<'a, K, V> {
        type Item = (&'a K, &'a V);

        fn next(&mut self) -> Option<Self::Item> {
            while let Some(bucket) = self.inner.next() {
                if let Bucket::Occupied(k, v) = bucket {
                    return Some((k, v));
                }
            }

            None
        }
    }
}
