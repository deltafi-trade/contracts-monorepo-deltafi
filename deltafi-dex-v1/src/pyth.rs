#![allow(missing_docs)]
/// Derived from https://github.com/project-serum/anchor/blob/9224e0fa99093943a6190e396bccbc3387e5b230/examples/pyth/programs/pyth/src/pc.rs
use bytemuck::{
    cast_slice, cast_slice_mut, from_bytes, from_bytes_mut, try_cast_slice, try_cast_slice_mut,
    Pod, PodCastError, Zeroable,
};

/// pyth program id, should be permanent and used for verifying the legitimacy
/// of price/product accounts
/// this is mainnet-beta address
pub const PYTH_PROGRAM_ID: &str = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH";

use std::mem::size_of;

pub const MAGIC: u32 = 0xa1b2c3d4;
pub const VERSION_2: u32 = 2;
pub const VERSION: u32 = VERSION_2;
pub const MAP_TABLE_SIZE: usize = 640;
pub const PROD_ACCT_SIZE: usize = 512;
pub const PROD_HDR_SIZE: usize = 48;
pub const PROD_ATTR_SIZE: usize = PROD_ACCT_SIZE - PROD_HDR_SIZE;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct AccKey {
    pub val: [u8; 32],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum AccountType {
    Unknown,
    Mapping,
    Product,
    Price,
}

#[derive(PartialEq, Copy, Clone)]
#[repr(C)]
pub enum PriceStatus {
    Unknown,
    Trading,
    Halted,
    Auction,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum CorpAction {
    NoCorpAct,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PriceInfo {
    pub price: i64,
    pub conf: u64,
    pub status: PriceStatus,
    pub corp_act: CorpAction,
    pub pub_slot: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PriceComp {
    publisher: AccKey,
    agg: PriceInfo,
    latest: PriceInfo,
}

impl PriceComp {
    pub fn is_active(&self) -> bool {
        self.agg.status == PriceStatus::Trading
    }

    pub fn new(publisher: AccKey, agg: PriceInfo, latest: PriceInfo) -> Self {
        PriceComp {
            publisher,
            agg,
            latest,
        }
    }
}

#[derive(PartialEq, Copy, Clone)]
#[repr(C)]
pub enum PriceType {
    Unknown,
    Price,
}

/// An exponentially-weighted moving average.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Ema {
    /// The current value of the EMA
    pub val: i64,
    /// numerator state for next update
    numer: i64,
    /// denominator state for next update
    denom: i64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Price {
    /// pyth magic number
    pub magic: u32,
    /// program version
    pub ver: u32,
    /// account type
    pub atype: u32,
    /// price account size
    pub size: u32,
    /// price or calculation type
    pub ptype: PriceType,
    /// price exponent
    pub expo: i32,
    /// number of component prices
    pub num: u32,
    /// number of quoters that make up aggregate
    pub num_qt: u32,
    /// slot of last valid (not unknown) aggregate price
    pub last_slot: u64,
    /// valid slot-time of agg. price
    pub valid_slot: u64,
    /// time-weighted average price
    pub twap: Ema,
    /// time-weighted average confidence interval
    pub twac: Ema,
    /// space for future derived values
    pub drv1: i64,
    /// space for future derived values
    pub drv2: i64,
    /// product account key
    pub prod: AccKey,
    /// next Price account in linked list
    pub next: AccKey,
    /// valid slot of previous update
    pub prev_slot: u64,
    /// aggregate price of previous update
    pub prev_price: i64,
    /// confidence interval of previous update
    pub prev_conf: u64,
    /// space for future derived values
    pub drv3: i64,
    /// aggregate price info
    pub agg: PriceInfo,
    /// price components one per quoter
    pub comp: [PriceComp; 32],
}

#[cfg(target_endian = "little")]
unsafe impl Zeroable for Price {}

#[cfg(target_endian = "little")]
unsafe impl Pod for Price {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Product {
    pub magic: u32,                 // pyth magic number
    pub ver: u32,                   // program version
    pub atype: u32,                 // account type
    pub size: u32,                  // price account size
    pub px_acc: AccKey,             // first price account in list
    pub attr: [u8; PROD_ATTR_SIZE], // key/value pairs of reference attr.
}

#[cfg(target_endian = "little")]
unsafe impl Zeroable for Product {}

#[cfg(target_endian = "little")]
unsafe impl Pod for Product {}

pub fn load<T: Pod>(data: &[u8]) -> Result<&T, PodCastError> {
    let size = size_of::<T>();
    if size > data.len() {
        return Err(PodCastError::SizeMismatch);
    }
    Ok(from_bytes(cast_slice::<u8, u8>(try_cast_slice(
        &data[0..size],
    )?)))
}

pub fn load_mut<T: Pod>(data: &mut [u8]) -> Result<&mut T, PodCastError> {
    let size = size_of::<T>();
    if size > data.len() {
        return Err(PodCastError::SizeMismatch);
    }
    Ok(from_bytes_mut(cast_slice_mut::<u8, u8>(
        try_cast_slice_mut(&mut data[0..size])?,
    )))
}

#[cfg(test)]
mod test {
    use super::*;
    use bytemuck::bytes_of_mut;

    #[test]
    fn test_load() {
        let mut pyth_price_data_vec = vec![0u8];
        let pyth_price_data = [0u8; std::mem::size_of::<Price>() * 2];
        pyth_price_data_vec.extend_from_slice(&pyth_price_data);

        let price_align = std::mem::align_of::<Price>();
        let shift = price_align - ((pyth_price_data_vec.as_ptr() as usize) % price_align);

        assert!(matches!(
            load::<Price>(&pyth_price_data_vec[shift..shift + price_align + 1]),
            Err(PodCastError::SizeMismatch)
        ));

        assert!(matches!(
            load_mut::<Price>(&mut pyth_price_data_vec[shift..shift + price_align]),
            Err(PodCastError::SizeMismatch)
        ));

        assert!(load::<Price>(
            &pyth_price_data_vec[shift..shift + std::mem::size_of::<Price>() + 1]
        )
        .is_ok());

        let mut _pyth_price = load_mut::<Price>(
            &mut pyth_price_data_vec[shift..shift + std::mem::size_of::<Price>()],
        );
        assert!(_pyth_price.is_ok());
        let mut pyth_price = _pyth_price.unwrap();

        pyth_price.num = 999_999u32;
        let pyth_price_bytes = bytes_of_mut(pyth_price);

        let mut _pyth_price = load_mut::<Price>(pyth_price_bytes);
        assert!(_pyth_price.is_ok());
        assert_eq!(_pyth_price.unwrap().num, 999_999u32);
    }
}
