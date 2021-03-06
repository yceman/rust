// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module implements some validity checks for attributes.
//! In particular it verifies that `#[inline]` and `#[repr]` attributes are
//! attached to items that actually support them and if there are
//! conflicts between multiple such attributes attached to the same
//! item.

use session::Session;

use syntax::ast;
use syntax::visit;
use syntax::visit::Visitor;

#[derive(Copy, Clone, PartialEq)]
enum Target {
    Fn,
    Struct,
    Union,
    Enum,
    Other,
}

impl Target {
    fn from_item(item: &ast::Item) -> Target {
        match item.node {
            ast::ItemKind::Fn(..) => Target::Fn,
            ast::ItemKind::Struct(..) => Target::Struct,
            ast::ItemKind::Union(..) => Target::Union,
            ast::ItemKind::Enum(..) => Target::Enum,
            _ => Target::Other,
        }
    }
}

struct CheckAttrVisitor<'a> {
    sess: &'a Session,
}

impl<'a> CheckAttrVisitor<'a> {
    /// Check any attribute.
    fn check_attribute(&self, attr: &ast::Attribute, item: &ast::Item, target: Target) {
        if let Some(name) = attr.name() {
            match &*name.as_str() {
                "inline" => self.check_inline(attr, item, target),
                "repr" => self.check_repr(attr, item, target),
                _ => (),
            }
        }
    }

    /// Check if an `#[inline]` is applied to a function.
    fn check_inline(&self, attr: &ast::Attribute, item: &ast::Item, target: Target) {
        if target != Target::Fn {
            struct_span_err!(self.sess, attr.span, E0518, "attribute should be applied to function")
                .span_label(item.span, "not a function")
                .emit();
        }
    }

    /// Check if an `#[repr]` attr is valid.
    fn check_repr(&self, attr: &ast::Attribute, item: &ast::Item, target: Target) {
        let words = match attr.meta_item_list() {
            Some(words) => words,
            None => {
                return;
            }
        };

        let mut int_reprs = 0;
        let mut is_c = false;
        let mut is_simd = false;

        for word in words {

            let name = match word.name() {
                Some(word) => word,
                None => continue,
            };

            let (message, label) = match &*name.as_str() {
                "C" => {
                    is_c = true;
                    if target != Target::Struct &&
                            target != Target::Union &&
                            target != Target::Enum {
                                ("attribute should be applied to struct, enum or union",
                                 "a struct, enum or union")
                    } else {
                        continue
                    }
                }
                "packed" => {
                    // Do not increment conflicting_reprs here, because "packed"
                    // can be used to modify another repr hint
                    if target != Target::Struct &&
                            target != Target::Union {
                                ("attribute should be applied to struct or union",
                                 "a struct or union")
                    } else {
                        continue
                    }
                }
                "simd" => {
                    is_simd = true;
                    if target != Target::Struct {
                        ("attribute should be applied to struct",
                         "a struct")
                    } else {
                        continue
                    }
                }
                "align" => {
                    if target != Target::Struct &&
                            target != Target::Union {
                        ("attribute should be applied to struct or union",
                         "a struct or union")
                    } else {
                        continue
                    }
                }
                "i8" | "u8" | "i16" | "u16" |
                "i32" | "u32" | "i64" | "u64" |
                "isize" | "usize" => {
                    int_reprs += 1;
                    if target != Target::Enum {
                        ("attribute should be applied to enum",
                         "an enum")
                    } else {
                        continue
                    }
                }
                _ => continue,
            };
            struct_span_err!(self.sess, attr.span, E0517, "{}", message)
                .span_label(item.span, format!("not {}", label))
                .emit();
        }

        // Warn on repr(u8, u16), repr(C, simd), and c-like-enum-repr(C, u8)
        if (int_reprs > 1)
           || (is_simd && is_c)
           || (int_reprs == 1 && is_c && is_c_like_enum(item)) {
            span_warn!(self.sess, attr.span, E0566,
                       "conflicting representation hints");
        }
    }
}

impl<'a> Visitor<'a> for CheckAttrVisitor<'a> {
    fn visit_item(&mut self, item: &'a ast::Item) {
        let target = Target::from_item(item);
        for attr in &item.attrs {
            self.check_attribute(attr, item, target);
        }
        visit::walk_item(self, item);
    }
}

pub fn check_crate(sess: &Session, krate: &ast::Crate) {
    visit::walk_crate(&mut CheckAttrVisitor { sess: sess }, krate);
}

fn is_c_like_enum(item: &ast::Item) -> bool {
    if let ast::ItemKind::Enum(ref def, _) = item.node {
        for variant in &def.variants {
            match variant.node.data {
                ast::VariantData::Unit(_) => { /* continue */ }
                _ => { return false; }
            }
        }
        true
    } else {
        false
    }
}
