macro_rules! user_type {
    (VarInt) => {
        i32
    };
    (LengthPrefixedVec <$inner:ident>) => {
        Vec<$inner>
    };
    (LengthInferredVecU8) => {
        Vec<u8>
    };
    (Angle) => {
        f32
    };
    ($typ:ty) => {
        $typ
    };
}

macro_rules! user_type_convert_to_writeable {
    (VarInt, $e:expr) => {
        VarInt(*$e as i32)
    };
    (LengthPrefixedVec <$inner:ident>, $e:expr) => {
        LengthPrefixedVec::from($e.as_slice())
    };
    (LengthInferredVecU8, $e:expr) => {
        LengthInferredVecU8::from($e.as_slice())
    };
    (Angle, $e:expr) => {
        Angle(*$e)
    };
    ($typ:ty, $e:expr) => {
        $e
    };
}

macro_rules! packets {
    (
        $(
            $packet:ident {
                $(
                    $field:ident $typ:ident $(<$generics:ident>)?
                );* $(;)?
            } $(,)?
        )*
    ) => {
        $(
            #[derive(Debug, Clone)]
            pub struct $packet {
                $(
                    pub $field: user_type!($typ $(<$generics>)?),
                )*
            }

            #[allow(unused_imports, unused_variables)]
            impl crate::Readable for $packet {
                fn read(buffer: &mut ::std::io::Cursor<&[u8]>, version: crate::ProtocolVersion) -> anyhow::Result<Self>
                where
                    Self: Sized
                {
                    use anyhow::Context as _;
                    $(
                        let $field = <$typ $(<$generics>)?>::read(buffer, version)
                            .context(concat!("failed to read field `", stringify!($field), "` of packet `", stringify!($packet), "`"))?
                            .into();
                    )*

                    Ok(Self {
                        $(
                            $field,
                        )*
                    })
                }
            }

            #[allow(unused_variables)]
            impl crate::Writeable for $packet {
                fn write(&self, buffer: &mut Vec<u8>, version: crate::ProtocolVersion) {
                    $(
                        user_type_convert_to_writeable!($typ $(<$generics>)?, &self.$field).write(buffer, version);
                    )*
                }
            }
        )*
    };
}

macro_rules! def_enum {
    (
        $ident:ident ($discriminant_type:ty) {
            $(
                $discriminant:literal = $variant:ident
                $(
                    {
                        $(
                            $field:ident $typ:ident $(<$generics:ident>)?
                        );* $(;)?
                    }
                )?
            ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone)]
        pub enum $ident {
            $(
                $variant
                $(
                    {
                        $(
                            $field: user_type!($typ $(<$generics>)?),
                        )*
                    }
                )?,
            )*
        }

        impl crate::Readable for $ident {
            fn read(buffer: &mut ::std::io::Cursor<&[u8]>, version: crate::ProtocolVersion) -> anyhow::Result<Self>
                where
                    Self: Sized
            {
                use anyhow::Context as _;
                let discriminant = <$discriminant_type>::read(buffer, version)
                    .context(concat!("failed to read discriminant for enum type ", stringify!($ident)))?;

                match discriminant.into() {
                    $(
                        $discriminant => {
                            $(
                                $(
                                    let $field = <$typ $(<$generics>)?>::read(buffer, version)
                                        .context(concat!("failed to read field `", stringify!($field),
                                            "` of enum `", stringify!($ident), "::", stringify!($variant), "`"))?
                                            .into();
                                )*
                            )?

                            Ok($ident::$variant $(
                                {
                                    $(
                                        $field,
                                    )*
                                }
                            )?)
                        },
                    )*
                    _ => Err(anyhow::anyhow!(
                        concat!(
                            "no discriminant for enum `", stringify!($ident), "` matched value {:?}"
                        ), discriminant
                    ))
                }
            }
        }

        impl crate::Writeable for $ident {
            fn write(&self, buffer: &mut Vec<u8>, version: crate::ProtocolVersion) {
                match self {
                    $(
                        $ident::$variant $(
                            {
                                $($field,)*
                            }
                        )? => {
                            let discriminant = <$discriminant_type>::from($discriminant);
                            discriminant.write(buffer, version);

                            $(
                                $(
                                    user_type_convert_to_writeable!($typ $(<$generics>)?, $field).write(buffer, version);
                                )*
                            )?
                        }
                    )*
                }
            }
        }
    };
}

macro_rules! packet_enum {
    (
        $ident:ident {
            $($id:literal = $packet:ident),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone)]
        pub enum $ident {
            $(
                $packet($packet),
            )*
        }

        impl $ident {
            /// Returns the packet ID of this packet.
            pub fn id(&self) -> u32 {
                match self {
                    $(
                        $ident::$packet(_) => $id,
                    )*
                }
            }
        }

        impl crate::Readable for $ident {
            fn read(buffer: &mut ::std::io::Cursor<&[u8]>, version: crate::ProtocolVersion) -> anyhow::Result<Self>
            where
                Self: Sized
            {
                let packet_id = VarInt::read(buffer, version)?.0;
                match packet_id {
                    $(
                        id if id == $id => Ok($ident::$packet($packet::read(buffer, version)?)),
                    )*
                    _ => Err(anyhow::anyhow!("unknown packet ID {}", packet_id)),
                }
            }
        }

        impl crate::Writeable for $ident {
            fn write(&self, buffer: &mut Vec<u8>, version: crate::ProtocolVersion) {
                VarInt(self.id() as i32).write(buffer, version);
                match self {
                    $(
                        $ident::$packet(packet) => {
                            packet.write(buffer, version);
                        }
                    )*
                }
            }
        }

        $(
            impl VariantOf<$ident> for $packet {
                fn discriminant_id() -> u32 { $id }

                #[allow(unreachable_patterns)]
                fn destructure(e: $ident) -> Option<Self> {
                    match e {
                        $ident::$packet(p) => Some(p),
                        _ => None,
                    }
                }
            }
        )*
    }
}

/// Trait implemented for packets which can be converted from a packet
/// enum. For example, `SpawnEntity` implements `VariantOf<ServerPlayPacket>`.
pub trait VariantOf<Enum> {
    /// Returns the unique ID used to determine whether
    /// an enum variant matches this variant.
    fn discriminant_id() -> u32;

    /// Attempts to destructure the `Enum` into this type.
    /// Returns `None` if `enum` is not the correct variant.
    fn destructure(e: Enum) -> Option<Self>
    where
        Self: Sized;
}

use crate::io::{Angle, LengthInferredVecU8, LengthPrefixedVec, Nbt, VarInt};
use crate::Slot;
use base::{BlockId, BlockPosition};
use nbt::Blob;
use uuid::Uuid;

pub mod client;
pub mod server;