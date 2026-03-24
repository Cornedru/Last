use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VmInstructionType {
    MemoryWrite,
    MemoryRead,
    XorOp,
    AddOp,
    SubOp,
    Jump,
    ConditionalJump,
    Return,
    PushConstant,
    Pop,
    Swap,
    Duplicate,
    Load,
    Store,
    Compare,
    Call,
    Unknown,
}

impl std::fmt::Display for VmInstructionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmInstructionType::MemoryWrite => write!(f, "MEMORY_WRITE"),
            VmInstructionType::MemoryRead => write!(f, "MEMORY_READ"),
            VmInstructionType::XorOp => write!(f, "XOR_OP"),
            VmInstructionType::AddOp => write!(f, "ADD_OP"),
            VmInstructionType::SubOp => write!(f, "SUB_OP"),
            VmInstructionType::Jump => write!(f, "JUMP"),
            VmInstructionType::ConditionalJump => write!(f, "CONDITIONAL_JUMP"),
            VmInstructionType::Return => write!(f, "RETURN"),
            VmInstructionType::PushConstant => write!(f, "PUSH_CONSTANT"),
            VmInstructionType::Pop => write!(f, "POP"),
            VmInstructionType::Swap => write!(f, "SWAP"),
            VmInstructionType::Duplicate => write!(f, "DUPLICATE"),
            VmInstructionType::Load => write!(f, "LOAD"),
            VmInstructionType::Store => write!(f, "STORE"),
            VmInstructionType::Compare => write!(f, "COMPARE"),
            VmInstructionType::Call => write!(f, "CALL"),
            VmInstructionType::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmOpcodeMapping {
    pub opcode_to_type: HashMap<i64, VmInstructionType>,
    pub opcode_to_name: HashMap<i64, String>,
    pub state_property_names: StatePropertyNames,
    pub heuristics_applied: Vec<String>,
}

impl Default for VmOpcodeMapping {
    fn default() -> Self {
        Self {
            opcode_to_type: HashMap::new(),
            opcode_to_name: HashMap::new(),
            state_property_names: StatePropertyNames::default(),
            heuristics_applied: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatePropertyNames {
    pub memory_prop: Option<String>,
    pub pointer_prop: Option<String>,
    pub accumulator_prop: Option<String>,
}

pub struct VmOpcodeAnalyzer<'a> {
    found_switch: Option<&'a SwitchStatement<'a>>,
    case_count: usize,
    state_props: StatePropertyNames,
}

impl<'a> VmOpcodeAnalyzer<'a> {
    pub fn new(_allocator: &'a Allocator) -> Self {
        Self {
            found_switch: None,
            case_count: 0,
            state_props: StatePropertyNames::default(),
        }
    }

    pub fn analyze(&mut self, program: &'a Program<'a>) -> VmOpcodeMapping {
        self.find_vm_switch(program);
        self.map_opcodes()
    }

    fn find_vm_switch(&mut self, program: &'a Program<'a>) {
        let mut largest_switch: Option<&SwitchStatement<'a>> = None;
        let mut largest_case_count = 0;
        let mut switches_found: Vec<usize> = Vec::new();

        eprintln!(
            "[VmOpcodeAnalyzer] Program has {} top-level statements",
            program.body.len()
        );

        for (i, stmt) in program.body.iter().enumerate() {
            let stmt_type = format!("{:?}", stmt);
            eprintln!(
                "[VmOpcodeAnalyzer] Statement {} type: {}",
                i,
                &stmt_type[..stmt_type.len().min(100)]
            );
            self.find_switch_in_stmt(
                stmt,
                &mut largest_switch,
                &mut largest_case_count,
                &mut switches_found,
            );
        }

        eprintln!(
            "[VmOpcodeAnalyzer] All switches found: {:?}",
            switches_found
        );
        eprintln!(
            "[VmOpcodeAnalyzer] Switches found with >15 cases: {:?}",
            switches_found
                .iter()
                .filter(|&&c| c > 15)
                .collect::<Vec<_>>()
        );

        if let Some(switch_stmt) = largest_switch {
            self.case_count = switch_stmt.cases.len();
            self.detect_state_properties(switch_stmt);
            self.found_switch = Some(switch_stmt);
        }
    }

    fn find_switch_in_stmt(
        &self,
        stmt: &'a Statement<'_>,
        largest_switch: &mut Option<&'a SwitchStatement<'a>>,
        largest_case_count: &mut usize,
        switches_found: &mut Vec<usize>,
    ) {
        let stmt_type = format!("{:?}", stmt);
        let stmt_preview = if stmt_type.len() > 60 {
            &stmt_type[..60]
        } else {
            &stmt_type
        };
        eprintln!("[VmOpcodeAnalyzer] find_switch_in_stmt: {}", stmt_preview);

        if let Statement::SwitchStatement(switch_stmt) = stmt {
            let cases = switch_stmt.cases.len();
            eprintln!("[VmOpcodeAnalyzer] Found switch with {} cases!", cases);
            if cases > *largest_case_count && cases > 0 {
                *largest_case_count = cases;
                *largest_switch = Some(switch_stmt);
                switches_found.push(cases);
            }
        }

        if let Statement::FunctionDeclaration(func) = stmt {
            eprintln!("[VmOpcodeAnalyzer] Traversing FunctionDeclaration body");
            if let Some(ref body) = func.body {
                for s in &body.statements {
                    self.find_switch_in_stmt(s, largest_switch, largest_case_count, switches_found);
                }
            }
        }

        self.find_switch_recursive(stmt, largest_switch, largest_case_count, switches_found);
    }

    fn find_switch_recursive(
        &self,
        stmt: &'a Statement<'_>,
        largest_switch: &mut Option<&'a SwitchStatement<'a>>,
        largest_case_count: &mut usize,
        switches_found: &mut Vec<usize>,
    ) {
        match stmt {
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_switch_in_stmt(s, largest_switch, largest_case_count, switches_found);
                }
            }
            Statement::ExpressionStatement(expr_stmt) => {
                eprintln!("[VmOpcodeAnalyzer] Handling ExpressionStatement");
                self.find_switch_in_expr(
                    &expr_stmt.expression,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::IfStatement(if_stmt) => {
                self.find_switch_recursive(
                    &if_stmt.consequent,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                if let Some(ref alternate) = if_stmt.alternate {
                    self.find_switch_recursive(
                        alternate,
                        largest_switch,
                        largest_case_count,
                        switches_found,
                    );
                }
            }
            Statement::WhileStatement(while_stmt) => {
                self.find_switch_recursive(
                    &while_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::ForStatement(for_stmt) => {
                self.find_switch_recursive(
                    &for_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::ForInStatement(for_in_stmt) => {
                self.find_switch_recursive(
                    &for_in_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::ForOfStatement(for_of_stmt) => {
                self.find_switch_recursive(
                    &for_of_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::DoWhileStatement(do_while_stmt) => {
                self.find_switch_recursive(
                    &do_while_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::SwitchStatement(switch_stmt) => {
                let cases = switch_stmt.cases.len();
                eprintln!("[VmOpcodeAnalyzer] Found switch with {} cases", cases);
                if cases > *largest_case_count && cases > 0 {
                    *largest_case_count = cases;
                    *largest_switch = Some(switch_stmt);
                    switches_found.push(cases);
                }
            }
            Statement::VariableDeclaration(var_decl) => {
                for decl in &var_decl.declarations {
                    if let Some(ref init) = decl.init {
                        self.find_switch_in_expr(
                            init,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            Statement::WithStatement(with_stmt) => {
                self.find_switch_recursive(
                    &with_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::LabeledStatement(labeled_stmt) => {
                self.find_switch_recursive(
                    &labeled_stmt.body,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Statement::TryStatement(try_stmt) => {
                for s in &try_stmt.block.body {
                    self.find_switch_in_stmt(s, largest_switch, largest_case_count, switches_found);
                }
                if let Some(ref handler) = try_stmt.handler {
                    for s in &handler.body.body {
                        self.find_switch_in_stmt(
                            s,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
                if let Some(ref finalizer) = try_stmt.finalizer {
                    for s in &finalizer.body {
                        self.find_switch_in_stmt(
                            s,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn find_switch_in_expr(
        &self,
        expr: &'a Expression<'_>,
        largest_switch: &mut Option<&'a SwitchStatement<'a>>,
        largest_case_count: &mut usize,
        switches_found: &mut Vec<usize>,
    ) {
        let discriminant = std::mem::discriminant(expr);
        eprintln!(
            "[VmOpcodeAnalyzer] find_switch_in_expr called, discriminant: {:?}",
            discriminant
        );
        match expr {
            Expression::FunctionExpression(func) => {
                eprintln!("[VmOpcodeAnalyzer] FunctionExpression matched!");
                if let Some(ref body) = func.body {
                    eprintln!(
                        "[VmOpcodeAnalyzer] Function body has {} statements",
                        body.statements.len()
                    );
                    for s in &body.statements {
                        self.find_switch_in_stmt(
                            s,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            Expression::ArrowFunctionExpression(arrow) => {
                for s in &arrow.body.statements {
                    self.find_switch_in_stmt(s, largest_switch, largest_case_count, switches_found);
                }
            }
            Expression::CallExpression(call) => {
                self.find_switch_in_expr(
                    &call.callee,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                for arg in &call.arguments {
                    if let Some(expr) = arg.as_expression() {
                        self.find_switch_in_expr(
                            expr,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for expr in &seq.expressions {
                    self.find_switch_in_expr(
                        expr,
                        largest_switch,
                        largest_case_count,
                        switches_found,
                    );
                }
            }
            Expression::ConditionalExpression(cond) => {
                self.find_switch_in_expr(
                    &cond.test,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                self.find_switch_in_expr(
                    &cond.consequent,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                self.find_switch_in_expr(
                    &cond.alternate,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::AssignmentExpression(assign) => {
                self.find_switch_in_expr(
                    &assign.right,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::ObjectExpression(obj) => {
                for prop in &obj.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(obj_prop) => {
                            self.find_switch_in_expr(
                                &obj_prop.value,
                                largest_switch,
                                largest_case_count,
                                switches_found,
                            );
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.find_switch_in_expr(
                                &spread.argument,
                                largest_switch,
                                largest_case_count,
                                switches_found,
                            );
                        }
                    }
                }
            }
            Expression::ArrayExpression(arr) => {
                for elem in &arr.elements {
                    if let Some(expr) = elem.as_expression() {
                        self.find_switch_in_expr(
                            expr,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            Expression::ClassExpression(class) => {
                for item in &class.body.body {
                    match item {
                        oxc_ast::ast::ClassElement::MethodDefinition(m) => {
                            if let Some(ref body) = m.value.body {
                                for s in &body.statements {
                                    self.find_switch_in_stmt(
                                        s,
                                        largest_switch,
                                        largest_case_count,
                                        switches_found,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Expression::ChainExpression(chain) => {
                if let oxc_ast::ast::ChainElement::CallExpression(call) = &chain.expression {
                    self.find_switch_in_expr(
                        &call.callee,
                        largest_switch,
                        largest_case_count,
                        switches_found,
                    );
                    for arg in &call.arguments {
                        if let Some(expr) = arg.as_expression() {
                            self.find_switch_in_expr(
                                expr,
                                largest_switch,
                                largest_case_count,
                                switches_found,
                            );
                        }
                    }
                }
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.find_switch_in_expr(
                    &tagged.tag,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::LogicalExpression(logical) => {
                self.find_switch_in_expr(
                    &logical.left,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                self.find_switch_in_expr(
                    &logical.right,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::NewExpression(new_expr) => {
                self.find_switch_in_expr(
                    &new_expr.callee,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                for arg in &new_expr.arguments {
                    if let Some(expr) = arg.as_expression() {
                        self.find_switch_in_expr(
                            expr,
                            largest_switch,
                            largest_case_count,
                            switches_found,
                        );
                    }
                }
            }
            Expression::YieldExpression(yield_expr) => {
                if let Some(ref arg) = yield_expr.argument {
                    self.find_switch_in_expr(
                        arg,
                        largest_switch,
                        largest_case_count,
                        switches_found,
                    );
                }
            }
            Expression::AwaitExpression(await_expr) => {
                self.find_switch_in_expr(
                    &await_expr.argument,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::UnaryExpression(unary) => {
                self.find_switch_in_expr(
                    &unary.argument,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::ParenthesizedExpression(paren) => {
                self.find_switch_in_expr(
                    &paren.expression,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::StaticMemberExpression(member) => {
                self.find_switch_in_expr(
                    &member.object,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            Expression::ComputedMemberExpression(member) => {
                self.find_switch_in_expr(
                    &member.object,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
                self.find_switch_in_expr(
                    &member.expression,
                    largest_switch,
                    largest_case_count,
                    switches_found,
                );
            }
            _ => {}
        }
    }

    fn detect_state_properties(&mut self, switch_stmt: &'a SwitchStatement<'a>) {
        let mut this_props: FxHashMap<String, usize> = FxHashMap::default();

        for case in &switch_stmt.cases {
            for stmt in &case.consequent {
                self.extract_this_properties(stmt, &mut this_props);
            }
        }

        for prop in ["h", "memory", "mem", "state", "data"] {
            if this_props.contains_key(prop) {
                self.state_props.memory_prop = Some(prop.to_string());
                break;
            }
        }

        for prop in ["g", "ptr", "pointer", "ip", "pc", "counter"] {
            if this_props.contains_key(prop) {
                self.state_props.pointer_prop = Some(prop.to_string());
                break;
            }
        }

        for prop in ["a", "acc", "accumulator", "result", "sum"] {
            if this_props.contains_key(prop) {
                self.state_props.accumulator_prop = Some(prop.to_string());
                break;
            }
        }
    }

    fn extract_this_properties(&self, stmt: &Statement, props: &mut FxHashMap<String, usize>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.extract_from_expr(&expr_stmt.expression, props);
            }
            Statement::IfStatement(if_stmt) => {
                self.extract_from_expr(&if_stmt.test, props);
                self.extract_this_properties(&if_stmt.consequent, props);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.extract_this_properties(s, props);
                }
            }
            _ => {}
        }
    }

    fn extract_from_expr(&self, expr: &Expression, props: &mut FxHashMap<String, usize>) {
        match expr {
            Expression::AssignmentExpression(assign) => {
                self.extract_from_expr(&assign.right, props);
            }
            Expression::StaticMemberExpression(member) => {
                if matches!(member.object, Expression::ThisExpression(_)) {
                    *props.entry(member.property.name.to_string()).or_insert(0) += 1;
                }
                self.extract_from_expr(&member.object, props);
            }
            Expression::ComputedMemberExpression(member) => {
                self.extract_from_expr(&member.object, props);
                self.extract_from_expr(&member.expression, props);
            }
            Expression::BinaryExpression(bin) => {
                self.extract_from_expr(&bin.left, props);
                self.extract_from_expr(&bin.right, props);
            }
            Expression::UnaryExpression(unary) => {
                self.extract_from_expr(&unary.argument, props);
            }
            _ => {}
        }
    }

    fn map_opcodes(&self) -> VmOpcodeMapping {
        let mut mapping = VmOpcodeMapping::default();
        mapping.state_property_names = self.state_props.clone();

        if let Some(switch_stmt) = self.found_switch {
            for (idx, case) in switch_stmt.cases.iter().enumerate() {
                let opcode = case
                    .test
                    .as_ref()
                    .and_then(|t| self.extract_numeric(t))
                    .unwrap_or(idx as i64);
                let instr_type = self.classify_case(case);
                mapping.opcode_to_type.insert(opcode, instr_type);
                mapping
                    .opcode_to_name
                    .insert(opcode, format!("OP_{}_{:?}", opcode, instr_type));
            }
        }

        mapping
    }

    fn extract_numeric(&self, test: &Expression) -> Option<i64> {
        match test {
            Expression::NumericLiteral(lit) => Some(lit.value as i64),
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::UnaryNegation =>
            {
                self.extract_numeric(&unary.argument).map(|n| -n)
            }
            _ => None,
        }
    }

    fn classify_case(&self, case: &SwitchCase) -> VmInstructionType {
        let mut flags = InstructionFlags::default();

        for stmt in &case.consequent {
            self.analyze_statement(stmt, &mut flags);
        }

        if flags.has_return {
            return VmInstructionType::Return;
        }
        if flags.has_binary_op {
            if let Some(op) = flags.binary_op {
                match op {
                    BinaryOperator::BitwiseXOR => return VmInstructionType::XorOp,
                    BinaryOperator::Addition => return VmInstructionType::AddOp,
                    BinaryOperator::Subtraction => return VmInstructionType::SubOp,
                    BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan => {
                        return VmInstructionType::Compare;
                    }
                    _ => {}
                }
            }
        }
        if flags.has_this_write && flags.has_member_access {
            return VmInstructionType::MemoryWrite;
        }
        if flags.has_jump {
            return VmInstructionType::Jump;
        }
        if flags.has_this_access && !flags.has_assignment {
            return VmInstructionType::MemoryRead;
        }

        VmInstructionType::Unknown
    }

    fn analyze_statement(&self, stmt: &Statement, flags: &mut InstructionFlags) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.analyze_expr(&expr_stmt.expression, flags);
            }
            Statement::ReturnStatement(_ret) => {
                flags.has_return = true;
            }
            Statement::BreakStatement(_) | Statement::ContinueStatement(_) => {
                flags.has_jump = true;
            }
            Statement::IfStatement(if_stmt) => {
                self.analyze_expr(&if_stmt.test, flags);
                self.analyze_statement(&if_stmt.consequent, flags);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.analyze_statement(s, flags);
                }
            }
            _ => {}
        }
    }

    fn analyze_expr(&self, expr: &Expression, flags: &mut InstructionFlags) {
        match expr {
            Expression::AssignmentExpression(assign) => {
                flags.has_assignment = true;
                flags.has_binary_op = true;
                if flags.binary_op.is_none() {
                    match assign.operator {
                        AssignmentOperator::BitwiseXOR => {
                            flags.binary_op = Some(BinaryOperator::BitwiseXOR);
                        }
                        AssignmentOperator::Addition => {
                            flags.binary_op = Some(BinaryOperator::Addition);
                        }
                        AssignmentOperator::Subtraction => {
                            flags.binary_op = Some(BinaryOperator::Subtraction);
                        }
                        _ => {}
                    }
                }
                self.analyze_expr(&assign.right, flags);
            }
            Expression::BinaryExpression(bin) => {
                flags.has_binary_op = true;
                if flags.binary_op.is_none() {
                    flags.binary_op = Some(bin.operator);
                }
                self.analyze_expr(&bin.left, flags);
                self.analyze_expr(&bin.right, flags);
            }
            Expression::StaticMemberExpression(member) => {
                flags.has_member_access = true;
                if matches!(member.object, Expression::ThisExpression(_)) {
                    flags.has_this_access = true;
                }
                self.analyze_expr(&member.object, flags);
            }
            Expression::ComputedMemberExpression(member) => {
                flags.has_member_access = true;
                self.analyze_expr(&member.object, flags);
                self.analyze_expr(&member.expression, flags);
            }
            Expression::ThisExpression(_) => {
                flags.has_this_access = true;
            }
            Expression::CallExpression(call) => {
                self.analyze_expr(&call.callee, flags);
            }
            Expression::UpdateExpression(_update) => {
                flags.has_this_write = true;
            }
            Expression::UnaryExpression(unary) => {
                self.analyze_expr(&unary.argument, flags);
            }
            Expression::ConditionalExpression(cond) => {
                self.analyze_expr(&cond.test, flags);
                self.analyze_expr(&cond.consequent, flags);
                self.analyze_expr(&cond.alternate, flags);
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct InstructionFlags {
    has_assignment: bool,
    has_binary_op: bool,
    binary_op: Option<BinaryOperator>,
    has_this_access: bool,
    has_this_write: bool,
    has_jump: bool,
    has_return: bool,
    has_member_access: bool,
}

pub fn analyze_vm_opcodes(js_code: &str) -> Result<VmOpcodeMapping> {
    let allocator = Allocator::default();
    let source_type = SourceType::default().with_module(false);

    let parsed = Parser::new(&allocator, js_code, source_type).parse();

    if !parsed.errors.is_empty() {
        anyhow::bail!("Parse errors: {:?}", parsed.errors);
    }

    let program = allocator.alloc(parsed.program);
    let mut analyzer = VmOpcodeAnalyzer::new(&allocator);
    let mapping = analyzer.analyze(program);

    if mapping.opcode_to_type.is_empty() {
        anyhow::bail!("No VM switch statement found (>15 cases required)");
    }

    Ok(mapping)
}

pub fn export_opcode_map(mapping: &VmOpcodeMapping, path: &str) -> Result<()> {
    let json =
        serde_json::to_string_pretty(mapping).context("Failed to serialize opcode mapping")?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_opcode_map(path: &str) -> Result<VmOpcodeMapping> {
    let content = std::fs::read_to_string(path).context("Failed to read opcode map")?;
    serde_json::from_str(&content).context("Failed to parse opcode map JSON")
}

pub fn get_default_opcode_map() -> VmOpcodeMapping {
    let mut mapping = VmOpcodeMapping::default();

    mapping.opcode_to_type.insert(0, VmInstructionType::Return);
    mapping.opcode_to_type.insert(1, VmInstructionType::Jump);
    mapping
        .opcode_to_type
        .insert(2, VmInstructionType::MemoryRead);
    mapping
        .opcode_to_type
        .insert(3, VmInstructionType::MemoryWrite);
    mapping.opcode_to_type.insert(4, VmInstructionType::XorOp);
    mapping.opcode_to_type.insert(5, VmInstructionType::AddOp);
    mapping.opcode_to_type.insert(6, VmInstructionType::SubOp);
    mapping
        .opcode_to_type
        .insert(7, VmInstructionType::PushConstant);
    mapping.opcode_to_type.insert(8, VmInstructionType::Pop);
    mapping.opcode_to_type.insert(9, VmInstructionType::Compare);
    mapping
        .opcode_to_type
        .insert(10, VmInstructionType::Duplicate);
    mapping.opcode_to_type.insert(11, VmInstructionType::Swap);
    mapping.opcode_to_type.insert(12, VmInstructionType::Load);
    mapping.opcode_to_type.insert(13, VmInstructionType::Store);
    mapping.opcode_to_type.insert(14, VmInstructionType::Call);
    mapping
        .opcode_to_type
        .insert(15, VmInstructionType::ConditionalJump);

    for (opcode, instr_type) in &mapping.opcode_to_type {
        mapping
            .opcode_to_name
            .insert(*opcode, format!("OP_{}_{:?}", opcode, instr_type));
    }

    mapping.state_property_names = StatePropertyNames {
        memory_prop: Some("h".to_string()),
        pointer_prop: Some("g".to_string()),
        accumulator_prop: Some("a".to_string()),
    };

    mapping
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_VM: &str = r#"
    function VM() {
        this.g = 0;
        this.h = [];
        this.a = 0;
        this.c = function() {
            while (this.g < 256) {
                switch (this[this.g ^ 52]) {
                    case 0: return;
                    case 1: this.g += 2; break;
                    case 2: this.a = this.h[this.g ^ 52]; break;
                    case 3: this.h[this.g ^ 52] = this.a; this.g++; break;
                    case 4: this.a ^= this.h[this.g ^ 52]; this.g++; break;
                    case 5: this.a += this.h[this.g ^ 52]; this.g++; break;
                    case 6: this.a -= this.h[this.g ^ 52]; this.g++; break;
                    case 7: this.a = 42; this.g++; break;
                    case 8: this.h.pop(); this.g++; break;
                    case 9: if (this.a == 0) this.g++; break;
                    case 10: this.h.push(0); this.g++; break;
                    case 11: var t = this.h.pop(); this.h.push(this.a); this.a = t; this.g++; break;
                    case 12: this.a = this.h.length; this.g++; break;
                    case 13: this.h[this.g ^ 70] = this.a; this.g++; break;
                    case 14: this.a = this.h[this.g ^ 70]; this.g++; break;
                    case 15: this.g = this.h[this.g ^ 70]; break;
                    case 16: this.a++; this.g++; break;
                }
            }
        };
    }
    "#;

    #[test]
    fn test_analyze_vm_opcodes() {
        let mapping = analyze_vm_opcodes(TEST_VM).unwrap();
        assert!(!mapping.opcode_to_type.is_empty());
        assert!(mapping.opcode_to_type.len() >= 10);
        assert_eq!(
            mapping.opcode_to_type.get(&0),
            Some(&VmInstructionType::Return)
        );
        assert_eq!(
            mapping.opcode_to_type.get(&4),
            Some(&VmInstructionType::XorOp)
        );
    }

    #[test]
    fn test_default_map() {
        let default = get_default_opcode_map();
        assert_eq!(default.opcode_to_type.len(), 16);
        assert_eq!(
            default.opcode_to_type.get(&0),
            Some(&VmInstructionType::Return)
        );
    }
}
