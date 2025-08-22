use std::fmt;
use std::ops::Deref;

use base64::alphabet::URL_SAFE;
use base64::engine::general_purpose::NO_PAD;
use base64::engine::GeneralPurpose;
use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub struct EncodedUid<T>(pub T);

impl<T: Deref<Target = Uuid> + From<Uuid>> EncodedUid<T> {
    pub fn new(uid: T) -> Self {
        Self(uid)
    }
}

impl<T: Deref<Target = Uuid>> fmt::Display for EncodedUid<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self.0)
    }
}

impl<T: Deref<Target = Uuid>> Deref for EncodedUid<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Deref<Target = Uuid>> Serialize for EncodedUid<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let engine = GeneralPurpose::new(&URL_SAFE, NO_PAD);
        let encoded = engine.encode(self.0.deref().as_bytes());

        serializer.serialize_str(&encoded)
    }
}

impl<'de, T: From<Uuid>> Deserialize<'de> for EncodedUid<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let engine = GeneralPurpose::new(&URL_SAFE, NO_PAD);
        let encoded = String::deserialize(deserializer)?;
        let decoded = engine.decode(encoded).map_err(serde::de::Error::custom)?;

        let uid = Uuid::from_slice(&decoded).map_err(serde::de::Error::custom)?;

        Ok(Self(T::from(uid)))
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::typed_uid;

    typed_uid! {
        serde::Serialize, serde::Deserialize;

        AccountUid,
    }

    #[test]
    fn can_serialize_and_deserialize_encoded_uids() {
        let encoded_uid = AccountUid::new();

        let serialized = serde_json::to_string(&encoded_uid).unwrap();
        assert!(!serialized.is_empty());

        let deserialized: AccountUid = serde_json::from_str(&serialized).unwrap();
        assert_eq!(encoded_uid.0, deserialized.0);
    }
}
