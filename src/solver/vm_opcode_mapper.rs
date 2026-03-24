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
    BitwiseOp,
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
            VmInstructionType::BitwiseOp => write!(f, "BITWISE_OP"),
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
    discovered_memory_prop: Option<String>,
    discovered_pointer_prop: Option<String>,
    discovered_accumulator_prop: Option<String>,
}

impl<'a> VmOpcodeAnalyzer<'a> {
    pub fn new(_allocator: &'a Allocator) -> Self {
        Self {
            found_switch: None,
            case_count: 0,
            state_props: StatePropertyNames::default(),
            discovered_memory_prop: None,
            discovered_pointer_prop: None,
            discovered_accumulator_prop: None,
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
        let mut ptr_candidates: FxHashMap<String, usize> = FxHashMap::default();
        let mut mem_candidates: FxHashMap<String, usize> = FxHashMap::default();
        let mut acc_candidates: FxHashMap<String, usize> = FxHashMap::default();

        for case in &switch_stmt.cases {
            for stmt in &case.consequent {
                self.find_local_pointer_increments(stmt, &mut ptr_candidates);
                self.find_local_memory_access(stmt, &mut mem_candidates);
                self.find_local_accumulator_usage(stmt, &mut acc_candidates);
            }
        }

        eprintln!("[DEBUG] ptr_candidates: {:?}", ptr_candidates);
        eprintln!("[DEBUG] mem_candidates: {:?}", mem_candidates);
        eprintln!("[DEBUG] acc_candidates: {:?}", acc_candidates);

        if let Some(ptr_name) = self.find_best_candidate(ptr_candidates) {
            self.discovered_pointer_prop = Some(ptr_name.clone());
            self.state_props.pointer_prop = Some(ptr_name);
            eprintln!("[DEBUG] Discovered pointer: {}", ptr_name);
        }

        if let Some(mem_name) = self.find_best_candidate(mem_candidates) {
            self.discovered_memory_prop = Some(mem_name.clone());
            self.state_props.memory_prop = Some(mem_name.clone());
            eprintln!("[DEBUG] Discovered memory: {}", mem_name);
        }

        if let Some(acc_name) = self.find_best_candidate(acc_candidates) {
            self.discovered_accumulator_prop = Some(acc_name.clone());
            self.state_props.accumulator_prop = Some(acc_name.clone());
            eprintln!("[DEBUG] Discovered accumulator: {}", acc_name);
        }
    }

    fn find_best_candidate(&self, mut candidates: FxHashMap<String, usize>) -> Option<String> {
        candidates.retain(|_, &mut v| v > 0);
        candidates.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k)
    }

    fn find_local_pointer_increments(&self, stmt: &Statement, candidates: &mut FxHashMap<String, usize>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.find_local_pointer_increments_expr(&expr_stmt.expression, candidates);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_local_pointer_increments(s, candidates);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.find_local_pointer_increments(&if_stmt.consequent, candidates);
            }
            _ => {}
        }
    }

    fn find_local_pointer_increments_expr(&self, expr: &Expression, candidates: &mut FxHashMap<String, usize>) {
        match expr {
            Expression::UpdateExpression(update) => {
                if update.operator == UpdateOperator::Increment || update.operator == UpdateOperator::Decrement {
                    if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument {
                        *candidates.entry(id.name.to_string()).or_insert(0) += 10;
                    } else if let SimpleAssignmentTarget::StaticMemberExpression(member) = &update.argument {
                        if matches!(member.object, Expression::ThisExpression(_)) {
                            *candidates.entry(member.property.name.to_string()).or_insert(0) += 10;
                        }
                    }
                }
            }
            Expression::AssignmentExpression(assign) => {
                if matches!(assign.operator, AssignmentOperator::Addition | AssignmentOperator::Subtraction) {
                    if let Some(simple) = assign.left.as_simple_assignment_target() {
                        match simple {
                            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                                *candidates.entry(id.name.to_string()).or_insert(0) += 8;
                            }
                            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                                if matches!(member.object, Expression::ThisExpression(_)) {
                                    *candidates.entry(member.property.name.to_string()).or_insert(0) += 8;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                self.find_local_pointer_increments_expr(&assign.right, candidates);
            }
            Expression::CallExpression(call) => {
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.find_local_pointer_increments_expr(e, candidates);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.find_local_pointer_increments_expr(e, candidates);
                }
            }
            _ => {}
        }
    }

    fn find_local_memory_access(&self, stmt: &Statement, candidates: &mut FxHashMap<String, usize>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.find_local_memory_access_expr(&expr_stmt.expression, candidates);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_local_memory_access(s, candidates);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.find_local_memory_access(&if_stmt.consequent, candidates);
            }
            _ => {}
        }
    }

    fn find_local_memory_access_expr(&self, expr: &Expression, candidates: &mut FxHashMap<String, usize>) {
        match expr {
            Expression::ComputedMemberExpression(member) => {
                if let Expression::Identifier(id) = &member.object {
                    *candidates.entry(id.name.to_string()).or_insert(0) += 15;
                }
                self.find_local_memory_access_expr(&member.object, candidates);
                self.find_local_memory_access_expr(&member.expression, candidates);
            }
            Expression::AssignmentExpression(assign) => {
                self.find_local_memory_access_expr(&assign.right, candidates);
            }
            Expression::CallExpression(call) => {
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.find_local_memory_access_expr(e, candidates);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.find_local_memory_access_expr(e, candidates);
                }
            }
            _ => {}
        }
    }

    fn find_local_accumulator_usage(&self, stmt: &Statement, candidates: &mut FxHashMap<String, usize>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.find_local_accumulator_expr(&expr_stmt.expression, candidates);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_local_accumulator_usage(s, candidates);
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.find_local_accumulator_expr(arg, candidates);
                }
            }
            _ => {}
        }
    }

    fn find_local_accumulator_expr(&self, expr: &Expression, candidates: &mut FxHashMap<String, usize>) {
        match expr {
            Expression::AssignmentExpression(assign) => {
                if let Some(simple) = assign.left.as_simple_assignment_target() {
                    match simple {
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                            let is_complex = matches!(
                                assign.right,
                                Expression::BinaryExpression(_)
                                | Expression::CallExpression(_)
                                | Expression::ComputedMemberExpression(_)
                            );
                            let weight = if is_complex { 15 } else { 3 };
                            *candidates.entry(id.name.to_string()).or_insert(0) += weight;
                        }
                        SimpleAssignmentTarget::StaticMemberExpression(member) => {
                            if matches!(member.object, Expression::ThisExpression(_)) {
                                *candidates.entry(member.property.name.to_string()).or_insert(0) += 15;
                            }
                        }
                        _ => {}
                    }
                }
                self.find_local_accumulator_expr(&assign.right, candidates);
            }
            Expression::CallExpression(call) => {
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.find_local_accumulator_expr(e, candidates);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.find_local_accumulator_expr(e, candidates);
                }
            }
            _ => {}
        }
    }

    fn is_small_numeric(&self, expr: &Expression) -> bool {
        match expr {
            Expression::NumericLiteral(lit) => lit.value.abs() < 100.0,
            Expression::UnaryExpression(unary) => {
                if unary.operator == UnaryOperator::UnaryNegation {
                    self.is_small_numeric(&unary.argument)
                } else {
                    false
                }
            }
            _ => false
        }
    }

    fn find_memory_array_access(&self, stmt: &Statement, candidates: &mut FxHashMap<String, usize>, ptr_name: Option<&String>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.find_memory_array_access_expr(&expr_stmt.expression, candidates, ptr_name);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_memory_array_access(s, candidates, ptr_name);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.find_memory_array_access(&if_stmt.consequent, candidates, ptr_name);
            }
            _ => {}
        }
    }

    fn find_memory_array_access_expr(&self, expr: &Expression, candidates: &mut FxHashMap<String, usize>, ptr_name: Option<&String>) {
        match expr {
            Expression::ComputedMemberExpression(member) => {
                if let Expression::StaticMemberExpression(obj) = &member.object {
                    if matches!(obj.object, Expression::ThisExpression(_)) {
                        let mem_name = obj.property.name.to_string();
                        if let Some(ptr) = ptr_name {
                            if self.expr_contains_xor_with_ptr(&member.expression, ptr) {
                                *candidates.entry(mem_name).or_insert(0) += 20;
                            }
                        } else {
                            if self.contains_any_binary_expr(&member.expression) {
                                *candidates.entry(mem_name).or_insert(0) += 5;
                            }
                        }
                    }
                }
                self.find_memory_array_access_expr(&member.object, candidates, ptr_name);
                self.find_memory_array_access_expr(&member.expression, candidates, ptr_name);
            }
            Expression::AssignmentExpression(assign) => {
                self.find_memory_array_access_expr(&assign.right, candidates, ptr_name);
            }
            Expression::CallExpression(call) => {
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.find_memory_array_access_expr(e, candidates, ptr_name);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.find_memory_array_access_expr(e, candidates, ptr_name);
                }
            }
            _ => {}
        }
    }

    fn expr_contains_xor_with_ptr(&self, expr: &Expression, ptr_name: &str) -> bool {
        match expr {
            Expression::BinaryExpression(bin) => {
                if bin.operator == BinaryOperator::BitwiseXOR {
                    let left_has_ptr = self.expr_has_this_property(&bin.left, ptr_name);
                    let right_has_ptr = self.expr_has_this_property(&bin.right, ptr_name);
                    return left_has_ptr || right_has_ptr;
                }
                self.expr_contains_xor_with_ptr(&bin.left, ptr_name) || 
                self.expr_contains_xor_with_ptr(&bin.right, ptr_name)
            }
            _ => false
        }
    }

    fn expr_has_this_property(&self, expr: &Expression, prop_name: &str) -> bool {
        match expr {
            Expression::StaticMemberExpression(member) => {
                if matches!(member.object, Expression::ThisExpression(_)) {
                    return member.property.name == prop_name;
                }
                self.expr_has_this_property(&member.object, prop_name)
            }
            Expression::ParenthesizedExpression(paren) => {
                self.expr_has_this_property(&paren.expression, prop_name)
            }
            _ => false
        }
    }

    fn contains_any_binary_expr(&self, expr: &Expression) -> bool {
        match expr {
            Expression::BinaryExpression(_) => true,
            Expression::StaticMemberExpression(member) => {
                self.contains_any_binary_expr(&member.object)
            }
            _ => false
        }
    }

    fn find_accumulator_assignments(&self, stmt: &Statement, candidates: &mut FxHashMap<String, usize>) {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.find_accumulator_assignments_expr(&expr_stmt.expression, candidates);
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.find_accumulator_assignments(s, candidates);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.find_accumulator_assignments(&if_stmt.consequent, candidates);
            }
            _ => {}
        }
    }

    fn find_accumulator_assignments_expr(&self, expr: &Expression, candidates: &mut FxHashMap<String, usize>) {
        match expr {
            Expression::AssignmentExpression(assign) => {
                if let Some(simple) = assign.left.as_simple_assignment_target() {
                    if let SimpleAssignmentTarget::StaticMemberExpression(member) = simple {
                        if matches!(member.object, Expression::ThisExpression(_)) {
                            let name = member.property.name.to_string();
                            if self.expr_contains_memory_access(&assign.right) {
                                *candidates.entry(name).or_insert(0) += 15;
                            } else if self.is_literal(&assign.right) {
                                *candidates.entry(name).or_insert(0) += 3;
                            }
                        }
                    }
                }
                self.find_accumulator_assignments_expr(&assign.right, candidates);
            }
            Expression::CallExpression(call) => {
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.find_accumulator_assignments_expr(e, candidates);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.find_accumulator_assignments_expr(e, candidates);
                }
            }
            _ => {}
        }
    }

    fn expr_contains_memory_access(&self, expr: &Expression) -> bool {
        match expr {
            Expression::ComputedMemberExpression(_) => true,
            Expression::StaticMemberExpression(member) => {
                self.expr_contains_memory_access(&member.object)
            }
            Expression::BinaryExpression(bin) => {
                self.expr_contains_memory_access(&bin.left) || self.expr_contains_memory_access(&bin.right)
            }
            Expression::UnaryExpression(unary) => {
                self.expr_contains_memory_access(&unary.argument)
            }
            _ => false
        }
    }

    fn is_literal(&self, expr: &Expression) -> bool {
        matches!(
            expr,
            Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BooleanLiteral(_)
        )
    }

    fn find_best_pointer(&self, candidates: FxHashMap<String, usize>) -> Option<String> {
        candidates.into_iter().max_by_key(|(_, count)| *count).map(|(name, _)| name)
    }

    fn find_best_memory(&self, candidates: FxHashMap<String, usize>) -> Option<String> {
        candidates.into_iter().max_by_key(|(_, count)| *count).map(|(name, _)| name)
    }

    fn find_best_accumulator(&self, candidates: FxHashMap<String, usize>) -> Option<String> {
        candidates.into_iter().max_by_key(|(_, count)| *count).map(|(name, _)| name)
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

        if flags.has_memory_write {
            return VmInstructionType::MemoryWrite;
        }

        if flags.has_accumulator_write {
            return VmInstructionType::MemoryWrite;
        }

        if flags.has_compound_assignment {
            if let Some(op) = flags.compound_assignment_op {
                match op {
                    BinaryOperator::BitwiseXOR => return VmInstructionType::XorOp,
                    BinaryOperator::Addition => return VmInstructionType::AddOp,
                    BinaryOperator::Subtraction => return VmInstructionType::SubOp,
                    _ => {}
                }
            }
        }

        if flags.has_memory_read {
            return VmInstructionType::MemoryRead;
        }

        if flags.has_accumulator_read {
            return VmInstructionType::MemoryRead;
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
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan => {
                        return VmInstructionType::Compare;
                    }
                    BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill => {
                        return VmInstructionType::BitwiseOp;
                    }
                    BinaryOperator::BitwiseAnd => return VmInstructionType::BitwiseOp,
                    BinaryOperator::BitwiseOR => return VmInstructionType::BitwiseOp,
                    BinaryOperator::Remainder => return VmInstructionType::BitwiseOp,
                    BinaryOperator::Multiplication => return VmInstructionType::BitwiseOp,
                    BinaryOperator::Division => return VmInstructionType::BitwiseOp,
                    _ => {}
                }
            }
        }

        if flags.has_jump {
            return VmInstructionType::Jump;
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
            Statement::BreakStatement(_) => {
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
                match assign.operator {
                    AssignmentOperator::BitwiseXOR => {
                        flags.has_compound_assignment = true;
                        flags.compound_assignment_op = Some(BinaryOperator::BitwiseXOR);
                        flags.has_binary_op = true;
                        flags.binary_op = Some(BinaryOperator::BitwiseXOR);
                    }
                    AssignmentOperator::Addition => {
                        flags.has_compound_assignment = true;
                        flags.compound_assignment_op = Some(BinaryOperator::Addition);
                        flags.has_binary_op = true;
                        flags.binary_op = Some(BinaryOperator::Addition);
                    }
                    AssignmentOperator::Subtraction => {
                        flags.has_compound_assignment = true;
                        flags.compound_assignment_op = Some(BinaryOperator::Subtraction);
                        flags.has_binary_op = true;
                        flags.binary_op = Some(BinaryOperator::Subtraction);
                    }
                    _ => {}
                }
                
                if let Some(simple) = assign.left.as_simple_assignment_target() {
                    match simple {
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                            if let Some(ref mem_name) = self.discovered_memory_prop {
                                if id.name == mem_name {
                                    flags.has_memory_write = true;
                                }
                            }
                            if let Some(ref acc_name) = self.discovered_accumulator_prop {
                                if id.name == acc_name {
                                    flags.has_accumulator_write = true;
                                }
                            }
                        }
                        SimpleAssignmentTarget::StaticMemberExpression(member) => {
                            if matches!(member.object, Expression::ThisExpression(_)) {
                                if let Some(ref mem_name) = self.discovered_memory_prop {
                                    if member.property.name == mem_name {
                                        flags.has_memory_write = true;
                                    }
                                }
                                if let Some(ref acc_name) = self.discovered_accumulator_prop {
                                    if member.property.name == acc_name {
                                        flags.has_accumulator_write = true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                self.analyze_expr(&assign.right, flags);
            }
            Expression::BinaryExpression(bin) => {
                flags.has_binary_op = true;
                if flags.binary_op.is_none() && !flags.has_compound_assignment {
                    flags.binary_op = Some(bin.operator);
                }
                self.analyze_expr(&bin.left, flags);
                self.analyze_expr(&bin.right, flags);
            }
            Expression::StaticMemberExpression(member) => {
                if matches!(member.object, Expression::ThisExpression(_)) {
                    if let Some(ref mem_name) = self.discovered_memory_prop {
                        if member.property.name == mem_name {
                            flags.has_memory_read = true;
                        }
                    }
                    if let Some(ref acc_name) = self.discovered_accumulator_prop {
                        if member.property.name == acc_name {
                            flags.has_accumulator_read = true;
                        }
                    }
                }
                self.analyze_expr(&member.object, flags);
            }
            Expression::ComputedMemberExpression(member) => {
                if let Expression::Identifier(id) = &member.object {
                    if let Some(ref mem_name) = self.discovered_memory_prop {
                        if id.name == mem_name {
                            flags.has_memory_read = true;
                        }
                    }
                }
                self.analyze_expr(&member.object, flags);
                self.analyze_expr(&member.expression, flags);
            }
            Expression::CallExpression(call) => {
                self.analyze_expr(&call.callee, flags);
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.analyze_expr(e, flags);
                    }
                }
            }
            Expression::UpdateExpression(update) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument {
                    if let Some(ref ptr_name) = self.discovered_pointer_prop {
                        if id.name == ptr_name {
                        }
                    }
                    if let Some(ref acc_name) = self.discovered_accumulator_prop {
                        if id.name == acc_name {
                            flags.has_accumulator_write = true;
                        }
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.analyze_expr(e, flags);
                }
            }
            Expression::ConditionalExpression(cond) => {
                self.analyze_expr(&cond.test, flags);
                self.analyze_expr(&cond.consequent, flags);
                self.analyze_expr(&cond.alternate, flags);
            }
            _ => {}
        }
    }

    fn is_this_property(&self, member: &StaticMemberExpression, prop_name: &str) -> bool {
        if matches!(member.object, Expression::ThisExpression(_)) {
            return member.property.name == prop_name;
        }
        false
    }

    fn is_memory_array_access(&self, expr: &Expression) -> bool {
        if let Expression::StaticMemberExpression(member) = expr {
            if let Some(ref mem_name) = self.discovered_memory_prop {
                return self.is_this_property(member, mem_name);
            }
        }
        false
    }

    fn is_pointer_property(&self, member: &StaticMemberExpression) -> bool {
        if let Some(ref ptr_name) = self.discovered_pointer_prop {
            return self.is_this_property(member, ptr_name);
        }
        false
    }

    fn is_accumulator_property(&self, member: &StaticMemberExpression) -> bool {
        if let Some(ref acc_name) = self.discovered_accumulator_prop {
            return self.is_this_property(member, acc_name);
        }
        false
    }

    fn is_this_h_array_access(&self, expr: &Expression) -> bool {
        if let Expression::StaticMemberExpression(member) = expr {
            if let Some(ref mem_name) = self.discovered_memory_prop {
                if member.property.name == mem_name {
                    return true;
                }
            }
            if member.property.name == "h" {
                return true;
            }
        }
        false
    }

    fn is_member_expr_this_h_access(&self, member: &StaticMemberExpression) -> bool {
        if let Some(ref mem_name) = self.discovered_memory_prop {
            if member.property.name == mem_name {
                return true;
            }
        }
        if member.property.name == "h" {
            return true;
        }
        false
    }

    fn assignment_target_is_this_h_write(&self, target: &AssignmentTarget) -> bool {
        match target {
            AssignmentTarget::StaticMemberExpression(member_expr) => {
                if let Some(ref mem_name) = self.discovered_memory_prop {
                    if member_expr.property.name == mem_name {
                        return true;
                    }
                }
                if member_expr.property.name == "h" {
                    return true;
                }
            }
            _ => {}
        }
        false
    }
}

#[derive(Default)]
struct InstructionFlags {
    has_return: bool,
    has_jump: bool,
    has_binary_op: bool,
    binary_op: Option<BinaryOperator>,
    has_compound_assignment: bool,
    compound_assignment_op: Option<BinaryOperator>,
    has_memory_write: bool,
    has_memory_read: bool,
    has_accumulator_write: bool,
    has_accumulator_read: bool,
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
