//! Schnorr proof of the address's incoming-viewing scalar: pk_d = ivk · DiversifyHash(d).
//! This proves receiving authority, not spending authority, without sharing a viewing key.
use anyhow::{anyhow, ensure, Result};
use ff::{Field, PrimeField, FromUniformBytes};
use group::{Group, GroupEncoding};
use orchard::keys::Scope;
use pasta_curves::{arithmetic::CurveExt, pallas};

use crate::{DevicePublic, DeviceSecret};

fn base(device: &DevicePublic) -> Result<pallas::Point> {
    let address = device.orchard_address()?;
    let hash = pallas::Point::hash_to_curve("z.cash:Orchard-gd");
    let g = hash(address.diversifier().as_array());
    Ok(if bool::from(g.is_identity()) { hash(&[]) } else { g })
}

fn challenge(device: &DevicePublic, r: &[u8; 32]) -> Result<pallas::Scalar> {
    let mut bytes = b"ozm1-address-owner-v1".to_vec();
    bytes.extend(bincode::serialize(device)?);
    bytes.extend(r);
    Ok(pallas::Scalar::from_uniform_bytes(blake2b_simd::blake2b(&bytes).as_array()))
}

pub fn prove(me: &DeviceSecret) -> Result<String> {
    prove_on(me, crate::fleet::Network::Regtest)
}

pub fn prove_on(me: &DeviceSecret, network: crate::fleet::Network) -> Result<String> {
    let device = me.public_on(network)?;
    let ivk = me.fvk()?.to_ivk(Scope::External).to_bytes();
    let x = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(ivk[32..].try_into()?))
        .ok_or_else(|| anyhow!("invalid incoming scalar"))?;
    let nonce = pallas::Scalar::random(&mut rand::rand_core::UnwrapErr(rand::rngs::SysRng));
    let r = (base(&device)? * nonce).to_bytes();
    let s = nonce + challenge(&device, &r)? * x;
    Ok(hex::encode([r.as_slice(), s.to_repr().as_slice()].concat()))
}

pub fn verify(device: &DevicePublic, proof: &str) -> Result<()> {
    let bytes = hex::decode(proof)?;
    ensure!(bytes.len() == 64, "address proof length");
    let r_bytes: [u8; 32] = bytes[..32].try_into()?;
    let r = Option::<pallas::Point>::from(pallas::Point::from_bytes(&r_bytes))
        .ok_or_else(|| anyhow!("invalid proof point"))?;
    let s = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(bytes[32..].try_into()?))
        .ok_or_else(|| anyhow!("invalid proof scalar"))?;
    let address = device.orchard_address()?.to_raw_address_bytes();
    let pk = Option::<pallas::Point>::from(pallas::Point::from_bytes(address[11..].try_into()?))
        .ok_or_else(|| anyhow!("invalid address point"))?;
    ensure!(!bool::from(r.is_identity()) && !bool::from(pk.is_identity()), "identity proof point");
    ensure!(base(device)? * s == r + pk * challenge(device, &r_bytes)?, "address ownership proof failed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_receiving_authority_binds_the_mesh_identity() {
        let owner = DeviceSecret::generate("owner", "payee");
        let thief = DeviceSecret::generate("thief", "payee");
        let proof = prove(&owner).unwrap();
        let mut device = owner.public().unwrap();
        verify(&device, &proof).unwrap();
        device.ed25519 = thief.public().unwrap().ed25519;
        assert!(verify(&device, &proof).is_err());
        device = thief.public().unwrap();
        device.address = owner.public().unwrap().address;
        assert!(verify(&device, &prove(&thief).unwrap()).is_err());
        assert!(verify(&owner.public().unwrap(), &"00".repeat(64)).is_err());
    }
}
