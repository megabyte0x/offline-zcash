//! Mesh packets and the link messages that carry them. See the crate README for the layout.

use anyhow::{bail, ensure, Result};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use crate::{MsgId, PeerId};

pub(crate) const VERSION: u8 = 1;
/// Fixed packet header size before the payload.
pub(crate) const HEADER_LEN: usize = 48;

const LINK_PACKET: u8 = 0x10;
const LINK_INVENTORY: u8 = 0x11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Message,
    Broadcast,
    Ack,
}

impl Kind {
    fn to_u8(self) -> u8 {
        match self {
            Kind::Message => 1,
            Kind::Broadcast => 2,
            Kind::Ack => 3,
        }
    }

    fn from_u8(b: u8) -> Result<Self> {
        Ok(match b {
            1 => Kind::Message,
            2 => Kind::Broadcast,
            3 => Kind::Ack,
            other => bail!("unknown packet type {other}"),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Packet {
    pub kind: Kind,
    /// Remaining links this copy may still travel (1 = deliver here, do not forward).
    pub ttl: u8,
    /// Links this copy has travelled when received (1 = from the originator directly).
    pub hops: u8,
    pub msg_id: MsgId,
    pub sender: PeerId,
    /// Zero for broadcasts.
    pub recipient: PeerId,
    pub created_ms: u64,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_LEN + self.payload.len());
        b.push(VERSION);
        b.push(self.kind.to_u8());
        b.push(self.ttl);
        b.push(self.hops);
        b.extend_from_slice(&self.msg_id.0);
        b.extend_from_slice(&self.sender.0);
        b.extend_from_slice(&self.recipient.0);
        b.extend_from_slice(&self.created_ms.to_be_bytes());
        b.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        b.extend_from_slice(&self.payload);
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self> {
        let mut r = Reader(b);
        let version = r.u8()?;
        ensure!(version == VERSION, "unsupported packet version {version}");
        let kind = Kind::from_u8(r.u8()?)?;
        let ttl = r.u8()?;
        let hops = r.u8()?;
        let msg_id = MsgId(r.array()?);
        let sender = PeerId(r.array()?);
        let recipient = PeerId(r.array()?);
        let created_ms = u64::from_be_bytes(r.array()?);
        let len = u32::from_be_bytes(r.array()?) as usize;
        let payload = r.take(len)?.to_vec();
        ensure!(r.0.is_empty(), "trailing bytes after packet");
        Ok(Packet { kind, ttl, hops, msg_id, sender, recipient, created_ms, payload })
    }
}

/// Persisted as the hex of the wire encoding.
impl Serialize for Packet {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(self.encode()))
    }
}

impl<'de> Deserialize<'de> for Packet {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let bytes = hex::decode(s).map_err(D::Error::custom)?;
        Packet::decode(&bytes).map_err(D::Error::custom)
    }
}

/// What a link carries once fragments are reassembled.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkMsg {
    Packet(Packet),
    /// Sent once when a link comes up: ids this node has already seen, and message ids it knows were acked.
    Inventory { held: Vec<MsgId>, acked: Vec<MsgId> },
}

impl LinkMsg {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            LinkMsg::Packet(p) => {
                let mut b = vec![LINK_PACKET];
                b.extend(p.encode());
                b
            }
            LinkMsg::Inventory { held, acked } => {
                let mut b = vec![LINK_INVENTORY];
                for ids in [held, acked] {
                    b.extend_from_slice(&(ids.len() as u32).to_be_bytes());
                    for id in ids {
                        b.extend_from_slice(&id.0);
                    }
                }
                b
            }
        }
    }

    pub fn decode(b: &[u8]) -> Result<Self> {
        let mut r = Reader(b);
        match r.u8()? {
            LINK_PACKET => Ok(LinkMsg::Packet(Packet::decode(r.0)?)),
            LINK_INVENTORY => {
                let mut lists = [Vec::new(), Vec::new()];
                for list in &mut lists {
                    let n = u32::from_be_bytes(r.array()?) as usize;
                    ensure!(n.saturating_mul(16) <= r.0.len(), "inventory truncated");
                    for _ in 0..n {
                        list.push(MsgId(r.array()?));
                    }
                }
                ensure!(r.0.is_empty(), "trailing bytes after inventory");
                let [held, acked] = lists;
                Ok(LinkMsg::Inventory { held, acked })
            }
            other => bail!("unknown link message {other:#x}"),
        }
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure!(self.0.len() >= n, "truncated");
        let (a, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let p = Packet {
            kind: Kind::Ack,
            ttl: 7,
            hops: 2,
            msg_id: MsgId::random(),
            sender: PeerId::random(),
            recipient: PeerId::random(),
            created_ms: 1_234_567,
            payload: vec![9; 100],
        };
        let enc = p.encode();
        assert_eq!(enc.len(), HEADER_LEN + 100);
        assert_eq!(Packet::decode(&enc).unwrap(), p);
        assert!(Packet::decode(&enc[..enc.len() - 1]).is_err());
        let m = LinkMsg::Packet(p);
        assert_eq!(LinkMsg::decode(&m.encode()).unwrap(), m);
        let inv = LinkMsg::Inventory { held: vec![MsgId::random()], acked: vec![MsgId::random(); 2] };
        assert_eq!(LinkMsg::decode(&inv.encode()).unwrap(), inv);
    }
}
