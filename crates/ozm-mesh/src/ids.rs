use std::{fmt, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

macro_rules! hex_id {
    ($(#[$doc:meta])* $name:ident, $len:expr) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name(pub [u8; $len]);

        impl $name {
            /// A fresh random id.
            pub fn random() -> Self {
                Self(rand::random())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&hex::encode(self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self)
            }
        }

        impl FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                let bytes = hex::decode(s)?;
                let arr: [u8; $len] = bytes.try_into().map_err(|_| {
                    anyhow::anyhow!("{} must be {} hex characters", stringify!($name), $len * 2)
                })?;
                Ok(Self(arr))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                s.parse().map_err(D::Error::custom)
            }
        }
    };
}

hex_id!(
    /// A mesh node id: 8 bytes, shown as 16 hex characters.
    PeerId,
    8
);
hex_id!(
    /// A packet id: 16 random bytes, shown as 32 hex characters.
    MsgId,
    16
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let p = PeerId([1, 2, 3, 4, 5, 6, 7, 0xff]);
        assert_eq!(p.to_string(), "01020304050607ff");
        assert_eq!("01020304050607ff".parse::<PeerId>().unwrap(), p);
        assert!("0102".parse::<PeerId>().is_err());
        let m = MsgId::random();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json.len(), 34);
        assert_eq!(serde_json::from_str::<MsgId>(&json).unwrap(), m);
    }
}
