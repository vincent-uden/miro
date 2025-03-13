use core::fmt;
use std::marker::PhantomData;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
};

pub fn serialize_point<S, T>(point: &iced::Point<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    use serde::ser::SerializeStruct;
    let mut state = serializer.serialize_struct("Point", 2)?;
    state.serialize_field("x", &point.x)?;
    state.serialize_field("y", &point.y)?;
    state.end()
}

pub fn deserialize_point<'de, D, T>(deserializer: D) -> Result<iced::Point<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    struct PointVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for PointVisitor<T>
    where
        T: Deserialize<'de> + Default,
    {
        type Value = iced::Point<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a Point with x and y fields")
        }

        fn visit_map<V>(self, mut map: V) -> Result<iced::Point<T>, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut x = None;
            let mut y = None;

            while let Some(key) = map.next_key()? {
                match key {
                    "x" => {
                        if x.is_some() {
                            return Err(de::Error::duplicate_field("x"));
                        }
                        x = Some(map.next_value()?);
                    }
                    "y" => {
                        if y.is_some() {
                            return Err(de::Error::duplicate_field("y"));
                        }
                        y = Some(map.next_value()?);
                    }
                    _ => {
                        let _ = map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
            }

            let x = x.unwrap_or_default();
            let y = y.unwrap_or_default();

            Ok(iced::Point { x, y })
        }
    }

    deserializer.deserialize_map(PointVisitor(PhantomData))
}

// Rectangle serialization functions
pub fn serialize_rectangle<S, T>(
    rect: &iced::Rectangle<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    use serde::ser::SerializeStruct;
    let mut state = serializer.serialize_struct("Rectangle", 4)?;
    state.serialize_field("x", &rect.x)?;
    state.serialize_field("y", &rect.y)?;
    state.serialize_field("width", &rect.width)?;
    state.serialize_field("height", &rect.height)?;
    state.end()
}

pub fn deserialize_rectangle<'de, D, T>(deserializer: D) -> Result<iced::Rectangle<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    struct RectangleVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for RectangleVisitor<T>
    where
        T: Deserialize<'de> + Default,
    {
        type Value = iced::Rectangle<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a Rectangle with x, y, width, and height fields")
        }

        fn visit_map<V>(self, mut map: V) -> Result<iced::Rectangle<T>, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut x = None;
            let mut y = None;
            let mut width = None;
            let mut height = None;

            while let Some(key) = map.next_key()? {
                match key {
                    "x" => {
                        if x.is_some() {
                            return Err(de::Error::duplicate_field("x"));
                        }
                        x = Some(map.next_value()?);
                    }
                    "y" => {
                        if y.is_some() {
                            return Err(de::Error::duplicate_field("y"));
                        }
                        y = Some(map.next_value()?);
                    }
                    "width" => {
                        if width.is_some() {
                            return Err(de::Error::duplicate_field("width"));
                        }
                        width = Some(map.next_value()?);
                    }
                    "height" => {
                        if height.is_some() {
                            return Err(de::Error::duplicate_field("height"));
                        }
                        height = Some(map.next_value()?);
                    }
                    _ => {
                        let _ = map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
            }

            let x = x.unwrap_or_default();
            let y = y.unwrap_or_default();
            let width = width.unwrap_or_default();
            let height = height.unwrap_or_default();

            Ok(iced::Rectangle {
                x,
                y,
                width,
                height,
            })
        }
    }

    deserializer.deserialize_map(RectangleVisitor(PhantomData))
}

pub mod option_point {
    use super::*;

    pub fn serialize<S, T>(
        option_point: &Option<iced::Point<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match option_point {
            Some(point) => {
                // Use the regular point serialization
                super::serialize_point(point, serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<iced::Point<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de> + Default,
    {
        struct OptionPointVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for OptionPointVisitor<T>
        where
            T: Deserialize<'de> + Default,
        {
            type Value = Option<iced::Point<T>>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("null or a Point with x and y fields")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                // Reuse the point deserializer
                let point = super::deserialize_point(deserializer)?;
                Ok(Some(point))
            }
        }

        deserializer.deserialize_option(OptionPointVisitor(PhantomData))
    }
}

// Color serialization function
pub fn serialize_color<S>(color: &iced::Color, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeStruct;
    let mut state = serializer.serialize_struct("Color", 4)?;
    state.serialize_field("r", &color.r)?;
    state.serialize_field("g", &color.g)?;
    state.serialize_field("b", &color.b)?;
    state.serialize_field("a", &color.a)?;
    state.end()
}

// Color deserialization function
pub fn deserialize_color<'de, D>(deserializer: D) -> Result<iced::Color, D::Error>
where
    D: Deserializer<'de>,
{
    struct ColorVisitor;

    impl<'de> Visitor<'de> for ColorVisitor {
        type Value = iced::Color;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a Color with r, g, b, and a fields")
        }

        fn visit_map<V>(self, mut map: V) -> Result<iced::Color, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut r = None;
            let mut g = None;
            let mut b = None;
            let mut a = None;

            while let Some(key) = map.next_key()? {
                match key {
                    "r" => {
                        if r.is_some() {
                            return Err(de::Error::duplicate_field("r"));
                        }
                        r = Some(map.next_value()?);
                    }
                    "g" => {
                        if g.is_some() {
                            return Err(de::Error::duplicate_field("g"));
                        }
                        g = Some(map.next_value()?);
                    }
                    "b" => {
                        if b.is_some() {
                            return Err(de::Error::duplicate_field("b"));
                        }
                        b = Some(map.next_value()?);
                    }
                    "a" => {
                        if a.is_some() {
                            return Err(de::Error::duplicate_field("a"));
                        }
                        a = Some(map.next_value()?);
                    }
                    _ => {
                        let _ = map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
            }

            let r = r.unwrap_or(0.0);
            let g = g.unwrap_or(0.0);
            let b = b.unwrap_or(0.0);
            let a = a.unwrap_or(1.0); // Default alpha to 1.0 (fully opaque)

            Ok(iced::Color { r, g, b, a })
        }
    }

    deserializer.deserialize_map(ColorVisitor)
}

#[allow(dead_code)]
// Optional color handling (similar to the Point and Rectangle implementations)
pub mod option_color {
    use super::*;

    pub fn serialize<S>(
        option_color: &Option<iced::Color>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match option_color {
            Some(color) => serializer.serialize_some(&ColorSerWrapper(color)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<iced::Color>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionColorVisitor;

        impl<'de> Visitor<'de> for OptionColorVisitor {
            type Value = Option<iced::Color>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("null or a Color with r, g, b, and a fields")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                let color = super::deserialize_color(deserializer)?;
                Ok(Some(color))
            }
        }

        deserializer.deserialize_option(OptionColorVisitor)
    }

    // Helper struct for serialization
    struct ColorSerWrapper<'a>(&'a iced::Color);

    impl Serialize for ColorSerWrapper<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            super::serialize_color(self.0, serializer)
        }
    }
}
