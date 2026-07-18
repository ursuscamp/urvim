//! Shared conversion from BearScript runtime values into application values.

use std::fmt;

use bearscript::{CowList, CowMap, Value};

/// A conversion failure with the path of the invalid BearScript value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BearValueError {
    path: String,
    message: String,
}

impl BearValueError {
    /// Creates an error for `path`.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Returns the path of the invalid value.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for BearValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "{} {}", self.path, self.message)
        }
    }
}

impl std::error::Error for BearValueError {}

/// A borrowed BearScript value carrying its location in the input.
#[derive(Clone, Debug)]
pub struct BearValueRef<'a> {
    value: &'a Value,
    path: String,
}

impl<'a> BearValueRef<'a> {
    /// Creates a reference rooted at `path`.
    pub fn new(value: &'a Value, path: impl Into<String>) -> Self {
        Self {
            value,
            path: path.into(),
        }
    }

    /// Returns the underlying BearScript value.
    pub fn value(&self) -> &'a Value {
        self.value
    }

    /// Returns the path to this value.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns whether this is BearScript `null`.
    pub fn is_null(&self) -> bool {
        matches!(self.value, Value::Null)
    }

    /// Reads this value as a string.
    pub fn string(self) -> Result<String, BearValueError> {
        let Value::String(value) = self.value else {
            return Err(self.error("must be a string"));
        };
        Ok(value.to_string())
    }

    /// Reads this value as a boolean.
    pub fn boolean(self) -> Result<bool, BearValueError> {
        let Value::Bool(value) = self.value else {
            return Err(self.error("must be a bool"));
        };
        Ok(*value)
    }

    /// Reads this value as a number and retains its path for checked conversion.
    pub fn number(self) -> Result<BearNumber, BearValueError> {
        let Value::Number(value) = self.value else {
            return Err(self.error("must be a number"));
        };
        Ok(BearNumber::new(*value, self.path))
    }

    /// Reads this value as a list.
    pub fn list(self) -> Result<BearListRef<'a>, BearValueError> {
        let Value::List(values) = self.value else {
            return Err(self.error("must be a list"));
        };
        Ok(BearListRef {
            values,
            path: self.path,
        })
    }

    /// Reads this value as a map.
    pub fn map(self) -> Result<BearMapRef<'a>, BearValueError> {
        let Value::Map(map) = self.value else {
            return Err(self.error("must be a map"));
        };
        Ok(BearMapRef {
            map,
            path: self.path,
        })
    }

    fn error(&self, message: impl Into<String>) -> BearValueError {
        BearValueError::new(self.path.clone(), message)
    }
}

/// A BearScript number carrying the path used in conversion errors.
#[derive(Clone, Debug)]
pub struct BearNumber {
    value: f64,
    path: String,
}

impl BearNumber {
    /// Creates a number rooted at `path`.
    pub fn new(value: f64, path: impl Into<String>) -> Self {
        Self {
            value,
            path: path.into(),
        }
    }

    /// Returns the underlying number.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Converts this number to a non-negative `usize`.
    pub fn non_negative_usize(self) -> Result<usize, BearValueError> {
        self.checked_integer(0.0, usize::MAX as f64, "must be a non-negative integer")
            .map(|value| value as usize)
    }

    /// Converts this number to a non-negative `u64`.
    pub fn non_negative_u64(self) -> Result<u64, BearValueError> {
        self.checked_integer(0.0, u64::MAX as f64, "must be a non-negative integer")
            .map(|value| value as u64)
    }

    /// Converts this number to a non-negative `u16`.
    pub fn non_negative_u16(self) -> Result<u16, BearValueError> {
        self.checked_integer(0.0, u16::MAX as f64, "must be a non-negative integer")
            .map(|value| value as u16)
    }

    /// Converts this number to a positive `u16`.
    pub fn positive_u16(self) -> Result<u16, BearValueError> {
        self.checked_integer(1.0, u16::MAX as f64, "must be a positive integer")
            .map(|value| value as u16)
    }

    /// Converts this number to an integer from zero through 255.
    pub fn byte(self) -> Result<u8, BearValueError> {
        self.checked_integer(0.0, u8::MAX as f64, "must be an integer from 0 to 255")
            .map(|value| value as u8)
    }

    fn checked_integer(
        self,
        minimum: f64,
        maximum: f64,
        expectation: &str,
    ) -> Result<f64, BearValueError> {
        if !self.value.is_finite()
            || self.value < minimum
            || self.value.fract() != 0.0
            || self.value > maximum
        {
            return Err(BearValueError::new(self.path, expectation));
        }
        Ok(self.value)
    }
}

/// A borrowed BearScript list that creates paths for its elements.
#[derive(Clone, Debug)]
pub struct BearListRef<'a> {
    values: &'a CowList,
    path: String,
}

impl<'a> BearListRef<'a> {
    /// Iterates over values with indexed paths.
    pub fn iter(&self) -> impl Iterator<Item = BearValueRef<'a>> + '_ {
        self.values
            .iter()
            .enumerate()
            .map(|(index, value)| BearValueRef::new(value, format!("{}[{index}]", self.path)))
    }
}

/// A borrowed BearScript map that creates paths for its fields.
#[derive(Clone, Debug)]
pub struct BearMapRef<'a> {
    map: &'a CowMap,
    path: String,
}

impl<'a> BearMapRef<'a> {
    /// Returns a required field.
    pub fn required(&self, key: &str) -> Result<BearValueRef<'a>, BearValueError> {
        self.optional(key)?
            .ok_or_else(|| BearValueError::new(self.path.clone(), format!("requires {key}")))
    }

    /// Returns an optional field, distinguishing a present `null` value.
    pub fn optional(&self, key: &str) -> Result<Option<BearValueRef<'a>>, BearValueError> {
        Ok(self
            .map
            .get(key)
            .map(|value| BearValueRef::new(value, self.field_path(key))))
    }

    /// Rejects keys not included in `allowed`.
    pub fn reject_unknown(&self, allowed: &[&str]) -> Result<(), BearValueError> {
        if let Some(key) = self.map.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(BearValueError::new(
                self.field_path(key),
                "is not a recognized field",
            ));
        }
        Ok(())
    }

    /// Iterates over fields with their paths.
    pub fn iter(&self) -> impl Iterator<Item = (&str, BearValueRef<'a>)> + '_ {
        self.map
            .iter()
            .map(|(key, value)| (key.as_str(), BearValueRef::new(value, self.field_path(key))))
    }

    fn field_path(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{key}", self.path)
        }
    }
}

/// Converts a borrowed BearScript value into an application value.
pub trait FromBearValue: Sized {
    /// Converts `value`, returning a path-aware error when its shape is invalid.
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError>;
}

impl FromBearValue for Value {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        Ok(value.value().clone())
    }
}

impl FromBearValue for String {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.string()
    }
}

impl FromBearValue for bool {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.boolean()
    }
}

impl FromBearValue for f64 {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.number().map(|number| number.value())
    }
}

impl FromBearValue for usize {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.number()?.non_negative_usize()
    }
}

impl FromBearValue for u64 {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.number()?.non_negative_u64()
    }
}

impl FromBearValue for u16 {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value.number()?.non_negative_u16()
    }
}

impl<T: FromBearValue> FromBearValue for Option<T> {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_bear(value).map(Some)
        }
    }
}

impl<T: FromBearValue> FromBearValue for Vec<T> {
    fn from_bear(value: BearValueRef<'_>) -> Result<Self, BearValueError> {
        value
            .list()?
            .iter()
            .map(T::from_bear)
            .collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn nested_readers_include_fields_and_indices_in_errors() {
        let value = Value::Map(
            HashMap::from([(
                "sections".to_string(),
                Value::List(vec![Value::Map(HashMap::new().into())].into()),
            )])
            .into(),
        );

        let sections = BearValueRef::new(&value, "options")
            .map()
            .unwrap()
            .required("sections")
            .unwrap()
            .list()
            .unwrap();
        let error = sections
            .iter()
            .next()
            .unwrap()
            .map()
            .unwrap()
            .required("width")
            .unwrap_err();

        assert_eq!(error.to_string(), "options.sections[0] requires width");
        assert_eq!(error.path(), "options.sections[0]");
    }

    #[test]
    fn recursive_conversion_reports_invalid_list_item() {
        let value = Value::List(
            vec![
                Value::String("one".into()),
                Value::Number(2.0),
                Value::String("three".into()),
            ]
            .into(),
        );

        let error = Vec::<String>::from_bear(BearValueRef::new(&value, "items")).unwrap_err();

        assert_eq!(error.to_string(), "items[1] must be a string");
    }

    #[test]
    fn option_treats_null_as_absent() {
        assert_eq!(
            Option::<String>::from_bear(BearValueRef::new(&Value::Null, "title")).unwrap(),
            None
        );
    }

    #[test]
    fn checked_integers_reject_invalid_numbers() {
        for value in [f64::NAN, f64::INFINITY, -1.0, 1.5, u16::MAX as f64 + 1.0] {
            assert!(BearNumber::new(value, "size").non_negative_u16().is_err());
        }
        assert_eq!(
            BearNumber::new(u16::MAX as f64, "size")
                .non_negative_u16()
                .unwrap(),
            u16::MAX
        );
    }

    #[test]
    fn positive_integer_rejects_zero() {
        assert_eq!(
            BearNumber::new(0.0, "ratio.first")
                .positive_u16()
                .unwrap_err()
                .to_string(),
            "ratio.first must be a positive integer"
        );
    }

    #[test]
    fn map_distinguishes_missing_and_present_null_fields() {
        let value = Value::Map(HashMap::from([("title".to_string(), Value::Null)]).into());
        let map = BearValueRef::new(&value, "options").map().unwrap();

        assert!(map.optional("missing").unwrap().is_none());
        assert!(map.optional("title").unwrap().unwrap().is_null());
    }

    #[test]
    fn map_rejects_unknown_fields_with_their_path() {
        let value = Value::Map(HashMap::from([("widht".to_string(), Value::Number(4.0))]).into());
        let map = BearValueRef::new(&value, "overlay").map().unwrap();

        assert_eq!(
            map.reject_unknown(&["width"]).unwrap_err().to_string(),
            "overlay.widht is not a recognized field"
        );
    }
}
