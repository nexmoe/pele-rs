//! The fdlibm evaluation order used by V8 for the bounded angles of Venn layouts.
//!
//! Adapted from V8 13.6.233, src/base/ieee754.cc.
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this software is freely granted,
//! provided that this notice is preserved.
//! Modified significantly by Google Inc. Copyright 2016 the V8 project authors.
//! All rights reserved. See LICENSE-BSD3 and NOTICE.
#![allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    reason = "fdlibm constants preserve reference rounding"
)]
fn high(x: f64) -> u32 {
    ((x.to_bits() >> 32) as u32) & 0x7fffffff
}
fn kernel_sin(x: f64, y: f64, tail: bool) -> f64 {
    const S1: f64 = -1.66666666666666324348e-1;
    const S2: f64 = 8.33333333332248946124e-3;
    const S3: f64 = -1.98412698298579493134e-4;
    const S4: f64 = 2.75573137070700676789e-6;
    const S5: f64 = -2.50507602534068634195e-8;
    const S6: f64 = 1.58969099521155010221e-10;
    if high(x) < 0x3e400000 {
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    if tail {
        x - (-v).mul_add(S1, z * (0.5 * y - v * r) - y)
    } else {
        x + v * (S1 + z * r)
    }
}
fn kernel_cos(x: f64, y: f64) -> f64 {
    const C1: f64 = 4.16666666666666019037e-2;
    const C2: f64 = -1.38888888888741095749e-3;
    const C3: f64 = 2.48015872894767294178e-5;
    const C4: f64 = -2.75573143513906633035e-7;
    const C5: f64 = 2.08757232129817482790e-9;
    const C6: f64 = -1.13596475577881948265e-11;
    let ix = high(x);
    if ix < 0x3e400000 {
        return 1.0;
    }
    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    if ix < 0x3fd33333 {
        1.0 - (0.5 * z - (z * r - x * y))
    } else {
        let qx = if ix > 0x3fe90000 {
            0.28125
        } else {
            f64::from_bits(u64::from(ix - 0x00200000) << 32)
        };
        let iz = 0.5 * z - qx;
        let a = 1.0 - qx;
        a - (iz - (z * r - x * y))
    }
}
fn reduce(x: f64) -> (i32, f64, f64) {
    const INV: f64 = 6.36619772367581382433e-1;
    const P1: f64 = 1.57079632673412561417;
    const T1: f64 = 6.07710050650619224932e-11;
    const P2: f64 = 6.07710050630396597660e-11;
    const T2: f64 = 2.02226624879595063154e-21;
    const P3: f64 = 2.02226624871116645580e-21;
    const T3: f64 = 8.47842766036889956997e-32;
    let ix = high(x);
    if ix < 0x4002d97c {
        let sign = if x > 0.0 { 1.0 } else { -1.0 };
        let mut z = x - sign * P1;
        let tail = if ix != 0x3ff921fb {
            sign * T1
        } else {
            z -= sign * P2;
            sign * T2
        };
        let y = z - tail;
        return (sign as i32, y, (z - y) - tail);
    }
    let n = (x.abs() * INV + 0.5) as i32;
    let f = f64::from(n);
    let mut r = x.abs() - f * P1;
    let mut w = f * T1;
    let mut y = r - w;
    let exponent = (ix >> 20) as i32;
    if exponent - ((high(y) >> 20) & 0x7ff) as i32 > 16 {
        let t = r;
        w = f * P2;
        r = t - w;
        w = f * T2 - ((t - r) - w);
        y = r - w;
        if exponent - ((high(y) >> 20) & 0x7ff) as i32 > 49 {
            let t = r;
            w = f * P3;
            r = t - w;
            w = f * T3 - ((t - r) - w);
            y = r - w;
        }
    }
    let tail = (r - y) - w;
    if x < 0.0 {
        (-n, -y, -tail)
    } else {
        (n, y, tail)
    }
}
pub(super) fn sin(x: f64) -> f64 {
    if high(x) <= 0x3fe921fb {
        return kernel_sin(x, 0.0, false);
    }
    // All angles produced by the layout lie in this reduction's accurate range.
    if x.abs() > 800_000.0 {
        return x.sin();
    }
    let (n, y, tail) = reduce(x);
    match n & 3 {
        0 => kernel_sin(y, tail, true),
        1 => kernel_cos(y, tail),
        2 => -kernel_sin(y, tail, true),
        _ => -kernel_cos(y, tail),
    }
}
pub(super) fn cos(x: f64) -> f64 {
    if high(x) <= 0x3fe921fb {
        return kernel_cos(x, 0.0);
    }
    if x.abs() > 800_000.0 {
        return x.cos();
    }
    let (n, y, tail) = reduce(x);
    match n & 3 {
        0 => kernel_cos(y, tail),
        1 => -kernel_sin(y, tail, true),
        2 => -kernel_cos(y, tail),
        _ => kernel_sin(y, tail, true),
    }
}
