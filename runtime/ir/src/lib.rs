#![forbid(unsafe_code)]

use std::collections::HashMap;

use tsvm_ast::{
    AssignmentOperator, BinaryOperator, Expression, ExpressionKind, FunctionDeclaration, Program,
    Span, Statement, StatementKind, TypeKind, TypeNode, VariableDeclaration,
};
use tsvm_parser::parse_source;
use tsvm_semantic::{analyze_source, SemanticDiagnostic};

#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
    pub entry: IrFunction,
    pub source_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParam>,
    pub return_type: IrType,
    pub blocks: Vec<IrBlock>,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrBlock {
    pub id: BlockId,
    pub instructions: Vec<IrInstruction>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct IrInstruction {
    pub kind: IrInstructionKind,
    pub result: Option<ValueId>,
    pub ty: IrType,
    pub source_span: Span,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum IrInstructionKind {
    LoadConst(IrConst),
    LoadLocal(String),
    StoreLocal(String, ValueId),
    LoadMember {
        object: ValueId,
        property: String,
    },
    StoreMember {
        object: ValueId,
        property: String,
        value: ValueId,
    },
    Binary {
        op: IrBinaryOp,
        left: ValueId,
        right: ValueId,
    },
    Call {
        callee: String,
        args: Vec<ValueId>,
    },
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Jump(BlockId),
    Return(Option<ValueId>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrConst {
    Number(String),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object(Vec<(String, ValueId)>),
    Array(Vec<ValueId>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IrBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    StrictEqual,
    NotEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
    NullishCoalesce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
    Object(Vec<(String, IrType)>),
    Array(Box<IrType>),
    Named(String),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoweredProgram {
    pub ir: Option<IrProgram>,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

pub fn lower_source(source: &str) -> LoweredProgram {
    let semantic = analyze_source(source);
    if !semantic.diagnostics.is_empty() {
        return LoweredProgram {
            ir: None,
            diagnostics: semantic.diagnostics,
        };
    }

    let parsed = parse_source(source);
    let mut lowerer = ProgramLowerer;
    let ir = lowerer.lower_program(&parsed.program);
    LoweredProgram {
        ir: Some(ir),
        diagnostics: Vec::new(),
    }
}

struct ProgramLowerer;

impl ProgramLowerer {
    fn lower_program(&mut self, program: &Program) -> IrProgram {
        let mut functions = Vec::new();
        let mut entry_statements = Vec::new();

        for statement in &program.body {
            match &statement.kind {
                StatementKind::Function(function) => {
                    functions.push(self.lower_function(function, statement.span));
                }
                StatementKind::Interface(_)
                | StatementKind::TypeAlias(_)
                | StatementKind::Class(_) => {}
                _ => entry_statements.push(statement.clone()),
            }
        }

        let entry_span = program.span.unwrap_or_else(empty_span);
        let mut entry_lowerer =
            FunctionLowerer::new("__entry", Vec::new(), IrType::Undefined, entry_span);
        for statement in &entry_statements {
            entry_lowerer.lower_statement(statement);
        }
        entry_lowerer.emit(
            IrInstructionKind::Return(None),
            None,
            IrType::Undefined,
            entry_span,
        );

        IrProgram {
            functions,
            entry: entry_lowerer.finish(),
            source_span: program.span,
        }
    }

    fn lower_function(&mut self, function: &FunctionDeclaration, span: Span) -> IrFunction {
        let params = function
            .params
            .iter()
            .map(|param| IrParam {
                name: param.name.clone(),
                ty: param.ty.as_ref().map_or(IrType::Unknown, type_from_node),
            })
            .collect::<Vec<_>>();
        let return_type = function
            .return_type
            .as_ref()
            .map_or(IrType::Undefined, type_from_node);

        let mut lowerer = FunctionLowerer::new(&function.name, params, return_type, span);
        for statement in &function.body {
            lowerer.lower_statement(statement);
        }
        lowerer.finish()
    }
}

struct FunctionLowerer {
    name: String,
    params: Vec<IrParam>,
    return_type: IrType,
    blocks: Vec<IrBlock>,
    current_block: BlockId,
    next_value: usize,
    locals: HashMap<String, IrType>,
    source_span: Span,
}

impl FunctionLowerer {
    fn new(name: &str, params: Vec<IrParam>, return_type: IrType, source_span: Span) -> Self {
        let locals = params
            .iter()
            .map(|param| (param.name.clone(), param.ty.clone()))
            .collect();
        Self {
            name: name.into(),
            params,
            return_type,
            blocks: vec![IrBlock {
                id: BlockId(0),
                instructions: Vec::new(),
            }],
            current_block: BlockId(0),
            next_value: 0,
            locals,
            source_span,
        }
    }

    fn finish(self) -> IrFunction {
        IrFunction {
            name: self.name,
            params: self.params,
            return_type: self.return_type,
            blocks: self.blocks,
            source_span: self.source_span,
        }
    }

    fn lower_statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Variable(variable) => self.lower_variable(variable, statement.span),
            StatementKind::Return(value) => {
                let value = value
                    .as_ref()
                    .map(|expression| self.lower_expression(expression).0);
                self.emit(
                    IrInstructionKind::Return(value),
                    None,
                    IrType::Undefined,
                    statement.span,
                );
            }
            StatementKind::Expression(expression) | StatementKind::Throw(expression) => {
                self.lower_expression(expression);
            }
            StatementKind::If(if_stmt) => {
                let (condition, _) = self.lower_expression(&if_stmt.test);
                let then_block = self.create_block();
                let else_block = self.create_block();
                let merge_block = self.create_block();
                self.emit(
                    IrInstructionKind::Branch {
                        condition,
                        then_block,
                        else_block,
                    },
                    None,
                    IrType::Undefined,
                    if_stmt.test.span,
                );

                self.current_block = then_block;
                self.lower_statement(&if_stmt.consequent);
                self.emit(
                    IrInstructionKind::Jump(merge_block),
                    None,
                    IrType::Undefined,
                    statement.span,
                );

                self.current_block = else_block;
                if let Some(alternate) = &if_stmt.alternate {
                    self.lower_statement(alternate);
                }
                self.emit(
                    IrInstructionKind::Jump(merge_block),
                    None,
                    IrType::Undefined,
                    statement.span,
                );

                self.current_block = merge_block;
            }
            StatementKind::Block(body) => {
                for statement in body {
                    self.lower_statement(statement);
                }
            }
            StatementKind::Export(inner) => self.lower_statement(inner),
            StatementKind::Interface(_)
            | StatementKind::TypeAlias(_)
            | StatementKind::Class(_)
            | StatementKind::Function(_)
            | StatementKind::Import(_)
            | StatementKind::Empty
            | StatementKind::Error
            | StatementKind::While(_)
            | StatementKind::For(_) => {}
        }
    }

    fn lower_variable(&mut self, variable: &VariableDeclaration, span: Span) {
        let declared_type = variable.ty.as_ref().map_or(IrType::Unknown, type_from_node);
        let (value, ty) = if let Some(expression) = &variable.initializer {
            self.lower_expression(expression)
        } else {
            self.load_const(IrConst::Undefined, IrType::Undefined, span)
        };

        let local_type = if declared_type == IrType::Unknown {
            ty
        } else {
            declared_type
        };
        self.locals
            .insert(variable.name.clone(), local_type.clone());
        self.emit(
            IrInstructionKind::StoreLocal(variable.name.clone(), value),
            None,
            local_type,
            span,
        );
    }

    fn lower_expression(&mut self, expression: &Expression) -> (ValueId, IrType) {
        match &expression.kind {
            ExpressionKind::Identifier(name) => {
                let ty = self.locals.get(name).cloned().unwrap_or(IrType::Unknown);
                let value = self.allocate_value();
                self.emit(
                    IrInstructionKind::LoadLocal(name.clone()),
                    Some(value),
                    ty.clone(),
                    expression.span,
                );
                (value, ty)
            }
            ExpressionKind::Number(value) => self.load_const(
                IrConst::Number(value.clone()),
                IrType::Number,
                expression.span,
            ),
            ExpressionKind::String(value) => self.load_const(
                IrConst::String(value.clone()),
                IrType::String,
                expression.span,
            ),
            ExpressionKind::Boolean(value) => {
                self.load_const(IrConst::Boolean(*value), IrType::Boolean, expression.span)
            }
            ExpressionKind::Null => self.load_const(IrConst::Null, IrType::Null, expression.span),
            ExpressionKind::Undefined => {
                self.load_const(IrConst::Undefined, IrType::Undefined, expression.span)
            }
            ExpressionKind::Object(properties) => {
                let mut values = Vec::new();
                let mut fields = Vec::new();
                for property in properties {
                    let (value, ty) = self.lower_expression(&property.value);
                    values.push((property.name.clone(), value));
                    fields.push((property.name.clone(), ty));
                }
                self.load_const(
                    IrConst::Object(values),
                    IrType::Object(fields),
                    expression.span,
                )
            }
            ExpressionKind::Array(elements) => {
                let mut values = Vec::new();
                let mut element_type = IrType::Unknown;
                for element in elements {
                    let (value, ty) = self.lower_expression(element);
                    values.push(value);
                    if element_type == IrType::Unknown {
                        element_type = ty;
                    }
                }
                self.load_const(
                    IrConst::Array(values),
                    IrType::Array(Box::new(element_type)),
                    expression.span,
                )
            }
            ExpressionKind::Member { object, property } => {
                let (object_value, object_type) = self.lower_expression(object);
                let ty = member_type(&object_type, property);
                let value = self.allocate_value();
                self.emit(
                    IrInstructionKind::LoadMember {
                        object: object_value,
                        property: property.clone(),
                    },
                    Some(value),
                    ty.clone(),
                    expression.span,
                );
                (value, ty)
            }
            ExpressionKind::Call { callee, arguments } => {
                let mut args = Vec::new();
                for argument in arguments {
                    args.push(self.lower_expression(argument).0);
                }
                let callee_name = callee_name(callee);
                let value = self.allocate_value();
                self.emit(
                    IrInstructionKind::Call {
                        callee: callee_name,
                        args,
                    },
                    Some(value),
                    IrType::Unknown,
                    expression.span,
                );
                (value, IrType::Unknown)
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let (left_value, _) = self.lower_expression(left);
                let (right_value, _) = self.lower_expression(right);
                let op = binary_op(*operator);
                let ty = binary_type(*operator);
                let value = self.allocate_value();
                self.emit(
                    IrInstructionKind::Binary {
                        op,
                        left: left_value,
                        right: right_value,
                    },
                    Some(value),
                    ty.clone(),
                    expression.span,
                );
                (value, ty)
            }
            ExpressionKind::Assignment {
                left,
                operator,
                right,
            } => self.lower_assignment(left, *operator, right, expression.span),
            ExpressionKind::Unary { argument, .. } => self.lower_expression(argument),
            ExpressionKind::Error => {
                self.load_const(IrConst::Undefined, IrType::Unknown, expression.span)
            }
        }
    }

    fn lower_assignment(
        &mut self,
        left: &Expression,
        operator: AssignmentOperator,
        right: &Expression,
        span: Span,
    ) -> (ValueId, IrType) {
        match &left.kind {
            ExpressionKind::Identifier(name) => {
                let (right_value, right_type) = self.lower_expression(right);
                self.emit(
                    IrInstructionKind::StoreLocal(name.clone(), right_value),
                    None,
                    right_type.clone(),
                    span,
                );
                (right_value, right_type)
            }
            ExpressionKind::Member { object, property } => {
                let (object_value, _) = self.lower_expression(object);
                let (right_value, right_type) = self.lower_expression(right);
                let stored_value = if operator == AssignmentOperator::Assign {
                    right_value
                } else {
                    let current_value = self.allocate_value();
                    self.emit(
                        IrInstructionKind::LoadMember {
                            object: object_value,
                            property: property.clone(),
                        },
                        Some(current_value),
                        right_type.clone(),
                        left.span,
                    );
                    let result = self.allocate_value();
                    self.emit(
                        IrInstructionKind::Binary {
                            op: assignment_binary_op(operator),
                            left: current_value,
                            right: right_value,
                        },
                        Some(result),
                        right_type.clone(),
                        span,
                    );
                    result
                };
                self.emit(
                    IrInstructionKind::StoreMember {
                        object: object_value,
                        property: property.clone(),
                        value: stored_value,
                    },
                    None,
                    right_type.clone(),
                    span,
                );
                (stored_value, right_type)
            }
            _ => self.lower_expression(right),
        }
    }

    fn load_const(&mut self, constant: IrConst, ty: IrType, span: Span) -> (ValueId, IrType) {
        let value = self.allocate_value();
        self.emit(
            IrInstructionKind::LoadConst(constant),
            Some(value),
            ty.clone(),
            span,
        );
        (value, ty)
    }

    fn create_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(IrBlock {
            id,
            instructions: Vec::new(),
        });
        id
    }

    fn allocate_value(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    fn emit(
        &mut self,
        kind: IrInstructionKind,
        result: Option<ValueId>,
        ty: IrType,
        source_span: Span,
    ) -> Option<ValueId> {
        self.blocks[self.current_block.0]
            .instructions
            .push(IrInstruction {
                kind,
                result,
                ty,
                source_span,
            });
        result
    }
}

fn type_from_node(ty: &TypeNode) -> IrType {
    match &ty.kind {
        TypeKind::Number => IrType::Number,
        TypeKind::String => IrType::String,
        TypeKind::Boolean => IrType::Boolean,
        TypeKind::Null => IrType::Null,
        TypeKind::Undefined => IrType::Undefined,
        TypeKind::Named(name) => IrType::Named(name.clone()),
        TypeKind::Array(inner) => IrType::Array(Box::new(type_from_node(inner))),
        TypeKind::Object(members) => IrType::Object(
            members
                .iter()
                .map(|member| (member.name.clone(), type_from_node(&member.ty)))
                .collect(),
        ),
    }
}

fn member_type(object_type: &IrType, property: &str) -> IrType {
    match object_type {
        IrType::Object(fields) => fields
            .iter()
            .find_map(|(name, ty)| (name == property).then(|| ty.clone()))
            .unwrap_or(IrType::Unknown),
        _ => IrType::Unknown,
    }
}

fn callee_name(expression: &Expression) -> String {
    match &expression.kind {
        ExpressionKind::Identifier(name) => name.clone(),
        ExpressionKind::Member { object, property } => {
            format!("{}.{}", callee_name(object), property)
        }
        _ => "<expr>".into(),
    }
}

fn binary_op(operator: BinaryOperator) -> IrBinaryOp {
    match operator {
        BinaryOperator::Add => IrBinaryOp::Add,
        BinaryOperator::Subtract => IrBinaryOp::Subtract,
        BinaryOperator::Multiply => IrBinaryOp::Multiply,
        BinaryOperator::Divide => IrBinaryOp::Divide,
        BinaryOperator::Remainder => IrBinaryOp::Remainder,
        BinaryOperator::Equal => IrBinaryOp::Equal,
        BinaryOperator::StrictEqual => IrBinaryOp::StrictEqual,
        BinaryOperator::NotEqual => IrBinaryOp::NotEqual,
        BinaryOperator::StrictNotEqual => IrBinaryOp::StrictNotEqual,
        BinaryOperator::Less => IrBinaryOp::Less,
        BinaryOperator::LessEqual => IrBinaryOp::LessEqual,
        BinaryOperator::Greater => IrBinaryOp::Greater,
        BinaryOperator::GreaterEqual => IrBinaryOp::GreaterEqual,
        BinaryOperator::LogicalAnd => IrBinaryOp::LogicalAnd,
        BinaryOperator::LogicalOr => IrBinaryOp::LogicalOr,
        BinaryOperator::NullishCoalesce => IrBinaryOp::NullishCoalesce,
    }
}

fn binary_type(operator: BinaryOperator) -> IrType {
    match operator {
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => IrType::Number,
        BinaryOperator::Equal
        | BinaryOperator::StrictEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::StrictNotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual
        | BinaryOperator::LogicalAnd
        | BinaryOperator::LogicalOr => IrType::Boolean,
        BinaryOperator::NullishCoalesce => IrType::Unknown,
    }
}

fn assignment_binary_op(operator: AssignmentOperator) -> IrBinaryOp {
    match operator {
        AssignmentOperator::Assign => IrBinaryOp::Add,
        AssignmentOperator::AddAssign => IrBinaryOp::Add,
        AssignmentOperator::SubtractAssign => IrBinaryOp::Subtract,
        AssignmentOperator::MultiplyAssign => IrBinaryOp::Multiply,
        AssignmentOperator::DivideAssign => IrBinaryOp::Divide,
        AssignmentOperator::RemainderAssign => IrBinaryOp::Remainder,
    }
}

fn empty_span() -> Span {
    let position = tsvm_ast::Position {
        byte: 0,
        line: 1,
        column: 1,
    };
    Span {
        start: position,
        end: position,
    }
}
