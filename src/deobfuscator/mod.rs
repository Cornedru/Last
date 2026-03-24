mod parallel_arenas;
mod transformers;

use transformers::{
    control_flow_flattening::ControlFlowFlattening, normalize_conditionals::NormalizeConditionals,
    proxy_functions::ReplaceProxyFunctions, sequence_expressions::SequenceExpressions,
    strings::Strings, useless_if::UselessIf,
    dynamic_xtea_decryptor::XteaDecryptionPass,
};

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_ast_visit::VisitMut;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::SourceType;
use crate::deobfuscator::transformers::numbers::NumbersVisitor;

pub struct MaStringDecoder;

impl MaStringDecoder {
    pub fn decode(js_code: &str, main_script: bool) -> String {
        let allocator = Allocator::default();
        
        let source_type: SourceType = SourceType::default().with_module(false);
        let parsed = Parser::new(&allocator, js_code, source_type).parse();
        
        let mut program = parsed.program;

        let mut numbers = NumbersVisitor::new(&allocator);
        numbers.visit_program(&mut program);

        let mut strings = Strings::new(&allocator, main_script);
        strings.visit_program(&mut program);

        let mut seq = SequenceExpressions::new(&allocator);
        seq.visit_program(&mut program);

        let mut proxy = ReplaceProxyFunctions::new(&allocator);
        proxy.visit_program(&mut program);

        XteaDecryptionPass::run(&allocator, &mut program);

        let mut cff = ControlFlowFlattening::new(&allocator);
        cff.visit_program(&mut program);

        let mut normalize_conditionals = NormalizeConditionals::new(&allocator);
        normalize_conditionals.visit_program(&mut program);

        let mut useless_if = UselessIf::new(&allocator);
        useless_if.visit_program(&mut program);

        Codegen::new()
            .build(&program)
            .code
    }
}

pub fn deobfuscate_with_allocator<'a>(
    js_code: &'a str,
    allocator: &'a Allocator,
    main_script: bool,
) -> Program<'a> {
    let source_type: SourceType = SourceType::default().with_module(false);
    let parsed = Parser::new(allocator, js_code, source_type).parse();

    let mut program = parsed.program;

    let mut numbers = NumbersVisitor::new(allocator);
    numbers.visit_program(&mut program);

    let mut strings = Strings::new(allocator, main_script);
    strings.visit_program(&mut program);

    let mut seq = SequenceExpressions::new(allocator);
    seq.visit_program(&mut program);

    let mut proxy = ReplaceProxyFunctions::new(allocator);
    proxy.visit_program(&mut program);

    XteaDecryptionPass::run(allocator, &mut program);

    let mut cff = ControlFlowFlattening::new(allocator);
    cff.visit_program(&mut program);

    let mut normalize_conditionals = NormalizeConditionals::new(allocator);
    normalize_conditionals.visit_program(&mut program);

    let mut useless_if = UselessIf::new(allocator);
    useless_if.visit_program(&mut program);

    program
}
