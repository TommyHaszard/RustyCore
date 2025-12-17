use uuid::Uuid;

use crate::api::descriptor::{AttributePermission, Descriptor};

#[derive(Debug, Ord, Eq, PartialEq, PartialOrd, Clone)]
pub struct Characteristic {
    pub uuid: Uuid,
    pub properties: Vec<CharacteristicProperty>,
    pub permissions: Vec<AttributePermission>,
    pub value: Option<Vec<u8>>,
    pub descriptors: Vec<Descriptor>,
}

impl Default for Characteristic {
    fn default() -> Self {
        Characteristic {
            uuid: Uuid::nil(),
            properties: vec![
                CharacteristicProperty::Read,
                CharacteristicProperty::Write,
                CharacteristicProperty::Notify,
            ],
            permissions: vec![
                AttributePermission::Readable,
                AttributePermission::Writeable,
            ],
            value: None,
            descriptors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq)]
pub enum CharacteristicProperty {
    Broadcast,
    Read,
    WriteWithoutResponse,
    Write,
    AuthenticatedSignedWrites,
    Notify,
    NotifyEncryptionRequired,
    Indicate,
    IndicateEncryptionRequired,
    ExtendedProperties,
}

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq)]
pub enum CharacteristicWriteType {
    WriteWithoutResponse,
    WriteWithResponse,
}
