//! Offline Zcash payments on an NU7 (Ironwood, v6) chain.
//!
//! No networking lives here except the online wallet sync and the gateway's
//! JSON-RPC calls (`rpc`). Everything a device does offline (build, verify,
//! seal, sign) works without a connection.

pub mod builder;
pub mod binding;
pub mod envelope;
pub mod fleet;
pub mod messages;
pub mod rpc;
pub mod sync;
pub mod verify;
pub mod wallet;
mod wallet_tree;

pub use fleet::{DevicePublic, DeviceSecret, Fleet, Policy};

use zcash_protocol::{consensus::BlockHeight, local_consensus::LocalNetwork};

/// The `ths` Regtest chain: every upgrade through NU7 active at height 1.
pub fn regtest_network() -> LocalNetwork {
    let one = Some(BlockHeight::from_u32(1));
    LocalNetwork {
        overwinter: one,
        sapling: one,
        blossom: one,
        heartwood: one,
        canopy: one,
        nu5: one,
        nu6: one,
        nu6_1: one,
        nu6_2: one,
        nu6_3: one,
        nu7: one,
    }
}

/// Number of zatoshis in one ZEC.
pub const COIN: u64 = 100_000_000;

/// Formats zatoshis as a decimal ZEC string.
pub fn fmt_zec(zat: u64) -> String {
    format!("{}.{:08}", zat / COIN, zat % COIN)
}

/// Parses a decimal ZEC amount ("0.25") into zatoshis.
pub fn parse_zec(s: &str) -> anyhow::Result<u64> {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    anyhow::ensure!(frac.len() <= 8, "at most 8 decimal places");
    let whole: u64 = if whole.is_empty() { 0 } else { whole.parse()? };
    let frac: u64 = if frac.is_empty() { 0 } else { format!("{frac:0<8}").parse()? };
    whole.checked_mul(COIN).and_then(|w| w.checked_add(frac)).ok_or_else(|| anyhow::anyhow!("amount too large"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zec_round_trip() {
        assert_eq!(parse_zec("0.25").unwrap(), 25_000_000);
        assert_eq!(parse_zec("1").unwrap(), COIN);
        assert_eq!(parse_zec(".00000001").unwrap(), 1);
        assert!(parse_zec("184467440738").is_err(), "must not wrap to 0.90448384");
        assert!(parse_zec("0.000000001").is_err());
        assert_eq!(fmt_zec(123_456_789), "1.23456789");
    }
}
