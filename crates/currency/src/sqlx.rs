//! Stores currencies as text columns holding the currency code.

use std::str::FromStr;

use sqlx::Database;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;

use crate::{CryptoCurrency, Currency};

/// Implements `Type`, `Encode` and `Decode` for a type that is stored as its `Display` text and
/// read back through `FromStr`.
macro_rules! text_column {
    ($currency:ty) => {
        impl<T> sqlx::Type<T> for $currency
        where
            T: Database,
            String: sqlx::Type<T>,
        {
            fn type_info() -> <T as Database>::TypeInfo {
                <String as sqlx::Type<T>>::type_info()
            }
        }

        impl<'q, T> sqlx::Encode<'q, T> for $currency
        where
            T: Database,
            String: sqlx::Encode<'q, T>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut <T as Database>::ArgumentBuffer<'q>,
            ) -> Result<IsNull, BoxDynError> {
                <String as sqlx::Encode<T>>::encode_by_ref(&self.to_string(), buf)
            }

            fn produces(&self) -> Option<<T as Database>::TypeInfo> {
                <String as sqlx::Encode<T>>::produces(&self.to_string())
            }

            fn size_hint(&self) -> usize {
                <String as sqlx::Encode<T>>::size_hint(&self.to_string())
            }
        }

        impl<'r, T> sqlx::Decode<'r, T> for $currency
        where
            T: Database,
            String: sqlx::Decode<'r, T>,
        {
            fn decode(value: <T as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
                let text = <String as sqlx::Decode<T>>::decode(value)?;
                <$currency>::from_str(&text).map_err(|_| format!("invalid currency: {text}").into())
            }
        }
    };
}

text_column!(CryptoCurrency);
text_column!(Currency);
