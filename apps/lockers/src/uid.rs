use foundation_uid::{Uuid, typed_uid};
use serde::{Deserialize, Serialize};

typed_uid! {
    Eq, PartialEq, Hash, Serialize, Deserialize;

    LockerUid,
}
