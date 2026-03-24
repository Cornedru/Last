use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use oxc_allocator::Allocator;
use oxc_allocator::CloneIn;
use oxc_ast::{ast::*, AstBuilder};
use oxc_ast_visit::walk_mut::{
    walk_call_expression, walk_expression, walk_program,
};
use oxc_ast_visit::VisitMut;
use oxc_span::SPAN;
use rustc_hash::FxHashMap;
use std::cell::Cell;

use crate::reverse::xtea::XTEA;
use crate::reverse::encryption::decrypt_cloudflare_response;

#[derive(Clone, Debug)]
struct XteaCallContext {
    callee_name: String,
    key: [u32; 4],
    encrypted_data: Vec<u8>,
    num_rounds: u32,
}

pub struct DynamicXteaDecryptor<'a> {
    ast: AstBuilder<'a>,
    pending_decryptions: Vec<XteaCallContext>,
    resolved_values: FxHashMap<usize, String>,
}

impl<'a> DynamicXteaDecryptor<'a> {
    pub fn new(allocator: &'a Allocator) -> Self {
        Self {
            ast: AstBuilder::new(allocator),
            pending_decryptions: Vec::new(),
            resolved_values: FxHashMap::default(),
        }
    }

    fn detect_xtea_pattern(&self, call: &CallExpression<'_>) -> Option<XteaCallContext> {
        if call.arguments.len() < 2 {
            return None;
        }

        let callee = call.callee.get_identifier_reference()?;
        let callee_name = callee.name.as_str();

        if !callee_name.contains("xtea") 
            && !callee_name.contains("decipher")
            && !callee_name.contains("decrypt")
        {
            return None;
        }

        let mut key = [0u32; 4];
        let mut num_rounds = 32u32;
        let mut encrypted_data = Vec::new();

        for (i, arg) in call.arguments.iter().enumerate() {
            match arg {
                Argument::ArrayExpression(arr) => {
                    for (j, elem) in arr.elements.iter().enumerate() {
                        if j < 4 {
                            if let ArrayExpressionElement::NumericLiteral(num) = elem {
                                key[j] = num.value as u32;
                            }
                        }
                    }
                }
                Argument::NumericLiteral(num) => {
                    if i == 1 {
                        num_rounds = num.value as u32;
                    }
                }
                Argument::StringLiteral(s) => {
                    if let Ok(decoded) = BASE64_STANDARD.decode(s.value.as_bytes()) {
                        encrypted_data = decoded;
                    }
                }
                Argument::CallExpression(inner_call) => {
                    if let Some(data) = self.extract_static_string_from_call(inner_call) {
                        encrypted_data = data;
                    }
                }
                _ => {}
            }
        }

        if key.iter().all(|&k| k != 0) && !encrypted_data.is_empty() {
            Some(XteaCallContext {
                callee_name: callee_name.to_string(),
                key,
                encrypted_data,
                num_rounds,
            })
        } else {
            None
        }
    }

    fn extract_static_string_from_call(&self, call: &CallExpression<'_>) -> Option<Vec<u8>> {
        if call.arguments.is_empty() {
            return None;
        }

        let first_arg = &call.arguments[0];
        if let Argument::StringLiteral(s) = first_arg {
            BASE64_STANDARD.decode(s.value.as_bytes()).ok()
        } else {
            None
        }
    }

    fn decrypt_xtea(&self, ctx: &XteaCallContext) -> Option<String> {
        let xtea = XTEA::new_with_rounds(&ctx.key, ctx.num_rounds);
        
        if ctx.encrypted_data.len() % 8 != 0 {
            return None;
        }

        let mut decrypted = vec![0u8; ctx.encrypted_data.len()];
        xtea.encipher_u8slice::<byteorder::LittleEndian>(
            &ctx.encrypted_data,
            &mut decrypted,
        );

        String::from_utf8(decrypted).ok()
    }

    fn try_decrypt_cloudflare_response(&self, data: &str) -> Option<String> {
        decrypt_cloudflare_response("test", data).ok()
    }
}

impl<'a> VisitMut<'a> for DynamicXteaDecryptor<'a> {
    fn visit_program(&mut self, node: &mut Program<'a>) {
        walk_program(self, node);
    }

    fn visit_call_expression(&mut self, call_expr: &mut CallExpression<'a>) {
        if let Some(xtea_ctx) = self.detect_xtea_pattern(call_expr) {
            if let Some(decrypted) = self.decrypt_xtea(&xtea_ctx) {
                let callee_expr = Expression::Identifier(self.ast.alloc(IdentifierReference {
                    span: SPAN,
                    name: self.ast.atom("atob"),
                    reference_id: Cell::new(None),
                }));
                
                let str_lit = self.ast.alloc(StringLiteral {
                    span: SPAN,
                    value: self.ast.atom(&decrypted),
                    raw: None,
                    lone_surrogates: false,
                });
                
                let new_call = self.ast.alloc(CallExpression {
                    span: call_expr.span,
                    callee: callee_expr,
                    arguments: self.ast.vec_from_iter([Argument::StringLiteral(str_lit)]),
                    type_arguments: None,
                    pure: false,
                    optional: false,
                });
                
                                            *call_expr = (*new_call).clone_in(self.ast.allocator);
                return;
            }
        }

        if call_expr.arguments.len() == 2 {
            if let Expression::Identifier(ident) = &call_expr.callee {
                if ident.name == "atob" || ident.name == "btoa" {
                    if let Argument::StringLiteral(s) = &call_expr.arguments[0] {
                        if s.value.len() > 50 {
                            if let Ok(decoded_vec) = BASE64_STANDARD.decode(s.value.as_bytes()) {
                                if let Ok(decrypted_str) = String::from_utf8(decoded_vec) {
                                    if decrypted_str.chars().all(|c| c.is_ascii_graphic() || c.is_whitespace()) {
                                        if let Some(cloudflare_decrypted) = 
                                            self.try_decrypt_cloudflare_response(s.value.as_str()) 
                                        {
                                            let callee_expr = Expression::Identifier(self.ast.alloc(IdentifierReference {
                                                span: SPAN,
                                                name: self.ast.atom(&ident.name),
                                                reference_id: Cell::new(None),
                                            }));
                                            
                                            let str_lit = self.ast.alloc(StringLiteral {
                                                span: SPAN,
                                                value: self.ast.atom(&cloudflare_decrypted),
                                                raw: None,
                                                lone_surrogates: false,
                                            });
                                            
                                            let new_call = self.ast.alloc(CallExpression {
                                                span: call_expr.span,
                                                callee: callee_expr,
                                                arguments: self.ast.vec_from_iter([Argument::StringLiteral(str_lit)]),
                                                type_arguments: None,
                                                pure: false,
                                                optional: false,
                                            });
                                            
                *call_expr = (*new_call).clone_in(self.ast.allocator);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        walk_call_expression(self, call_expr);
    }

    fn visit_expression(&mut self, node: &mut Expression<'a>) {
        if let Expression::CallExpression(call_expr) = node {
            self.visit_call_expression(call_expr);
            return;
        }

        walk_expression(self, node);
    }
}

pub struct XteaDecryptionPass;

impl XteaDecryptionPass {
    pub fn run<'a>(allocator: &'a Allocator, program: &mut Program<'a>) {
        let mut decryptor = DynamicXteaDecryptor::new(allocator);
        decryptor.visit_program(program);
    }
}
