use crate::{address::Address, device::Device};
use failure::format_err;
use fuseable::Result;
use std::{collections::HashMap, sync::MutexGuard};

pub struct MockMemory {
    hash_map: HashMap<usize, u8>,
}

impl MockMemory {
    pub fn all_zeros() -> MockMemory { MockMemory { hash_map: HashMap::new() } }

    pub fn filled_with_device_defaults(device: &MutexGuard<Device>) -> MockMemory {
        let mut memory = MockMemory::all_zeros();
        for raw_register in device.raw.values() {
            match &raw_register.default {
                Some(default) => memory.write(&raw_register.address, default.to_vec()),
                None => {}
            }
        }
        memory
    }

    pub fn write(&mut self, address: &Address, value: Vec<u8>) {
        let offset = address.as_u64() as usize;

        for (i, byte) in value.iter().rev().enumerate() {
            self.hash_map.insert(offset + i, *byte);
        }
    }

    pub fn read(&self, address: &Address) -> Result<Vec<u8>> {
        let offset = address.as_u64() as usize;

        let mut ret: Vec<u8> = (0..address
            .bytes()
            .ok_or_else(|| format_err!("MockMemoy doesn't support unbounded read"))?)
            .map(|x| *self.hash_map.get(&(offset + x)).unwrap_or(&0u8))
            .collect();

        ret.reverse();

        Ok(ret)
    }
}
