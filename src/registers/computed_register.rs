use crate::{
    bytes::{FromBytes, ToBytes},
    camera,
    common::{Description, Range},
    device::{Device, DeviceLike},
    lua_util::FailureCompat,
};

use fuseable::{type_name, Either, FuseableError};
use fuseable_derive::Fuseable;

use failure::format_err;

use rlua::{Function, RegistryKey, ToLua};

use serde_derive::*;

#[derive(Debug, Deserialize, Fuseable, Clone)]
#[serde(tag = "type")]
enum ComputedRegisterType {
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "string")]
    String,
    #[serde(rename = "binary")]
    Binary,
}

#[derive(Debug, Deserialize, Fuseable)]
pub struct ComputedRegister {
    #[fuseable(ro)]
    description: Description,
    #[fuseable(ro)]
    get: Option<String>,
    #[fuseable(ro)]
    set: Option<String>,
    #[fuseable(ro)]
    #[serde(flatten)]
    range: Option<Range>,
    #[fuseable(ro)]
    #[serde(flatten)]
    ty: ComputedRegisterType,

    #[fuseable(skip)]
    #[serde(skip)]
    read_function: std::cell::RefCell<Option<RegistryKey>>,
    #[fuseable(skip)]
    #[serde(skip)]
    write_function: std::cell::RefCell<Option<RegistryKey>>,
}

impl ComputedRegister {
    pub fn read_value(
        &self,
        path: &mut dyn Iterator<Item = &str>,
        device: &Device,
    ) -> fuseable::Result<Either<Vec<String>, Vec<u8>>> {
        match path.next() {
            Some(s) => Err(FuseableError::not_a_directory(type_name(&self), s)),
            None => camera::with_camera(|camera| {
                camera
                    .lua_vm
                    .context(|lua_ctx| {
                        lua_ctx.scope(|lua| {
                            let (raw_tbl, cooked_tbl, computed_tbl) =
                                ro_tables_from_device!(lua_ctx, lua, device);

                            if self.read_function.borrow().is_none() {
                                let script = self.get.as_ref().ok_or_else(|| {
                                    format_err!(
                                        "cannot not read computed value {:#?} with no get script",
                                        self
                                    )
                                })?;

                                let script =
                                    format!("function (raw, cooked, computed) {} end", script);

                                *self.read_function.borrow_mut() =
                                    Some(lua_ctx.create_registry_value(
                                        lua_ctx.load(&script).eval::<Function>()?,
                                    )?);
                            }

                            lua_ctx
                                .registry_value::<Function>(
                                    self.read_function.borrow().as_ref().unwrap(),
                                )?
                                .call::<_, rlua::Value>((raw_tbl, cooked_tbl, computed_tbl))
                                .map_err(|e| e.into())
                                .and_then(ToBytes::to_bytes)
                        })
                    })
                    .map(Either::Right)
            }),
        }
    }

    pub fn write_value(
        &self,
        path: &mut dyn Iterator<Item = &str>,
        value: Vec<u8>,
        device: &Device,
    ) -> fuseable::Result<()> {
        match path.next() {
            Some(s) => Err(FuseableError::not_a_directory(type_name(&self), s)),
            None => camera::with_camera(|camera| {
                camera.lua_vm.context(|lua_ctx| {
                    lua_ctx.scope(|lua| {
                        let (raw_tbl, cooked_tbl, computed_tbl) =
                            rw_tables_from_device!(lua_ctx, lua, device);

                        let value = match self.ty {
                            ComputedRegisterType::Float => {
                                <f64 as FromBytes>::from_bytes(value)?.to_lua(lua_ctx)?
                            }
                            ComputedRegisterType::Int => {
                                <i64 as FromBytes>::from_bytes(value)?.to_lua(lua_ctx)?
                            }
                            ComputedRegisterType::String => {
                                <String as FromBytes>::from_bytes(value)?.to_lua(lua_ctx)?
                            }
                            ComputedRegisterType::Binary => value.to_lua(lua_ctx)?,
                        };


                        if self.write_function.borrow().is_none() {
                            let script = self.set.as_ref().ok_or_else(|| {
                                format_err!(
                                    "cannot not write computed value {:#?} with no set script",
                                    self
                                )
                            })?;

                            let script =
                                format!("function (value, raw, cooked, computed) {} end", script);

                            *self.write_function.borrow_mut() =
                                Some(lua_ctx.create_registry_value(
                                    lua_ctx.load(&script).eval::<Function>()?,
                                )?);
                        }

                        lua_ctx
                            .registry_value::<Function>(
                                self.write_function.borrow().as_ref().unwrap(),
                            )?
                            .call((value, raw_tbl, cooked_tbl, computed_tbl))
                            .map_err(|e| e.into())
                    })
                })
            }),
        }
    }
}
