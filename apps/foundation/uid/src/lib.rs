#[cfg(feature = "encoding")]
mod encoding;

#[cfg(feature = "encoding")]
pub use crate::encoding::EncodedUid;

pub use uuid::Uuid;

#[macro_export]
macro_rules! typed_uid {
    (@internal [$($derive:path),*] $name:ident) => {
        #[derive(Copy, Clone $(, $derive)*)]
        pub struct $name(::uuid::Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl From<::uuid::Uuid> for $name {
            fn from(uid: ::uuid::Uuid) -> Self {
                Self(uid)
            }
        }

        impl ::std::ops::Deref for $name {
            type Target = ::uuid::Uuid;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }
    };

    (@internal [$($derive:path),*] $name:ident, $($rest:ident),+) => {
        typed_uid!(@internal [$($derive),*] $name);
        typed_uid!(@internal [$($derive),*] $($rest),+);
    };

    ($($derive:path),+ ; $($name:ident),+ $(,)?) => {
        typed_uid!(@internal [$($derive),+] $($name),+);
    };

    ($($name:ident),+ $(,)?) => {
        typed_uid!(@internal [] $($name),+);
    };
}
