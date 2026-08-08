#![forbid(unsafe_code)]

use std::collections::HashMap;

use tsvm_ast::{
    AssignmentOperator, BinaryOperator, Expression, ExpressionKind, FunctionDeclaration, Program,
    Span, Statement, StatementKind, TypeKind, TypeNode, UnaryOperator, VariableDeclaration,
};
use tsvm_parser::parse_source;

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticOutput {
    pub symbols: SymbolTable,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolTable {
    pub values: Vec<Symbol>,
    pub types: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Function,
    Interface,
    TypeAlias,
    Class,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    UnknownIdentifier,
    UnknownType,
    DuplicateSymbol,
    TypeMismatch,
    MissingProperty,
    WrongArgumentCount,
    InvalidCallTarget,
    InvalidMemberAccess,
    ReturnTypeMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Type {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
    Object(HashMap<String, Type>),
    Array(Box<Type>),
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
        variadic: bool,
    },
    Unknown,
}

#[derive(Debug, Clone)]
struct ValueBinding {
    ty: Type,
}

#[derive(Debug, Clone)]
struct FunctionBinding {
    declaration: FunctionDeclaration,
    statement_span: Span,
}

#[derive(Default)]
struct Analyzer {
    symbols: SymbolTable,
    type_bindings: HashMap<String, Type>,
    value_scopes: Vec<HashMap<String, ValueBinding>>,
    functions: Vec<FunctionBinding>,
    diagnostics: Vec<SemanticDiagnostic>,
    current_return_type: Option<Type>,
}

pub fn analyze_source(source: &str) -> SemanticOutput {
    let parsed = parse_source(source);
    let mut analyzer = Analyzer::default();

    for diagnostic in parsed.diagnostics {
        analyzer.diagnostics.push(SemanticDiagnostic {
            code: DiagnosticCode::TypeMismatch,
            message: diagnostic.message,
            span: diagnostic.span,
        });
    }

    analyzer.analyze_program(&parsed.program)
}

impl Analyzer {
    fn analyze_program(mut self, program: &Program) -> SemanticOutput {
        self.push_scope();
        self.install_browser_builtins();
        self.collect_top_level_symbols(program);

        for function in self.functions.clone() {
            self.analyze_function_body(&function);
        }

        for statement in &program.body {
            if !matches!(statement.kind, StatementKind::Function(_)) {
                self.analyze_statement(statement);
            }
        }

        SemanticOutput {
            symbols: self.symbols,
            diagnostics: self.diagnostics,
        }
    }

    fn install_browser_builtins(&mut self) {
        let mut console_shape = HashMap::new();
        console_shape.insert(
            "log".into(),
            Type::Function {
                params: Vec::new(),
                return_type: Box::new(Type::Undefined),
                variadic: true,
            },
        );
        self.define_value(
            "console",
            Type::Object(console_shape),
            SymbolKind::Import,
            None,
        );
    }

    fn collect_top_level_symbols(&mut self, program: &Program) {
        for statement in &program.body {
            self.collect_statement_symbol(statement, true);
        }
    }

    fn collect_statement_symbol(&mut self, statement: &Statement, top_level: bool) {
        match &statement.kind {
            StatementKind::Interface(interface) => {
                let ty = Type::Object(
                    interface
                        .members
                        .iter()
                        .map(|member| (member.name.clone(), self.resolve_type_node(&member.ty)))
                        .collect(),
                );
                self.define_type(&interface.name, ty, SymbolKind::Interface, statement.span);
            }
            StatementKind::TypeAlias(alias) => {
                let ty = self.resolve_type_node(&alias.ty);
                self.define_type(&alias.name, ty, SymbolKind::TypeAlias, statement.span);
            }
            StatementKind::Class(class) => {
                let ty = Type::Object(
                    class
                        .members
                        .iter()
                        .map(|member| {
                            (
                                member.name.clone(),
                                member
                                    .ty
                                    .as_ref()
                                    .map_or(Type::Unknown, |ty| self.resolve_type_node(ty)),
                            )
                        })
                        .collect(),
                );
                self.define_type(&class.name, ty, SymbolKind::Class, statement.span);
            }
            StatementKind::Function(function) => {
                let ty = Type::Function {
                    params: function
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .ty
                                .as_ref()
                                .map_or(Type::Unknown, |ty| self.resolve_type_node(ty))
                        })
                        .collect(),
                    return_type: Box::new(
                        function
                            .return_type
                            .as_ref()
                            .map_or(Type::Undefined, |ty| self.resolve_type_node(ty)),
                    ),
                    variadic: false,
                };
                self.define_value(
                    &function.name,
                    ty,
                    SymbolKind::Function,
                    Some(statement.span),
                );
                if top_level {
                    self.functions.push(FunctionBinding {
                        declaration: function.clone(),
                        statement_span: statement.span,
                    });
                }
            }
            StatementKind::Export(inner) => self.collect_statement_symbol(inner, top_level),
            _ => {}
        }
    }

    fn analyze_function_body(&mut self, binding: &FunctionBinding) {
        let function = &binding.declaration;
        let return_type = function
            .return_type
            .as_ref()
            .map_or(Type::Undefined, |ty| self.resolve_type_node(ty));
        let previous_return = self.current_return_type.replace(return_type);

        self.push_scope();
        for param in &function.params {
            let ty = param
                .ty
                .as_ref()
                .map_or(Type::Unknown, |ty| self.resolve_type_node(ty));
            self.define_value(&param.name, ty, SymbolKind::Variable, Some(param.span));
        }

        for statement in &function.body {
            self.analyze_statement(statement);
        }

        self.pop_scope();
        self.current_return_type = previous_return;

        if function.body.is_empty() {
            self.diagnostic(
                DiagnosticCode::ReturnTypeMismatch,
                "function body is empty",
                binding.statement_span,
            );
        }
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Variable(variable) => self.analyze_variable(variable, statement.span),
            StatementKind::Return(value) => self.analyze_return(value.as_ref(), statement.span),
            StatementKind::Throw(value) | StatementKind::Expression(value) => {
                self.infer_expression(value);
            }
            StatementKind::If(if_stmt) => {
                self.infer_expression(&if_stmt.test);
                self.analyze_statement(&if_stmt.consequent);
                if let Some(alternate) = &if_stmt.alternate {
                    self.analyze_statement(alternate);
                }
            }
            StatementKind::While(while_stmt) => {
                self.infer_expression(&while_stmt.test);
                self.analyze_statement(&while_stmt.body);
            }
            StatementKind::For(for_stmt) => {
                self.push_scope();
                if let Some(initializer) = &for_stmt.initializer {
                    self.analyze_statement(initializer);
                }
                if let Some(test) = &for_stmt.test {
                    self.infer_expression(test);
                }
                if let Some(update) = &for_stmt.update {
                    self.infer_expression(update);
                }
                self.analyze_statement(&for_stmt.body);
                self.pop_scope();
            }
            StatementKind::Block(body) => {
                self.push_scope();
                for statement in body {
                    self.analyze_statement(statement);
                }
                self.pop_scope();
            }
            StatementKind::Export(inner) => self.analyze_statement(inner),
            StatementKind::Interface(_)
            | StatementKind::TypeAlias(_)
            | StatementKind::Class(_)
            | StatementKind::Function(_)
            | StatementKind::Import(_)
            | StatementKind::Empty
            | StatementKind::Error => {}
        }
    }

    fn analyze_variable(&mut self, variable: &VariableDeclaration, span: Span) {
        let declared = variable.ty.as_ref().map(|ty| self.resolve_type_node(ty));
        let initializer = variable
            .initializer
            .as_ref()
            .map(|expression| self.infer_expression(expression));

        if let (Some(expected), Some(actual)) = (&declared, &initializer) {
            self.require_assignable(expected, actual, span, DiagnosticCode::TypeMismatch);
        }

        let ty = declared.or(initializer).unwrap_or(Type::Unknown);
        self.define_value(&variable.name, ty, SymbolKind::Variable, Some(span));
    }

    fn analyze_return(&mut self, value: Option<&Expression>, span: Span) {
        let actual = value
            .map(|expression| self.infer_expression(expression))
            .unwrap_or(Type::Undefined);
        let Some(expected) = self.current_return_type.clone() else {
            return;
        };

        self.require_assignable(&expected, &actual, span, DiagnosticCode::ReturnTypeMismatch);
    }

    fn infer_expression(&mut self, expression: &Expression) -> Type {
        match &expression.kind {
            ExpressionKind::Identifier(name) => self.lookup_value(name).map_or_else(
                || {
                    self.diagnostic(
                        DiagnosticCode::UnknownIdentifier,
                        &format!("unknown identifier `{name}`"),
                        expression.span,
                    );
                    Type::Unknown
                },
                |binding| binding.ty,
            ),
            ExpressionKind::Number(_) => Type::Number,
            ExpressionKind::String(_) => Type::String,
            ExpressionKind::Boolean(_) => Type::Boolean,
            ExpressionKind::Null => Type::Null,
            ExpressionKind::Undefined => Type::Undefined,
            ExpressionKind::Object(properties) => Type::Object(
                properties
                    .iter()
                    .map(|property| {
                        (
                            property.name.clone(),
                            self.infer_expression(&property.value),
                        )
                    })
                    .collect(),
            ),
            ExpressionKind::Array(elements) => {
                let mut element_type = Type::Unknown;
                for element in elements {
                    let actual = self.infer_expression(element);
                    if element_type == Type::Unknown {
                        element_type = actual;
                    } else {
                        self.require_assignable(
                            &element_type,
                            &actual,
                            element.span,
                            DiagnosticCode::TypeMismatch,
                        );
                    }
                }
                Type::Array(Box::new(element_type))
            }
            ExpressionKind::Member { object, property } => {
                let object_type = self.infer_expression(object);
                self.member_type(&object_type, property, expression.span)
            }
            ExpressionKind::Call { callee, arguments } => {
                let callee_type = self.infer_expression(callee);
                self.infer_call(&callee_type, arguments, expression.span)
            }
            ExpressionKind::Unary { operator, argument } => {
                let actual = self.infer_expression(argument);
                match operator {
                    UnaryOperator::Negate => {
                        self.require_assignable(
                            &Type::Number,
                            &actual,
                            argument.span,
                            DiagnosticCode::TypeMismatch,
                        );
                        Type::Number
                    }
                    UnaryOperator::Not => Type::Boolean,
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => self.infer_binary(left, *operator, right, expression.span),
            ExpressionKind::Assignment {
                left,
                operator,
                right,
            } => self.infer_assignment(left, *operator, right, expression.span),
            ExpressionKind::Error => Type::Unknown,
        }
    }

    fn infer_binary(
        &mut self,
        left: &Expression,
        operator: BinaryOperator,
        right: &Expression,
        span: Span,
    ) -> Type {
        let left_type = self.infer_expression(left);
        let right_type = self.infer_expression(right);

        match operator {
            BinaryOperator::Add => {
                if left_type == Type::String || right_type == Type::String {
                    Type::String
                } else {
                    self.require_assignable(
                        &Type::Number,
                        &left_type,
                        left.span,
                        DiagnosticCode::TypeMismatch,
                    );
                    self.require_assignable(
                        &Type::Number,
                        &right_type,
                        right.span,
                        DiagnosticCode::TypeMismatch,
                    );
                    Type::Number
                }
            }
            BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Remainder => {
                self.require_assignable(
                    &Type::Number,
                    &left_type,
                    left.span,
                    DiagnosticCode::TypeMismatch,
                );
                self.require_assignable(
                    &Type::Number,
                    &right_type,
                    right.span,
                    DiagnosticCode::TypeMismatch,
                );
                Type::Number
            }
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
                self.require_assignable(
                    &Type::Boolean,
                    &left_type,
                    left.span,
                    DiagnosticCode::TypeMismatch,
                );
                self.require_assignable(
                    &Type::Boolean,
                    &right_type,
                    right.span,
                    DiagnosticCode::TypeMismatch,
                );
                Type::Boolean
            }
            BinaryOperator::NullishCoalesce => {
                if left_type == Type::Null || left_type == Type::Undefined {
                    right_type
                } else {
                    left_type
                }
            }
            BinaryOperator::Equal
            | BinaryOperator::StrictEqual
            | BinaryOperator::NotEqual
            | BinaryOperator::StrictNotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual => {
                let _ = span;
                Type::Boolean
            }
        }
    }

    fn infer_assignment(
        &mut self,
        left: &Expression,
        operator: AssignmentOperator,
        right: &Expression,
        span: Span,
    ) -> Type {
        let left_type = self.infer_expression(left);
        let right_type = self.infer_expression(right);

        match operator {
            AssignmentOperator::Assign => {
                self.require_assignable(
                    &left_type,
                    &right_type,
                    span,
                    DiagnosticCode::TypeMismatch,
                );
                left_type
            }
            AssignmentOperator::AddAssign
            | AssignmentOperator::SubtractAssign
            | AssignmentOperator::MultiplyAssign
            | AssignmentOperator::DivideAssign
            | AssignmentOperator::RemainderAssign => {
                self.require_assignable(
                    &Type::Number,
                    &left_type,
                    left.span,
                    DiagnosticCode::TypeMismatch,
                );
                self.require_assignable(
                    &Type::Number,
                    &right_type,
                    right.span,
                    DiagnosticCode::TypeMismatch,
                );
                Type::Number
            }
        }
    }

    fn infer_call(&mut self, callee_type: &Type, arguments: &[Expression], span: Span) -> Type {
        let Type::Function {
            params,
            return_type,
            variadic,
        } = callee_type
        else {
            if callee_type != &Type::Unknown {
                self.diagnostic(
                    DiagnosticCode::InvalidCallTarget,
                    "value is not callable",
                    span,
                );
            }
            return Type::Unknown;
        };

        if !variadic && params.len() != arguments.len() {
            self.diagnostic(
                DiagnosticCode::WrongArgumentCount,
                &format!(
                    "expected {} arguments, found {}",
                    params.len(),
                    arguments.len()
                ),
                span,
            );
        }

        for (expected, argument) in params.iter().zip(arguments) {
            let actual = self.infer_expression(argument);
            self.require_assignable(
                expected,
                &actual,
                argument.span,
                DiagnosticCode::TypeMismatch,
            );
        }

        return_type.as_ref().clone()
    }

    fn member_type(&mut self, object_type: &Type, property: &str, span: Span) -> Type {
        match object_type {
            Type::Object(fields) => fields.get(property).cloned().unwrap_or_else(|| {
                self.diagnostic(
                    DiagnosticCode::InvalidMemberAccess,
                    &format!("property `{property}` does not exist"),
                    span,
                );
                Type::Unknown
            }),
            Type::Unknown => Type::Unknown,
            _ => {
                self.diagnostic(
                    DiagnosticCode::InvalidMemberAccess,
                    "member access requires an object",
                    span,
                );
                Type::Unknown
            }
        }
    }

    fn resolve_type_node(&mut self, ty: &TypeNode) -> Type {
        match &ty.kind {
            TypeKind::Number => Type::Number,
            TypeKind::String => Type::String,
            TypeKind::Boolean => Type::Boolean,
            TypeKind::Null => Type::Null,
            TypeKind::Undefined => Type::Undefined,
            TypeKind::Named(name) => self.type_bindings.get(name).cloned().unwrap_or_else(|| {
                self.diagnostic(
                    DiagnosticCode::UnknownType,
                    &format!("unknown type `{name}`"),
                    ty.span,
                );
                Type::Unknown
            }),
            TypeKind::Array(inner) => Type::Array(Box::new(self.resolve_type_node(inner))),
            TypeKind::Object(members) => Type::Object(
                members
                    .iter()
                    .map(|member| (member.name.clone(), self.resolve_type_node(&member.ty)))
                    .collect(),
            ),
        }
    }

    fn require_assignable(
        &mut self,
        expected: &Type,
        actual: &Type,
        span: Span,
        code: DiagnosticCode,
    ) {
        if expected == &Type::Unknown || actual == &Type::Unknown {
            return;
        }

        match (expected, actual) {
            (Type::Number, Type::Number)
            | (Type::String, Type::String)
            | (Type::Boolean, Type::Boolean)
            | (Type::Null, Type::Null)
            | (Type::Undefined, Type::Undefined) => {}
            (Type::Array(expected), Type::Array(actual)) => {
                self.require_assignable(expected, actual, span, code);
            }
            (Type::Object(expected_fields), Type::Object(actual_fields)) => {
                for (name, expected_field_type) in expected_fields {
                    let Some(actual_field_type) = actual_fields.get(name) else {
                        self.diagnostic(
                            DiagnosticCode::MissingProperty,
                            &format!("missing property `{name}`"),
                            span,
                        );
                        continue;
                    };
                    self.require_assignable(expected_field_type, actual_field_type, span, code);
                }
            }
            _ => self.diagnostic(code, "type mismatch", span),
        }
    }

    fn define_type(&mut self, name: &str, ty: Type, kind: SymbolKind, span: Span) {
        if self.type_bindings.contains_key(name) {
            self.diagnostic(
                DiagnosticCode::DuplicateSymbol,
                &format!("duplicate type `{name}`"),
                span,
            );
            return;
        }
        self.type_bindings.insert(name.into(), ty);
        self.symbols.types.push(Symbol {
            name: name.into(),
            kind,
        });
    }

    fn define_value(&mut self, name: &str, ty: Type, kind: SymbolKind, span: Option<Span>) {
        if self
            .value_scopes
            .last()
            .is_some_and(|scope| scope.contains_key(name))
        {
            if let Some(span) = span {
                self.diagnostic(
                    DiagnosticCode::DuplicateSymbol,
                    &format!("duplicate value `{name}`"),
                    span,
                );
            }
            return;
        }

        let Some(scope) = self.value_scopes.last_mut() else {
            return;
        };
        scope.insert(name.into(), ValueBinding { ty });
        if self.value_scopes.len() == 1 {
            self.symbols.values.push(Symbol {
                name: name.into(),
                kind,
            });
        }
    }

    fn lookup_value(&self, name: &str) -> Option<ValueBinding> {
        self.value_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn push_scope(&mut self) {
        self.value_scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.value_scopes.pop();
    }

    fn diagnostic(&mut self, code: DiagnosticCode, message: &str, span: Span) {
        self.diagnostics.push(SemanticDiagnostic {
            code,
            message: message.into(),
            span,
        });
    }
}
