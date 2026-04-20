/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CATransform3D` implementation for Core Animation.

use crate::abi::{impl_GuestRet_for_large_struct, GuestArg};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::matrix::Matrix;
use crate::mem::SafeRead;
use crate::Environment;

// На 32-битных системах (ARMv6/v7) CGFloat — это всегда 32-битный float
type CGFloat = f32;

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C, packed)]
/// CATransform3D представляет собой матрицу 4x4 для 3D-преобразований.
pub struct CATransform3D {
    pub m11: CGFloat, pub m12: CGFloat, pub m13: CGFloat, pub m14: CGFloat,
    pub m21: CGFloat, pub m22: CGFloat, pub m23: CGFloat, pub m24: CGFloat,
    pub m31: CGFloat, pub m32: CGFloat, pub m33: CGFloat, pub m34: CGFloat,
    pub m41: CGFloat, pub m42: CGFloat, pub m43: CGFloat, pub m44: CGFloat,
}

unsafe impl SafeRead for CATransform3D {}

impl GuestArg for CATransform3D {
    const REG_COUNT: usize = 16;

    fn from_regs(regs: &[u32]) -> Self {
        CATransform3D {
            m11: GuestArg::from_regs(&regs[0..1]),
            m12: GuestArg::from_regs(&regs[1..2]),
            m13: GuestArg::from_regs(&regs[2..3]),
            m14: GuestArg::from_regs(&regs[3..4]),
            m21: GuestArg::from_regs(&regs[4..5]),
            m22: GuestArg::from_regs(&regs[5..6]),
            m23: GuestArg::from_regs(&regs[6..7]),
            m24: GuestArg::from_regs(&regs[7..8]),
            m31: GuestArg::from_regs(&regs[8..9]),
            m32: GuestArg::from_regs(&regs[9..10]),
            m33: GuestArg::from_regs(&regs[10..11]),
            m34: GuestArg::from_regs(&regs[11..12]),
            m41: GuestArg::from_regs(&regs[12..13]),
            m42: GuestArg::from_regs(&regs[13..14]),
            m43: GuestArg::from_regs(&regs[14..15]),
            m44: GuestArg::from_regs(&regs[15..16]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.m11.to_regs(&mut regs[0..1]);
        self.m12.to_regs(&mut regs[1..2]);
        self.m13.to_regs(&mut regs[2..3]);
        self.m14.to_regs(&mut regs[3..4]);
        self.m21.to_regs(&mut regs[4..5]);
        self.m22.to_regs(&mut regs[5..6]);
        self.m23.to_regs(&mut regs[6..7]);
        self.m24.to_regs(&mut regs[7..8]);
        self.m31.to_regs(&mut regs[8..9]);
        self.m32.to_regs(&mut regs[9..10]);
        self.m33.to_regs(&mut regs[10..11]);
        self.m34.to_regs(&mut regs[11..12]);
        self.m41.to_regs(&mut regs[12..13]);
        self.m42.to_regs(&mut regs[13..14]);
        self.m43.to_regs(&mut regs[14..15]);
        self.m44.to_regs(&mut regs[15..16]);
    }
}

// Этот макрос автоматически реализует возврат структуры через скрытый указатель (sret)
impl_GuestRet_for_large_struct!(CATransform3D);

impl From<CATransform3D> for Matrix<4> {
    fn from(value: CATransform3D) -> Matrix<4> {
        Matrix::<4>::from_columns([
            [value.m11, value.m12, value.m13, value.m14],
            [value.m21, value.m22, value.m23, value.m24],
            [value.m31, value.m32, value.m33, value.m34],
            [value.m41, value.m42, value.m43, value.m44],
        ])
    }
}

impl From<Matrix<4>> for CATransform3D {
    fn from(matrix: Matrix<4>) -> Self {
        let columns = matrix.columns();
        CATransform3D {
            m11: columns[0][0], m12: columns[0][1], m13: columns[0][2], m14: columns[0][3],
            m21: columns[1][0], m22: columns[1][1], m23: columns[1][2], m24: columns[1][3],
            m31: columns[2][0], m32: columns[2][1], m33: columns[2][2], m34: columns[2][3],
            m41: columns[3][0], m42: columns[3][1], m43: columns[3][2], m44: columns[3][3],
        }
    }
}

#[rustfmt::skip]
pub const CATransform3DIdentity: CATransform3D = CATransform3D {
    m11: 1.0, m12: 0.0, m13: 0.0, m14: 0.0,
    m21: 0.0, m22: 1.0, m23: 0.0, m24: 0.0,
    m31: 0.0, m32: 0.0, m33: 1.0, m34: 0.0,
    m41: 0.0, m42: 0.0, m43: 0.0, m44: 1.0,
};

pub const CONSTANTS: ConstantExports = &[(
    "_CATransform3DIdentity",
    HostConstant::Custom(|env| {
        env.mem
            .alloc_and_write(CATransform3DIdentity)
            .cast()
            .cast_const()
    }),
)];

impl CATransform3D {
    pub fn make_translation(tx: CGFloat, ty: CGFloat, tz: CGFloat) -> Self {
        let mut t = CATransform3DIdentity;
        t.m41 = tx;
        t.m42 = ty;
        t.m43 = tz;
        t
    }

    pub fn make_scale(sx: CGFloat, sy: CGFloat, sz: CGFloat) -> Self {
        let mut t = CATransform3DIdentity;
        t.m11 = sx;
        t.m22 = sy;
        t.m33 = sz;
        t
    }

    pub fn make_rotation(angle: CGFloat, x: CGFloat, y: CGFloat, z: CGFloat) -> Self {
        let length = (x * x + y * y + z * z).sqrt();
        if length == 0.0 {
            return CATransform3DIdentity;
        }

        let nx = x / length;
        let ny = y / length;
        let nz = z / length;

        let c = angle.cos();
        let s = angle.sin();
        let t = 1.0 - c;

        CATransform3D {
            m11: t * nx * nx + c,
            m12: t * nx * ny + nz * s,
            m13: t * nx * nz - ny * s,
            m14: 0.0,

            m21: t * nx * ny - nz * s,
            m22: t * ny * ny + c,
            m23: t * ny * nz + nx * s,
            m24: 0.0,

            m31: t * nx * nz + ny * s,
            m32: t * ny * nz - nx * s,
            m33: t * nz * nz + c,
            m34: 0.0,

            m41: 0.0,
            m42: 0.0,
            m43: 0.0,
            m44: 1.0,
        }
    }

    pub fn concat(self, other: Self) -> Self {
        let a: Matrix<4> = self.into();
        let b: Matrix<4> = other.into();
        Matrix::<4>::multiply(&a, &b).into()
    }

    pub fn rotate(self, angle: CGFloat, x: CGFloat, y: CGFloat, z: CGFloat) -> Self {
        let rot_mat = Self::make_rotation(angle, x, y, z);
        rot_mat.concat(self)
    }
    
    pub fn scale(self, sx: CGFloat, sy: CGFloat, sz: CGFloat) -> Self {
        let scale_mat = Self::make_scale(sx, sy, sz);
        scale_mat.concat(self)
    }
}

fn CATransform3DMakeTranslation(_env: &mut Environment, tx: CGFloat, ty: CGFloat, tz: CGFloat) -> CATransform3D {
    CATransform3D::make_translation(tx, ty, tz)
}

fn CATransform3DMakeScale(_env: &mut Environment, sx: CGFloat, sy: CGFloat, sz: CGFloat) -> CATransform3D {
    CATransform3D::make_scale(sx, sy, sz)
}

fn CATransform3DMakeRotation(_env: &mut Environment, angle: CGFloat, x: CGFloat, y: CGFloat, z: CGFloat) -> CATransform3D {
    CATransform3D::make_rotation(angle, x, y, z)
}

fn CATransform3DScale(_env: &mut Environment, t: CATransform3D, sx: CGFloat, sy: CGFloat, sz: CGFloat) -> CATransform3D {
    t.scale(sx, sy, sz)
}

fn CATransform3DRotate(_env: &mut Environment, t: CATransform3D, angle: CGFloat, x: CGFloat, y: CGFloat, z: CGFloat) -> CATransform3D {
    t.rotate(angle, x, y, z)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CATransform3DMakeTranslation(_, _, _)),
    export_c_func!(CATransform3DMakeScale(_, _, _)),
    export_c_func!(CATransform3DMakeRotation(_, _, _, _)),
    export_c_func!(CATransform3DScale(_, _, _, _)),
    export_c_func!(CATransform3DRotate(_, _, _, _, _)), // Добавили эту строку (5 аргументов)
];
