use cosmwasm_std::{StdError, StdResult};
use std::array::TryFromSliceError;
use std::convert::TryInto;

/// This is just a copy of the necessary funtions from storage-plus
/// but not introduced until cw-storage-plus 0.10.0.  Can remove this 
/// file entirely once we upgrade cw-storage-plus.

pub trait KeyDeserialize {
    fn from_slice(key: &Vec<u8>) -> StdResult<Self>
    where
        Self: Sized;
}

impl KeyDeserialize for u64 {
    fn from_slice(key: &Vec<u8>) -> StdResult<u64> {
        Ok(u64::from_be_bytes(key.as_slice().try_into().map_err(
            |err: TryFromSliceError| StdError::generic_err(err.to_string()),
        )?))
    }
}

impl KeyDeserialize for String {
    fn from_slice(key: &Vec<u8>) -> StdResult<String> {
        String::from_utf8(key.clone()).map_err(StdError::invalid_utf8)
    }
}
