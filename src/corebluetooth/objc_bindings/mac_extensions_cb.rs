use std::ffi::CStr;

use objc2::rc::Retained;
use objc2_core_bluetooth::{CBCharacteristic, CBService, CBUUID};
use objc2_foundation::{NSString, NSUUID};
use uuid::Uuid;

// NOTE: Bluetooth Short Sevice UUIDs follow this pattern:
// xxxxxxxx-0000-1000-8000-00805F9B34FB
// Last 12 bytes are always the same 
const BLUETOOTH_BASE_LOWER_96: u128 = 0x0000_1000_8000_00805F9B34FB;
const LOWER_96_BIT_MASK: u128 = 0xFFFFFFFFFFFFFFFFFFFFFFFF;

pub fn uuid_to_cbuuid(uuid: Uuid) -> Retained<CBUUID> {
    unsafe { CBUUID::UUIDWithString(&NSString::from_str(&uuid.to_short_string())) }
}

pub fn nsuuid_to_uuid(uuid: &NSUUID) -> Uuid {
    Uuid::parse_str(&uuid.UUIDString().to_string()).unwrap()
}

pub unsafe fn cbuuid_to_uuid(cbuuid: &CBUUID) -> Uuid {
    unsafe {
        let uuid_string = cbuuid.UUIDString().to_string();
        Uuid::from_string(uuid_string)
    }
}

pub unsafe fn nsstring_to_string(nsstring: *const NSString) -> Option<String> {
    unsafe {
        nsstring
            .as_ref()
            .and_then(|ns| CStr::from_ptr(ns.UTF8String()).to_str().ok())
            .map(String::from)
    }
}

pub trait UuidExtension {
    fn get_uuid(self) -> Uuid;
}

impl UuidExtension for &CBService {
    fn get_uuid(self) -> Uuid {
        unsafe { self.UUID().get_uuid() }
    }
}

impl UuidExtension for &CBCharacteristic {
    fn get_uuid(self) -> Uuid {
        unsafe { self.UUID().get_uuid() }
    }
}

impl UuidExtension for &CBUUID {
    fn get_uuid(self) -> Uuid {
        let uuid_str = unsafe { self.UUIDString() }.to_string();
        return Uuid::from_string(uuid_str);
    }
}

pub trait CbuuidConvert {
    fn from_short(uuid: u32) -> Uuid;

    fn from_string(uuid_str: String) -> Uuid;

    fn to_short_string(&self) -> String;
}

impl CbuuidConvert for Uuid {
    fn from_short(short_uuid: u32) -> Uuid {
        let full_uuid = ((short_uuid as u128) << 96) | BLUETOOTH_BASE_LOWER_96;
        Uuid::from_u128(full_uuid)
    }

    // NOTE: CoreBluetooth uses 4char (16 bit) Short UUIDs for Standard Service Identification, to be data efficient.
    fn from_string(uuid_string: String) -> Uuid {
        match Uuid::parse_str(&uuid_string) {
            Ok(uuid) => uuid,
            Err(_) => {
                let long = match uuid_string.len() {
                    4 => format!("0000{}-0000-1000-8000-00805f9b34fb", uuid_string),
                    8 => format!("{}-0000-1000-8000-00805f9b34fb", uuid_string),
                    _ => uuid_string.clone(),
                };
                Uuid::parse_str(&long)
                    .unwrap_or_else(|_| panic!("Invalid UUID string: {}", uuid_string))
            }
        }
    }

    fn to_short_string(&self) -> String {
        let uuid = self.as_u128();
        let lower_96_bits = uuid & LOWER_96_BIT_MASK;

        if lower_96_bits == BLUETOOTH_BASE_LOWER_96 {
            let assigned_value = (uuid >> 96) as u32;

            match assigned_value {
                0..=0xFFFF => format!("{:04x}", assigned_value),
                _ => format!("{:08x}", assigned_value),
            }
        } else {
            self.to_string()
        }
    }
}
