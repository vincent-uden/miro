use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use num::Num;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Vector<T> {
    pub x: T,
    pub y: T,
}

impl<T> Vector<T>
where
    T: Num + Copy,
{
    pub fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
    pub fn scale(&mut self, scale: T) {
        self.x = self.x * scale;
        self.y = self.y * scale;
    }

    pub fn scaled(self, scale: T) -> Vector<T> {
        let mut out = self;
        out.scale(scale);
        out
    }

    pub fn non_uniform_scale(&mut self, scale: Vector<T>) {
        self.x = self.x * scale.x;
        self.y = self.y * scale.y;
    }

    pub fn non_uniform_scaled(self, scale: Vector<T>) -> Vector<T> {
        let mut out = self;
        out.non_uniform_scale(scale);
        out
    }

    pub fn div_inverted(self) -> Self {
        Self {
            x: T::one() / self.x,
            y: T::one() / self.y,
        }
    }

    pub fn zero() -> Vector<T> {
        Self {
            x: T::zero(),
            y: T::zero(),
        }
    }
}

impl<T> Add for Vector<T>
where
    T: Num + Copy,
{
    type Output = Vector<T>;

    fn add(self, rhs: Self) -> Self::Output {
        let mut out = self;
        out.add_assign(rhs);
        out
    }
}

impl<T> AddAssign for Vector<T>
where
    T: Num + Copy,
{
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
    }
}

impl<T> Sub for Vector<T>
where
    T: Num + Copy,
{
    type Output = Vector<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        let mut out = self;
        out.sub_assign(rhs);
        out
    }
}

impl<T> SubAssign for Vector<T>
where
    T: Num + Copy,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
    }
}

impl<T> Neg for Vector<T>
where
    T: Num + Copy + Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl From<Vector<f32>> for Vector<i32> {
    fn from(value: Vector<f32>) -> Self {
        Vector {
            x: value.x.round() as i32,
            y: value.y.round() as i32,
        }
    }
}

impl From<Vector<f32>> for iced::Point {
    fn from(val: Vector<f32>) -> Self {
        iced::Point::new(val.x, val.y)
    }
}

impl From<Vector<f32>> for iced::Vector {
    fn from(val: Vector<f32>) -> Self {
        iced::Vector::new(val.x, val.y)
    }
}

impl From<Vector<f32>> for iced::Size {
    fn from(val: Vector<f32>) -> Self {
        iced::Size::new(val.x, val.y)
    }
}

impl From<iced::Point> for Vector<f32> {
    fn from(value: iced::Point) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl From<iced::Vector> for Vector<f32> {
    fn from(value: iced::Vector) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl From<iced::Size> for Vector<f32> {
    fn from(value: iced::Size) -> Self {
        Self {
            x: value.width,
            y: value.height,
        }
    }
}

impl From<mupdf::Size> for Vector<f32> {
    fn from(value: mupdf::Size) -> Self {
        Self {
            x: value.width,
            y: value.height,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect<T> {
    /// Top left
    pub x0: Vector<T>,
    /// Bottom right
    pub x1: Vector<T>,
}

impl<T> Rect<T>
where
    T: Num + Copy + std::fmt::Debug + PartialOrd,
{
    pub fn from_points(top_left: Vector<T>, bottom_right: Vector<T>) -> Self {
        Self {
            x0: top_left,
            x1: bottom_right,
        }
    }

    pub fn from_pos_size(pos: Vector<T>, size: Vector<T>) -> Self {
        Self {
            x0: pos,
            x1: pos + size,
        }
    }

    pub fn center(&self) -> Vector<T> {
        (self.x0 + self.x1).scaled(T::one() / (T::one() + T::one()))
    }

    pub fn width(&self) -> T {
        self.x1.x - self.x0.x
    }

    pub fn height(&self) -> T {
        self.x1.y - self.x0.y
    }

    pub fn size(&self) -> Vector<T> {
        Vector::new(self.width(), self.height())
    }

    pub fn translate(&mut self, offset: Vector<T>) {
        self.x0 += offset;
        self.x1 += offset;
    }

    /// Scales the rectangle around it's center point
    pub fn scale(&mut self, s: T) {
        let x0 = self.x0;
        let x1 = self.x1;
        self.x0 = (x0.scaled(T::one() + s) + x1.scaled(T::one() - s))
            .scaled(T::one() / (T::one() + T::one()));
        self.x1 = (x0.scaled(T::one() - s) + x1.scaled(T::one() + s))
            .scaled(T::one() / (T::one() + T::one()));
    }

    /// Returns a new rectangle scaled around its center point
    pub fn scaled(&self, s: T) -> Self {
        let mut out = *self;
        out.scale(s);
        out
    }

    pub fn contains(&self, v: Vector<T>) -> bool {
        self.x0.x < v.x && self.x1.x > v.x && self.x0.y < v.y && self.x1.y > v.y
    }
}

impl From<Rect<f32>> for mupdf::Rect {
    fn from(val: Rect<f32>) -> Self {
        mupdf::Rect::new(val.x0.x, val.x0.y, val.x1.x, val.x1.y)
    }
}

impl From<Rect<i32>> for mupdf::IRect {
    fn from(val: Rect<i32>) -> Self {
        mupdf::IRect::new(val.x0.x, val.x0.y, val.x1.x, val.x1.y)
    }
}

impl From<Rect<f32>> for mupdf::IRect {
    fn from(val: Rect<f32>) -> Self {
        let irect: Rect<i32> = val.into();
        irect.into()
    }
}

impl From<Rect<f32>> for Rect<i32> {
    fn from(val: Rect<f32>) -> Self {
        Rect {
            x0: Vector {
                x: val.x0.x.round() as i32,
                y: val.x0.y.round() as i32,
            },
            x1: Vector {
                x: val.x1.x.round() as i32,
                y: val.x1.y.round() as i32,
            },
        }
    }
}

impl From<iced::Rectangle> for Rect<f32> {
    fn from(value: iced::Rectangle) -> Self {
        let top_left: Vector<f32> = value.position().into();
        let size: Vector<f32> = value.size().into();
        Self {
            x0: top_left,
            x1: top_left + size,
        }
    }
}

impl From<mupdf::Rect> for Rect<f32> {
    fn from(value: mupdf::Rect) -> Self {
        Self {
            x0: Vector {
                x: value.x0,
                y: value.y0,
            },
            x1: Vector {
                x: value.x1,
                y: value.y1,
            },
        }
    }
}

impl From<mupdf::IRect> for Rect<i32> {
    fn from(value: mupdf::IRect) -> Self {
        Self {
            x0: Vector {
                x: value.x0,
                y: value.y0,
            },
            x1: Vector {
                x: value.x1,
                y: value.y1,
            },
        }
    }
}

impl From<mupdf::Rect> for Rect<i32> {
    fn from(value: mupdf::Rect) -> Self {
        Self {
            x0: Vector {
                x: value.x0 as i32,
                y: value.y0 as i32,
            },
            x1: Vector {
                x: value.x1 as i32,
                y: value.y1 as i32,
            },
        }
    }
}

impl From<mupdf::IRect> for Rect<f32> {
    fn from(value: mupdf::IRect) -> Self {
        Self {
            x0: Vector {
                x: value.x0 as f32,
                y: value.y0 as f32,
            },
            x1: Vector {
                x: value.x1 as f32,
                y: value.y1 as f32,
            },
        }
    }
}

impl<T> From<Rect<T>> for iced::Rectangle<T>
where
    T: Num + Copy,
{
    fn from(val: Rect<T>) -> Self {
        iced::Rectangle {
            x: val.x0.x,
            y: val.x0.y,
            width: val.x1.x - val.x0.x,
            height: val.x1.y - val.x0.y,
        }
    }
}
