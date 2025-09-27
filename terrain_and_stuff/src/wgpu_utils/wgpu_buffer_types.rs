//! Explicitly padded and/or aligned types following wgsl rules.
//! See [wgsl spec on alignment and size](https://www.w3.org/TR/WGSL/#alignment-and-size)
//!
//! This is especially important for cases where [`glam`] isn't explicit about padding and alignment.

use bytemuck::{CheckedBitPattern, Pod, Zeroable};

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct F32RowPadded {
    pub v: f32,
    pub padding0: f32,
    pub padding1: f32,
    pub padding2: f32,
}

impl From<f32> for F32RowPadded {
    #[inline]
    fn from(v: f32) -> Self {
        Self {
            v,
            padding0: 0.0,
            padding1: 0.0,
            padding2: 0.0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct U32RowPadded {
    pub v: u32,
    pub padding0: u32,
    pub padding1: u32,
    pub padding2: u32,
}

impl From<u32> for U32RowPadded {
    #[inline]
    fn from(v: u32) -> Self {
        Self {
            v,
            padding0: 0,
            padding1: 0,
            padding2: 0,
        }
    }
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl From<glam::Vec2> for Vec2 {
    #[inline]
    fn from(v: glam::Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<[f32; 2]> for Vec2 {
    #[inline]
    fn from([x, y]: [f32; 2]) -> Self {
        Self { x, y }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec2RowPadded {
    pub x: f32,
    pub y: f32,
    pub padding0: f32,
    pub padding1: f32,
}

impl From<glam::Vec2> for Vec2RowPadded {
    #[inline]
    fn from(v: glam::Vec2) -> Self {
        Self {
            x: v.x,
            y: v.y,
            padding0: 0.0,
            padding1: 0.0,
        }
    }
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct UVec2 {
    pub x: u32,
    pub y: u32,
}

impl From<glam::UVec2> for UVec2 {
    #[inline]
    fn from(v: glam::UVec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<[u8; 2]> for UVec2 {
    #[inline]
    fn from([x, y]: [u8; 2]) -> Self {
        Self {
            x: x as u32,
            y: y as u32,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct UVec2RowPadded {
    pub x: u32,
    pub y: u32,
    pub padding0: u32,
    pub padding1: u32,
}

impl From<glam::UVec2> for UVec2RowPadded {
    #[inline]
    fn from(v: glam::UVec2) -> Self {
        Self {
            x: v.x,
            y: v.y,
            padding0: 0,
            padding1: 0,
        }
    }
}

impl From<[u8; 2]> for UVec2RowPadded {
    #[inline]
    fn from([x, y]: [u8; 2]) -> Self {
        Self {
            x: x as u32,
            y: y as u32,
            padding0: 0,
            padding1: 0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec3RowPadded {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub padding: f32,
}

impl serde::Serialize for Vec3RowPadded {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        glam::Vec3::from(*self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Vec3RowPadded {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        glam::Vec3::deserialize(deserializer).map(Self::from)
    }
}

impl PartialEq for Vec3RowPadded {
    fn eq(&self, other: &Self) -> bool {
        glam::Vec3::from(*self) == glam::Vec3::from(*other)
    }
}

impl Eq for Vec3RowPadded {}

impl From<Vec3RowPadded> for glam::Vec3 {
    #[inline]
    fn from(v: Vec3RowPadded) -> Self {
        glam::vec3(v.x, v.y, v.z)
    }
}

impl From<glam::Vec3> for Vec3RowPadded {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            padding: 0.0,
        }
    }
}

impl From<glam::Vec3A> for Vec3RowPadded {
    #[inline]
    fn from(v: glam::Vec3A) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            padding: 0.0,
        }
    }
}

#[repr(C, align(4))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec3Unpadded {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<glam::Vec3> for Vec3Unpadded {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<glam::Vec3A> for Vec3Unpadded {
    #[inline]
    fn from(v: glam::Vec3A) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl From<glam::Vec4> for Vec4 {
    #[inline]
    fn from(v: glam::Vec4) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            w: v.w,
        }
    }
}

impl From<Vec4> for glam::Vec4 {
    #[inline]
    fn from(val: Vec4) -> Self {
        glam::vec4(val.x, val.y, val.z, val.w)
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Mat3 {
    c0: Vec3RowPadded,
    c1: Vec3RowPadded,
    c2: Vec3RowPadded,
}

impl From<glam::Mat3> for Mat3 {
    #[inline]
    fn from(m: glam::Mat3) -> Self {
        Self {
            c0: m.x_axis.into(),
            c1: m.y_axis.into(),
            c2: m.z_axis.into(),
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Mat4 {
    c0: Vec4,
    c1: Vec4,
    c2: Vec4,
    c3: Vec4,
}

impl From<glam::Mat4> for Mat4 {
    #[inline]
    fn from(m: glam::Mat4) -> Self {
        Self {
            c0: m.x_axis.into(),
            c1: m.y_axis.into(),
            c2: m.z_axis.into(),
            c3: m.w_axis.into(),
        }
    }
}

impl From<glam::Affine3A> for Mat4 {
    #[inline]
    fn from(m: glam::Affine3A) -> Self {
        glam::Mat4::from(m).into()
    }
}

impl From<Mat4> for glam::Mat4 {
    #[inline]
    fn from(val: Mat4) -> Self {
        Self::from_cols(val.c0.into(), val.c1.into(), val.c2.into(), val.c3.into())
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Mat4x3 {
    c0: Vec3RowPadded,
    c1: Vec3RowPadded,
    c2: Vec3RowPadded,
    c3: Vec3RowPadded,
}

impl From<glam::Affine3A> for Mat4x3 {
    #[inline]
    fn from(m: glam::Affine3A) -> Self {
        Self {
            c0: m.matrix3.x_axis.into(),
            c1: m.matrix3.y_axis.into(),
            c2: m.matrix3.z_axis.into(),
            c3: m.translation.into(),
        }
    }
}

/// A Vec4 of pure padding (i.e. 16 bytes of padding)
///
/// Useful utility to pad uniform buffers out to a multiple of 16 rows,
/// (256 bytes is the alignment requirement for Uniform buffers)
#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod, Default)]
pub struct PaddingRow {
    p: [f32; 4],
}

/// A wrapper around an enum that is always stored as a u32 for consumption in wgsl.
#[derive(Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(transparent)]
pub struct WgslEnum<T>
where
    T: Copy + Into<u32> + CheckedBitPattern + Zeroable + 'static,
{
    value: u32,
    _marker: std::marker::PhantomData<T>,
}

// Bytemuck doesn't allow deriving Pod if any of the generics parameters are not Pod.
// https://github.com/Lokathor/bytemuck/issues/191
unsafe impl<T: Copy + Into<u32> + CheckedBitPattern + Zeroable + 'static> bytemuck::Pod
    for WgslEnum<T>
{
}

impl<T: Copy + Into<u32> + CheckedBitPattern + Zeroable> WgslEnum<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: value.into(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn set(&mut self, value: T) {
        self.value = value.into();
    }

    pub fn get(&self) -> T {
        bytemuck::checked::try_cast(self.value).unwrap()
    }
}

impl<T: Copy + Into<u32> + CheckedBitPattern + Zeroable + 'static + serde::Serialize>
    serde::Serialize for WgslEnum<T>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.get().serialize(serializer)
    }
}

impl<'de, T: Copy + Into<u32> + CheckedBitPattern + Zeroable + 'static + serde::Deserialize<'de>>
    serde::Deserialize<'de> for WgslEnum<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(|value| Self::new(value))
    }
}
