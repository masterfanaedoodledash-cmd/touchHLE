/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGContext.h`

use super::cg_affine_transform::CGAffineTransform;
use super::cg_image::CGImageRef;
use super::{cg_bitmap_context, cg_color, CGFloat, CGPoint, CGRect};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::frameworks::core_graphics::cg_bitmap_context::{
    CGBitmapContextGetHeight, CGBitmapContextGetWidth,
};
use crate::frameworks::core_graphics::cg_color::CGColorRef;
use crate::frameworks::core_graphics::cg_geometry::CGPointZero;
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

type CGInterpolationQuality = i32;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// CGContext seems to be a CFType-based type, but in our implementation those
// are just Objective-C types, so we need a class for it, but its name is not
// visible anywhere.
@implementation _touchHLE_CGContext: NSObject

- (())dealloc {
    let host_obj = env.objc.borrow::<CGContextHostObject>(this);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    if bitmap_data.data_is_owned {
        env.mem.free(bitmap_data.data);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

pub(super) struct CGContextHostObject {
    pub(super) subclass: CGContextSubclass,
    pub(super) rgb_fill_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    pub(super) rgb_stroke_color: (CGFloat, CGFloat, CGFloat, CGFloat), // Добавлено
    pub(super) transform: CGAffineTransform,
    // Стек теперь хранит кортеж из двух цветов и трансформации
    pub(super) state_stack: Vec<((CGFloat, CGFloat, CGFloat, CGFloat), (CGFloat, CGFloat, CGFloat, CGFloat), CGAffineTransform)>,
}
impl HostObject for CGContextHostObject {}

pub(super) enum CGContextSubclass {
    CGBitmapContext(cg_bitmap_context::CGBitmapContextData),
}

pub type CGContextRef = CFTypeRef;

pub fn CGContextRelease(env: &mut Environment, c: CGContextRef) {
    if !c.is_null() {
        CFRelease(env, c);
    }
}
pub fn CGContextRetain(env: &mut Environment, c: CGContextRef) -> CGContextRef {
    if !c.is_null() {
        CFRetain(env, c)
    } else {
        c
    }
}

fn CGContextSetFillColorWithColor(env: &mut Environment, context: CGContextRef, color: CGColorRef) {
    let (r, g, b, a) = cg_color::to_rgba(&env.objc, color);
    CGContextSetRGBFillColor(env, context, r, g, b, a)
}

pub fn CGContextSetRGBFillColor(
    env: &mut Environment,
    context: CGContextRef,
    red: CGFloat,
    green: CGFloat,
    blue: CGFloat,
    alpha: CGFloat,
) {
    let color = (red, green, blue, alpha);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_fill_color = color;
}

pub fn CGContextSetRGBStrokeColor(
    env: &mut Environment,
    context: CGContextRef,
    red: CGFloat,
    green: CGFloat,
    blue: CGFloat,
    alpha: CGFloat,
) {
    if context.is_null() {
        return;
    }
    // Пишем напрямую в поле структуры через borrow_mut
    env.objc.borrow_mut::<CGContextHostObject>(context).rgb_stroke_color = (red, green, blue, alpha);
}

fn CGContextSetGrayFillColor(
    env: &mut Environment,
    context: CGContextRef,
    gray: CGFloat,
    alpha: CGFloat,
) {
    let color = (gray, gray, gray, alpha);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_fill_color = color;
}

pub fn CGContextFillRect(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
) {
    if context.is_null() {
        log!("Warning: CGContextFillRect called with null context, skipping");
        return;
    }
    cg_bitmap_context::fill_rect(env, context, rect, /* clear: */ false);
}

pub fn CGContextClearRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    cg_bitmap_context::fill_rect(env, context, rect, /* clear: */ true);
}

pub fn CGContextClipToRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    if rect.origin == CGPointZero
        && rect.size.height == CGBitmapContextGetHeight(env, context) as f32
        && rect.size.width == CGBitmapContextGetWidth(env, context) as f32
    {
        assert!(env
            .objc
            .borrow_mut::<CGContextHostObject>(context)
            .transform
            .is_identity());
        // All good, clipping is not needed!
        return;
    }
    todo!();
}

pub fn CGContextConcatCTM(
    env: &mut Environment,
    context: CGContextRef,
    transform: CGAffineTransform,
) {
    log_dbg!("CGContextConcatCTM({:?})", transform);
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = transform.concat(host_obj.transform);
}
pub fn CGContextGetCTM(env: &mut Environment, context: CGContextRef) -> CGAffineTransform {
    let res = env.objc.borrow::<CGContextHostObject>(context).transform;
    log_dbg!("CGContextGetCTM() => {:?}", res);
    res
}
pub fn CGContextRotateCTM(env: &mut Environment, context: CGContextRef, angle: CGFloat) {
    log_dbg!("CGContextRotateCTM({:?})", angle);
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.rotate(angle);
}
pub fn CGContextScaleCTM(env: &mut Environment, context: CGContextRef, x: CGFloat, y: CGFloat) {
    log_dbg!("CGContextScaleCTM({:?})", (x, y));
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.scale(x, y);
}
pub fn CGContextTranslateCTM(
    env: &mut Environment,
    context: CGContextRef,
    tx: CGFloat,
    ty: CGFloat,
) {
    log_dbg!("CGContextTranslateCTM({:?})", (tx, ty));
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.translate(tx, ty);
}

pub fn CGContextDrawImage(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
    image: CGImageRef,
) {
    if context.is_null() {
        log!("Warning: CGContextDrawImage called with null context, skipping");
        return; // safely ignore instead of crashing
    }
    cg_bitmap_context::draw_image(env, context, rect, image);
}

fn CGContextSaveGState(env: &mut Environment, context: CGContextRef) {
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.state_stack.push((
        host_obj.rgb_fill_color,
        host_obj.rgb_stroke_color, // Сохраняем цвет обводки
        host_obj.transform,
    ));
}

fn CGContextRestoreGState(env: &mut Environment, context: CGContextRef) {
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    if let Some(state) = host_obj.state_stack.pop() {
        host_obj.rgb_fill_color = state.0;
        host_obj.rgb_stroke_color = state.1; // Восстанавливаем цвет обводки
        host_obj.transform = state.2;
    }
}

fn CGContextSetInterpolationQuality(
    _env: &mut Environment,
    context: CGContextRef,
    quality: CGInterpolationQuality,
) {
    log!(
        "TODO: CGContextSetInterpolationQuality({:?}, {:?})",
        context,
        quality
    );
}

fn CGContextGetTextPosition(
    _env: &mut Environment,
    _context: CGContextRef,
) -> CGPoint {
    CGPoint { x: 0.0, y: 0.0 }
}

fn CGContextSetTextPosition(
    _env: &mut Environment,
    _context: CGContextRef,
    _x: CGFloat,
    _y: CGFloat,
) {
}

fn CGContextSetTextDrawingMode(
    _env: &mut Environment,
    _context: CGContextRef,
    _mode: i32,
) {
}

fn CGContextSetCharacterSpacing(
    _env: &mut Environment,
    _context: CGContextRef,
    _spacing: CGFloat,
) {
}

fn CGContextSetTextMatrix(
    _env: &mut Environment,
    _context: CGContextRef,
    _t: CGAffineTransform,
) {
}

fn CGContextSelectFont(
    _env: &mut Environment,
    _context: CGContextRef,
    _name: crate::mem::ConstPtr<u8>,
    _size: CGFloat,
    _encoding: i32,
) {
}

fn CGContextShowTextAtPoint(
    _env: &mut Environment,
    _context: CGContextRef,
    _x: CGFloat,
    _y: CGFloat,
    _string: crate::mem::ConstPtr<u8>,
    _length: u32,
) {
}

fn CGContextShowText(
    _env: &mut Environment,
    _context: CGContextRef,
    _string: crate::mem::ConstPtr<u8>,
    _length: u32,
) {
}

fn CGContextSetFontSize(
    _env: &mut Environment,
    _context: CGContextRef,
    _size: CGFloat,
) {
}

fn CGContextSetFont(
    _env: &mut Environment,
    _context: CGContextRef,
    _font: crate::mem::ConstVoidPtr,
) {
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGContextRetain(_)),
    export_c_func!(CGContextRelease(_)),
    export_c_func!(CGContextSetFillColorWithColor(_, _)),
    export_c_func!(CGContextSetRGBFillColor(_, _, _, _, _)),
    export_c_func!(CGContextSetRGBStrokeColor(_, _, _, _, _)),
    export_c_func!(CGContextSetGrayFillColor(_, _, _)),
    export_c_func!(CGContextFillRect(_, _)),
    export_c_func!(CGContextClearRect(_, _)),
    export_c_func!(CGContextClipToRect(_, _)),
    export_c_func!(CGContextConcatCTM(_, _)),
    export_c_func!(CGContextGetCTM(_)),
    export_c_func!(CGContextRotateCTM(_, _)),
    export_c_func!(CGContextScaleCTM(_, _, _)),
    export_c_func!(CGContextTranslateCTM(_, _, _)),
    export_c_func!(CGContextDrawImage(_, _, _)),
    export_c_func!(CGContextSaveGState(_)),
    export_c_func!(CGContextRestoreGState(_)),
    export_c_func!(CGContextSetInterpolationQuality(_, _)),
    export_c_func!(CGContextGetTextPosition(_)),
    export_c_func!(CGContextSetTextPosition(_, _, _)),
    export_c_func!(CGContextSetTextDrawingMode(_, _)),
    export_c_func!(CGContextSetCharacterSpacing(_, _)),
    export_c_func!(CGContextSetTextMatrix(_, _)),
    export_c_func!(CGContextSelectFont(_, _, _, _)),
    export_c_func!(CGContextShowTextAtPoint(_, _, _, _, _)),
    export_c_func!(CGContextShowText(_, _, _)),
    export_c_func!(CGContextSetFontSize(_, _)),
    export_c_func!(CGContextSetFont(_, _)),
];
