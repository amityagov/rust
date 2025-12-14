use std::str::FromStr;
use sqlx::Database;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use crate::{CryptoCurrency, Currency};

impl<'r, T> sqlx::Decode<'r, T> for CryptoCurrency
where
    T: Database,
    String: sqlx::Decode<'r, T>,
{
    fn decode(
        value: <T as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<T>>::decode(value)?;
        CryptoCurrency::from_str(&s).map_err(|_| format!("Invalid crypto currency: {s}").into())
    }
}

impl<'q, T> sqlx::Encode<'q, T> for CryptoCurrency
where
    T: Database,
    String: sqlx::Encode<'q, T>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <T as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError> {
        <String as sqlx::Encode<T>>::encode_by_ref(&self.to_string(), buf)
    }

    fn produces(&self) -> Option<<T as sqlx::Database>::TypeInfo> {
        <String as sqlx::Encode<T>>::produces(&self.to_string())
    }

    fn size_hint(&self) -> usize {
        <String as sqlx::Encode<T>>::size_hint(&self.to_string())
    }
}

impl<T: Database> sqlx::Type<T> for CryptoCurrency
where
    String: sqlx::Type<T>,
{
    fn type_info() -> <T as sqlx::Database>::TypeInfo {
        <String as sqlx::Type<T>>::type_info()
    }
}

impl<'r, T> sqlx::Decode<'r, T> for Currency
where
    T: Database,
    String: sqlx::Decode<'r, T>,
{
    fn decode(value: <T as Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<T>>::decode(value)?;

        if let Ok(c) = iso_currency::Currency::from_str(&s) {
            return Ok(Currency::Fiat(c));
        } else if let Ok(c) = CryptoCurrency::from_str(&s) {
            return Ok(Currency::Crypto(c));
        }
        Err(format!("Invalid currency: {s}").into())
    }
}

impl<'q, T> sqlx::Encode<'q, T> for Currency
where
    T: Database,
    String: sqlx::Encode<'q, T>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <T as Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, BoxDynError> {
        let value = self.to_string();
        <String as sqlx::Encode<T>>::encode_by_ref(&value, buf)
    }

    fn produces(&self) -> Option<<T as sqlx::Database>::TypeInfo> {
        let value = self.to_string();

        <String as sqlx::Encode<T>>::produces(&value)
    }

    fn size_hint(&self) -> usize {
        let value = self.to_string();
        <String as sqlx::Encode<T>>::size_hint(&value)
    }
}

impl<T> sqlx::Type<T> for Currency
where
    T: Database,
    String: sqlx::Type<T>,
{
    fn type_info() -> <T as Database>::TypeInfo {
        <String as sqlx::Type<T>>::type_info()
    }
}
