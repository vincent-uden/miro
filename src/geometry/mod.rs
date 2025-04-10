use std::ops::{Add, AddAssign, Sub, SubAssign};

use num::Num;
use serde::{Deserialize, Serialize};
use tracing::debug;

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

impl Into<Vector<i32>> for Vector<f32> {
    fn into(self) -> Vector<i32> {
        Vector {
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}

impl Into<iced::Point> for Vector<f32> {
    fn into(self) -> iced::Point {
        iced::Point::new(self.x, self.y)
    }
}

impl Into<iced::Vector> for Vector<f32> {
    fn into(self) -> iced::Vector {
        iced::Vector::new(self.x, self.y)
    }
}

impl Into<iced::Size> for Vector<f32> {
    fn into(self) -> iced::Size {
        iced::Size::new(self.x, self.y)
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect<T> {
    pub x0: Vector<T>,
    pub x1: Vector<T>,
}

impl<T> Rect<T>
where
    T: Num + Copy + std::fmt::Debug,
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
        (self.x0 + self.x1).scaled(T::one() + T::one())
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

    pub fn scale(&mut self, s: T) {
        let x0 = self.x0;
        let x1 = self.x1;
        // TODO: Something is wrong here. Double check the math until tests pass
        println!("{:?} {:?}", x0.scaled(T::one() + s), T::one());
        self.x0 = (x0.scaled(T::one() + s) + x1.scaled(T::one() - s))
            .scaled(T::one() / (T::one() + T::one()));
        self.x1 = (x0.scaled(T::one() - s) + x1.scaled(T::one() + s))
            .scaled(T::one() / (T::one() + T::one()));
    }
}

impl Into<iced::Rectangle> for Rect<f32> {
    fn into(self) -> iced::Rectangle {
        iced::Rectangle::new(self.x0.into(), (self.x1 - self.x0).into())
    }
}

impl Into<mupdf::Rect> for Rect<f32> {
    fn into(self) -> mupdf::Rect {
        mupdf::Rect::new(self.x0.x, self.x0.y, self.x1.x, self.x1.y)
    }
}

impl Into<mupdf::IRect> for Rect<i32> {
    fn into(self) -> mupdf::IRect {
        mupdf::IRect::new(self.x0.x, self.x0.y, self.x1.x, self.x1.y)
    }
}

impl Into<mupdf::IRect> for Rect<f32> {
    fn into(self) -> mupdf::IRect {
        let irect: Rect<i32> = self.into();
        irect.into()
    }
}

impl Into<Rect<i32>> for Rect<f32> {
    fn into(self) -> Rect<i32> {
        Rect {
            x0: self.x0.into(),
            x1: self.x1.into(),
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
