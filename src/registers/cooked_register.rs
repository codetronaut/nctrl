use crate::{
    address::Address,
    common::{to_hex_string, Description},
    communication_channel::CommunicationChannel,
    valuemap::*,
};

use failure::format_err;
use fuseable::{type_name, Either, FuseableError};
use fuseable_derive::Fuseable;
use itertools::izip;
use log::debug;
use parse_num::parse_num_mask;

use serde_derive::*;
use std::fmt::Debug;


#[derive(Debug, Serialize, Fuseable)]
pub struct CookedRegister {
    #[fuseable(ro)]
    pub address: Address,
    #[fuseable(ro)]
    pub description: Option<Description>,
    #[serde(default, deserialize_with = "deser_valuemap")]
    pub map: Option<ValueMap>,
    #[serde(default = "bool_false")]
    #[fuseable(ro)]
    pub writable: bool,
    #[fuseable(ro)]
    pub default: Option<u64>,
}

impl CookedRegister {
    pub fn read_value(
        &self,
        path: &mut dyn Iterator<Item = &str>,
        comm_channel: &CommunicationChannel,
    ) -> fuseable::Result<Either<Vec<String>, Vec<u8>>> {
        match path.next() {
            Some(s) => Err(FuseableError::not_a_directory(type_name(&self), s)),
            None => {
                let value = comm_channel.read_value(&self.address)?;

                match &self.map {
                    Some(map) => map.lookup(value).map(Either::Right),
                    None => to_hex_string(&value).map(Either::Right),
                }
            }
        }
    }

    pub fn write_value(
        &self,
        path: &mut dyn Iterator<Item = &str>,
        value: Vec<u8>,
        comm_channel: &CommunicationChannel,
    ) -> fuseable::Result<()> {
        match path.next() {
            Some(s) => Err(FuseableError::not_a_directory(type_name(&self), s)),
            None => {
                let encoded_value = match &self.map {
                    Some(map) => map.encode(value.clone())?,
                    None => {
                        if let Some(width) = self.address.bytes() {
                            let (mask, mut value) =
                                parse_num_mask(String::from_utf8_lossy(&value))?;

                            if value.len() > width as usize {
                                return Err(format_err!("value {:?} to write was longer ({}) than cooked register {:?} with width of {}", value, value.len(), self, width));
                            }

                            // pad value to the length of the register
                            while value.len() < width as usize {
                                value.insert(0, 0);
                            }

                            match mask {
                                Some(mut mask) => {
                                    // pad mask to the length of the register
                                    while mask.len() < width as usize {
                                        mask.insert(0, 0);
                                    }

                                    let current_value = comm_channel.read_value(&self.address)?;

                                    izip!(mask, value, current_value)
                                        .map(|(m, val, cur)| (val & m) | (cur & !m))
                                        .collect()
                                }
                                None => value,
                            }
                        } else {
                            panic!("the cooked register written to {:?} did not specify a width, don't know what to do", self)
                        }
                    }
                };

                debug!("{:?} encoded to {:?}", value, encoded_value);

                comm_channel.write_value(&self.address, encoded_value)
            }
        }
    }
}
