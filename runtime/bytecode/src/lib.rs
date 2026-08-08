#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use tsvm_ast::Span;
use tsvm_ir::{
    lower_source, IrBinaryOp, IrConst, IrFunction, IrInstructionKind, IrProgram, IrType,
};
use tsvm_semantic::SemanticDiagnostic;

pub const BYTECODE_MAGIC: [u8; 4] = *b"TSVM";
pub const BYTECODE_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeOutput {
    pub module: Option<BytecodeModule>,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeModule {
    pub header: BytecodeHeader,
    pub constants: Vec<Constant>,
    pub functions: Vec<BytecodeFunction>,
    pub source_map: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeHeader {
    pub magic: [u8; 4],
    pub version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Constant {
    Number(String),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeFunction {
    pub name: String,
    pub params: Vec<TypeTag>,
    pub return_type: TypeTag,
    pub blocks: Vec<BytecodeBlock>,
    pub exception_table: Vec<ExceptionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeBlock {
    pub id: u32,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operands: Vec<u32>,
    pub type_tag: TypeTag,
    pub source_ref: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Opcode {
    LoadConst,
    BuildObject,
    BuildArray,
    LoadLocal,
    StoreLocal,
    LoadMember,
    StoreMember,
    Binary,
    Call,
    Branch,
    Jump,
    Return,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypeTag {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
    Object,
    Array,
    Named,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceRef {
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionEntry {
    pub start_block: u32,
    pub end_block: u32,
    pub handler_block: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyError {
    pub code: VerifyErrorCode,
    pub message: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VerifyErrorCode {
    InvalidHeader,
    EmptyFunctionTable,
    InvalidConstantReference,
    InvalidLocalReference,
    InvalidValueReference,
    InvalidJumpTarget,
    InvalidSourceReference,
    InvalidExceptionEntry,
    MissingTerminator,
    InvalidTypeState,
}

pub fn compile_source(source: &str) -> BytecodeOutput {
    let lowered = lower_source(source);
    let Some(ir) = lowered.ir else {
        return BytecodeOutput {
            module: None,
            diagnostics: lowered.diagnostics,
        };
    };

    BytecodeOutput {
        module: Some(Compiler::default().compile(&ir)),
        diagnostics: Vec::new(),
    }
}

pub fn encode_module(module: &BytecodeModule) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&module.header.magic);
    write_u16(&mut out, module.header.version);
    write_u32(&mut out, module.constants.len() as u32);
    write_u32(&mut out, module.source_map.len() as u32);
    write_u32(&mut out, module.functions.len() as u32);

    for constant in &module.constants {
        write_constant(&mut out, constant);
    }
    for source in &module.source_map {
        write_u32(&mut out, source.start_byte);
        write_u32(&mut out, source.end_byte);
        write_u32(&mut out, source.start_line);
        write_u32(&mut out, source.start_column);
        write_u32(&mut out, source.end_line);
        write_u32(&mut out, source.end_column);
    }
    for function in &module.functions {
        write_string(&mut out, &function.name);
        write_u32(&mut out, function.params.len() as u32);
        for param in &function.params {
            out.push(type_tag_to_u8(*param));
        }
        out.push(type_tag_to_u8(function.return_type));
        write_u32(&mut out, function.blocks.len() as u32);
        for block in &function.blocks {
            write_u32(&mut out, block.id);
            write_u32(&mut out, block.instructions.len() as u32);
            for instruction in &block.instructions {
                out.push(opcode_to_u8(instruction.opcode));
                out.push(type_tag_to_u8(instruction.type_tag));
                write_u32(&mut out, instruction.source_ref);
                write_u32(&mut out, instruction.operands.len() as u32);
                for operand in &instruction.operands {
                    write_u32(&mut out, *operand);
                }
            }
        }
        write_u32(&mut out, function.exception_table.len() as u32);
        for entry in &function.exception_table {
            write_u32(&mut out, entry.start_block);
            write_u32(&mut out, entry.end_block);
            write_u32(&mut out, entry.handler_block);
        }
    }

    out
}

pub fn decode_module(bytes: &[u8]) -> Result<BytecodeModule, DecodeError> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_magic()?;
    if magic != BYTECODE_MAGIC {
        return Err(DecodeError {
            message: "invalid bytecode magic".into(),
        });
    }
    let version = reader.read_u16()?;
    if version != BYTECODE_VERSION {
        return Err(DecodeError {
            message: "unsupported bytecode version".into(),
        });
    }
    let constant_count = reader.read_u32()? as usize;
    let source_count = reader.read_u32()? as usize;
    let function_count = reader.read_u32()? as usize;

    let mut constants = Vec::with_capacity(constant_count);
    for _ in 0..constant_count {
        constants.push(reader.read_constant()?);
    }

    let mut source_map = Vec::with_capacity(source_count);
    for _ in 0..source_count {
        source_map.push(SourceRef {
            start_byte: reader.read_u32()?,
            end_byte: reader.read_u32()?,
            start_line: reader.read_u32()?,
            start_column: reader.read_u32()?,
            end_line: reader.read_u32()?,
            end_column: reader.read_u32()?,
        });
    }

    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        let name = reader.read_string()?;
        let param_count = reader.read_u32()? as usize;
        let mut params = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            params.push(reader.read_type_tag()?);
        }
        let return_type = reader.read_type_tag()?;
        let block_count = reader.read_u32()? as usize;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let id = reader.read_u32()?;
            let instruction_count = reader.read_u32()? as usize;
            let mut instructions = Vec::with_capacity(instruction_count);
            for _ in 0..instruction_count {
                let opcode = reader.read_opcode()?;
                let type_tag = reader.read_type_tag()?;
                let source_ref = reader.read_u32()?;
                let operand_count = reader.read_u32()? as usize;
                let mut operands = Vec::with_capacity(operand_count);
                for _ in 0..operand_count {
                    operands.push(reader.read_u32()?);
                }
                instructions.push(Instruction {
                    opcode,
                    operands,
                    type_tag,
                    source_ref,
                });
            }
            blocks.push(BytecodeBlock { id, instructions });
        }
        let exception_count = reader.read_u32()? as usize;
        let mut exception_table = Vec::with_capacity(exception_count);
        for _ in 0..exception_count {
            exception_table.push(ExceptionEntry {
                start_block: reader.read_u32()?,
                end_block: reader.read_u32()?,
                handler_block: reader.read_u32()?,
            });
        }
        functions.push(BytecodeFunction {
            name,
            params,
            return_type,
            blocks,
            exception_table,
        });
    }

    reader.finish()?;

    Ok(BytecodeModule {
        header: BytecodeHeader { magic, version },
        constants,
        functions,
        source_map,
    })
}

pub fn verify_module(module: &BytecodeModule) -> Result<(), Vec<VerifyError>> {
    let mut verifier = Verifier {
        module,
        errors: Vec::new(),
    };
    verifier.verify();
    if verifier.errors.is_empty() {
        Ok(())
    } else {
        Err(verifier.errors)
    }
}

pub fn source_ref_from_span(span: Span) -> SourceRef {
    SourceRef {
        start_byte: span.start.byte as u32,
        end_byte: span.end.byte as u32,
        start_line: span.start.line as u32,
        start_column: span.start.column as u32,
        end_line: span.end.line as u32,
        end_column: span.end.column as u32,
    }
}

#[derive(Default)]
struct Compiler {
    constants: Vec<Constant>,
    constant_ids: HashMap<Constant, u32>,
    source_map: Vec<SourceRef>,
    source_ids: HashMap<SourceRef, u32>,
}

impl Compiler {
    fn compile(mut self, ir: &IrProgram) -> BytecodeModule {
        let mut functions = Vec::new();
        for function in &ir.functions {
            functions.push(self.compile_function(function));
        }
        functions.push(self.compile_function(&ir.entry));

        BytecodeModule {
            header: BytecodeHeader {
                magic: BYTECODE_MAGIC,
                version: BYTECODE_VERSION,
            },
            constants: self.constants,
            functions,
            source_map: self.source_map,
        }
    }

    fn compile_function(&mut self, function: &IrFunction) -> BytecodeFunction {
        let mut local_ids = HashMap::new();
        for (index, param) in function.params.iter().enumerate() {
            local_ids.insert(param.name.clone(), index as u32);
        }
        let mut next_local = function.params.len() as u32;

        let blocks = function
            .blocks
            .iter()
            .map(|block| {
                let instructions = block
                    .instructions
                    .iter()
                    .map(|instruction| {
                        self.compile_instruction(instruction, &mut local_ids, &mut next_local)
                    })
                    .collect();
                BytecodeBlock {
                    id: block.id.0 as u32,
                    instructions,
                }
            })
            .collect();

        BytecodeFunction {
            name: function.name.clone(),
            params: function
                .params
                .iter()
                .map(|param| type_tag(&param.ty))
                .collect(),
            return_type: type_tag(&function.return_type),
            blocks,
            exception_table: Vec::new(),
        }
    }

    fn compile_instruction(
        &mut self,
        instruction: &tsvm_ir::IrInstruction,
        local_ids: &mut HashMap<String, u32>,
        next_local: &mut u32,
    ) -> Instruction {
        let source_ref = self.intern_source(source_ref_from_span(instruction.source_span));
        let type_tag = type_tag(&instruction.ty);
        match &instruction.kind {
            IrInstructionKind::LoadConst(constant) => Instruction {
                opcode: match constant {
                    IrConst::Object(_) => Opcode::BuildObject,
                    IrConst::Array(_) => Opcode::BuildArray,
                    _ => Opcode::LoadConst,
                },
                operands: self.ir_const_operands(constant),
                type_tag,
                source_ref,
            },
            IrInstructionKind::LoadLocal(name) => Instruction {
                opcode: Opcode::LoadLocal,
                operands: vec![local_id(name, local_ids, next_local)],
                type_tag,
                source_ref,
            },
            IrInstructionKind::StoreLocal(name, value) => Instruction {
                opcode: Opcode::StoreLocal,
                operands: vec![local_id(name, local_ids, next_local), value.0 as u32],
                type_tag,
                source_ref,
            },
            IrInstructionKind::LoadMember { object, property } => Instruction {
                opcode: Opcode::LoadMember,
                operands: vec![
                    object.0 as u32,
                    self.intern_constant(Constant::Symbol(property.clone())),
                ],
                type_tag,
                source_ref,
            },
            IrInstructionKind::StoreMember {
                object,
                property,
                value,
            } => Instruction {
                opcode: Opcode::StoreMember,
                operands: vec![
                    object.0 as u32,
                    self.intern_constant(Constant::Symbol(property.clone())),
                    value.0 as u32,
                ],
                type_tag,
                source_ref,
            },
            IrInstructionKind::Binary { op, left, right } => Instruction {
                opcode: Opcode::Binary,
                operands: vec![binary_op_code(*op), left.0 as u32, right.0 as u32],
                type_tag,
                source_ref,
            },
            IrInstructionKind::Call { callee, args } => {
                let mut operands = vec![
                    self.intern_constant(Constant::Symbol(callee.clone())),
                    args.len() as u32,
                ];
                operands.extend(args.iter().map(|arg| arg.0 as u32));
                Instruction {
                    opcode: Opcode::Call,
                    operands,
                    type_tag,
                    source_ref,
                }
            }
            IrInstructionKind::Branch {
                condition,
                then_block,
                else_block,
            } => Instruction {
                opcode: Opcode::Branch,
                operands: vec![condition.0 as u32, then_block.0 as u32, else_block.0 as u32],
                type_tag,
                source_ref,
            },
            IrInstructionKind::Jump(block) => Instruction {
                opcode: Opcode::Jump,
                operands: vec![block.0 as u32],
                type_tag,
                source_ref,
            },
            IrInstructionKind::Return(value) => Instruction {
                opcode: Opcode::Return,
                operands: value.map_or_else(Vec::new, |value| vec![value.0 as u32]),
                type_tag,
                source_ref,
            },
        }
    }

    fn ir_const_operands(&mut self, constant: &IrConst) -> Vec<u32> {
        match constant {
            IrConst::Number(value) => vec![self.intern_constant(Constant::Number(value.clone()))],
            IrConst::String(value) => vec![self.intern_constant(Constant::String(value.clone()))],
            IrConst::Boolean(value) => vec![self.intern_constant(Constant::Boolean(*value))],
            IrConst::Null => vec![self.intern_constant(Constant::Null)],
            IrConst::Undefined => vec![self.intern_constant(Constant::Undefined)],
            IrConst::Object(properties) => {
                let mut operands = vec![properties.len() as u32];
                for (name, value) in properties {
                    operands.push(self.intern_constant(Constant::Symbol(name.clone())));
                    operands.push(value.0 as u32);
                }
                operands
            }
            IrConst::Array(elements) => {
                let mut operands = vec![elements.len() as u32];
                operands.extend(elements.iter().map(|value| value.0 as u32));
                operands
            }
        }
    }

    fn intern_constant(&mut self, constant: Constant) -> u32 {
        if let Some(index) = self.constant_ids.get(&constant) {
            return *index;
        }
        let index = self.constants.len() as u32;
        self.constants.push(constant.clone());
        self.constant_ids.insert(constant, index);
        index
    }

    fn intern_source(&mut self, source: SourceRef) -> u32 {
        if let Some(index) = self.source_ids.get(&source) {
            return *index;
        }
        let index = self.source_map.len() as u32;
        self.source_map.push(source.clone());
        self.source_ids.insert(source, index);
        index
    }
}

struct Verifier<'module> {
    module: &'module BytecodeModule,
    errors: Vec<VerifyError>,
}

impl Verifier<'_> {
    fn verify(&mut self) {
        if self.module.header.magic != BYTECODE_MAGIC
            || self.module.header.version != BYTECODE_VERSION
        {
            self.error(VerifyErrorCode::InvalidHeader, "invalid bytecode header");
        }
        if self.module.functions.is_empty() {
            self.error(
                VerifyErrorCode::EmptyFunctionTable,
                "function table is empty",
            );
        }

        for function in &self.module.functions {
            self.verify_function(function);
        }
    }

    fn verify_function(&mut self, function: &BytecodeFunction) {
        let block_ids = function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<HashSet<_>>();
        let mut has_terminator = false;
        for entry in &function.exception_table {
            if !block_ids.contains(&entry.start_block)
                || !block_ids.contains(&entry.end_block)
                || !block_ids.contains(&entry.handler_block)
                || entry.start_block > entry.end_block
            {
                self.error(
                    VerifyErrorCode::InvalidExceptionEntry,
                    "invalid exception table entry",
                );
            }
        }

        for block in &function.blocks {
            let mut defined_values = HashSet::new();
            let mut local_count = function.params.len() as u32;
            for instruction in &block.instructions {
                self.verify_instruction(
                    instruction,
                    &block_ids,
                    &mut defined_values,
                    &mut local_count,
                );
                if matches!(
                    instruction.opcode,
                    Opcode::Return | Opcode::Jump | Opcode::Branch
                ) {
                    has_terminator = true;
                }
            }
        }

        if !has_terminator {
            self.error(
                VerifyErrorCode::MissingTerminator,
                "function has no return, jump, or branch",
            );
        }
    }

    fn verify_instruction(
        &mut self,
        instruction: &Instruction,
        block_ids: &HashSet<u32>,
        defined_values: &mut HashSet<u32>,
        local_count: &mut u32,
    ) {
        if instruction.source_ref as usize >= self.module.source_map.len() {
            self.error(
                VerifyErrorCode::InvalidSourceReference,
                "invalid source map reference",
            );
        }

        match instruction.opcode {
            Opcode::LoadConst => {
                self.expect_operands(instruction, 1);
                self.check_constant(instruction.operands.first().copied());
                self.define_value(instruction, defined_values);
            }
            Opcode::BuildObject => {
                if let Some(count) = instruction.operands.first().copied() {
                    let expected = 1 + count as usize * 2;
                    if instruction.operands.len() != expected {
                        self.error(
                            VerifyErrorCode::InvalidValueReference,
                            "object operand count does not match property count",
                        );
                    }
                    for pair in instruction.operands[1..].chunks(2) {
                        self.check_constant(pair.first().copied());
                        self.check_value(pair.get(1).copied(), defined_values);
                    }
                } else {
                    self.error(
                        VerifyErrorCode::InvalidValueReference,
                        "object instruction missing property count",
                    );
                }
                self.define_value(instruction, defined_values);
            }
            Opcode::BuildArray => {
                if let Some(count) = instruction.operands.first().copied() {
                    if instruction.operands.len() != count as usize + 1 {
                        self.error(
                            VerifyErrorCode::InvalidValueReference,
                            "array operand count does not match element count",
                        );
                    }
                    for operand in instruction.operands.iter().skip(1) {
                        self.check_value(Some(*operand), defined_values);
                    }
                } else {
                    self.error(
                        VerifyErrorCode::InvalidValueReference,
                        "array instruction missing element count",
                    );
                }
                self.define_value(instruction, defined_values);
            }
            Opcode::LoadLocal => {
                self.expect_operands(instruction, 1);
                self.check_local(instruction.operands.first().copied(), *local_count);
                self.define_value(instruction, defined_values);
            }
            Opcode::StoreLocal => {
                self.expect_operands(instruction, 2);
                if let Some(local) = instruction.operands.first().copied() {
                    if local > *local_count {
                        self.error(
                            VerifyErrorCode::InvalidLocalReference,
                            "store skips local index",
                        );
                    }
                    if local == *local_count {
                        *local_count += 1;
                    }
                }
                self.check_value(instruction.operands.get(1).copied(), defined_values);
            }
            Opcode::LoadMember => {
                self.expect_operands(instruction, 2);
                self.check_value(instruction.operands.first().copied(), defined_values);
                self.check_constant(instruction.operands.get(1).copied());
                self.define_value(instruction, defined_values);
            }
            Opcode::StoreMember => {
                self.expect_operands(instruction, 3);
                self.check_value(instruction.operands.first().copied(), defined_values);
                self.check_constant(instruction.operands.get(1).copied());
                self.check_value(instruction.operands.get(2).copied(), defined_values);
            }
            Opcode::Binary => {
                self.expect_operands(instruction, 3);
                self.check_value(instruction.operands.get(1).copied(), defined_values);
                self.check_value(instruction.operands.get(2).copied(), defined_values);
                if instruction.type_tag == TypeTag::Unknown {
                    self.error(
                        VerifyErrorCode::InvalidTypeState,
                        "binary instruction must have verifier-visible type",
                    );
                }
                self.define_value(instruction, defined_values);
            }
            Opcode::Call => {
                if instruction.operands.len() < 2 {
                    self.error(
                        VerifyErrorCode::InvalidValueReference,
                        "call operands missing",
                    );
                    return;
                }
                self.check_constant(instruction.operands.first().copied());
                let argc = instruction.operands[1] as usize;
                if instruction.operands.len() != argc + 2 {
                    self.error(
                        VerifyErrorCode::InvalidValueReference,
                        "call argument count does not match operands",
                    );
                }
                for operand in instruction.operands.iter().skip(2) {
                    self.check_value(Some(*operand), defined_values);
                }
                self.define_value(instruction, defined_values);
            }
            Opcode::Branch => {
                self.expect_operands(instruction, 3);
                self.check_value(instruction.operands.first().copied(), defined_values);
                self.check_block(instruction.operands.get(1).copied(), block_ids);
                self.check_block(instruction.operands.get(2).copied(), block_ids);
            }
            Opcode::Jump => {
                self.expect_operands(instruction, 1);
                self.check_block(instruction.operands.first().copied(), block_ids);
            }
            Opcode::Return => {
                if instruction.operands.len() > 1 {
                    self.error(
                        VerifyErrorCode::InvalidValueReference,
                        "return accepts zero or one value",
                    );
                }
                if let Some(value) = instruction.operands.first().copied() {
                    self.check_value(Some(value), defined_values);
                }
            }
        }
    }

    fn expect_operands(&mut self, instruction: &Instruction, count: usize) {
        if instruction.operands.len() != count {
            self.error(
                VerifyErrorCode::InvalidValueReference,
                "invalid operand count",
            );
        }
    }

    fn check_constant(&mut self, index: Option<u32>) {
        if index.is_none_or(|index| index as usize >= self.module.constants.len()) {
            self.error(
                VerifyErrorCode::InvalidConstantReference,
                "invalid constant pool reference",
            );
        }
    }

    fn check_local(&mut self, index: Option<u32>, local_count: u32) {
        if index.is_none_or(|index| index >= local_count) {
            self.error(
                VerifyErrorCode::InvalidLocalReference,
                "invalid local reference",
            );
        }
    }

    fn check_value(&mut self, index: Option<u32>, defined_values: &HashSet<u32>) {
        if index.is_none_or(|index| !defined_values.contains(&index)) {
            self.error(
                VerifyErrorCode::InvalidValueReference,
                "invalid value reference",
            );
        }
    }

    fn check_block(&mut self, index: Option<u32>, block_ids: &HashSet<u32>) {
        if index.is_none_or(|index| !block_ids.contains(&index)) {
            self.error(VerifyErrorCode::InvalidJumpTarget, "invalid jump target");
        }
    }

    fn define_value(&mut self, instruction: &Instruction, defined_values: &mut HashSet<u32>) {
        if let Some(value) = instruction_result_value(instruction, defined_values.len() as u32) {
            defined_values.insert(value);
        }
    }

    fn error(&mut self, code: VerifyErrorCode, message: &str) {
        self.errors.push(VerifyError {
            code,
            message: message.into(),
        });
    }
}

fn instruction_result_value(instruction: &Instruction, next_value: u32) -> Option<u32> {
    match instruction.opcode {
        Opcode::LoadConst
        | Opcode::BuildObject
        | Opcode::BuildArray
        | Opcode::LoadLocal
        | Opcode::LoadMember
        | Opcode::Binary
        | Opcode::Call => Some(next_value),
        _ => None,
    }
}

fn local_id(name: &str, local_ids: &mut HashMap<String, u32>, next_local: &mut u32) -> u32 {
    if let Some(id) = local_ids.get(name) {
        *id
    } else {
        let id = *next_local;
        *next_local += 1;
        local_ids.insert(name.into(), id);
        id
    }
}

fn type_tag(ty: &IrType) -> TypeTag {
    match ty {
        IrType::Number => TypeTag::Number,
        IrType::String => TypeTag::String,
        IrType::Boolean => TypeTag::Boolean,
        IrType::Null => TypeTag::Null,
        IrType::Undefined => TypeTag::Undefined,
        IrType::Object(_) => TypeTag::Object,
        IrType::Array(_) => TypeTag::Array,
        IrType::Named(_) => TypeTag::Named,
        IrType::Unknown => TypeTag::Unknown,
    }
}

fn binary_op_code(op: IrBinaryOp) -> u32 {
    match op {
        IrBinaryOp::Add => 0,
        IrBinaryOp::Subtract => 1,
        IrBinaryOp::Multiply => 2,
        IrBinaryOp::Divide => 3,
        IrBinaryOp::Remainder => 4,
        IrBinaryOp::Equal => 5,
        IrBinaryOp::StrictEqual => 6,
        IrBinaryOp::NotEqual => 7,
        IrBinaryOp::StrictNotEqual => 8,
        IrBinaryOp::Less => 9,
        IrBinaryOp::LessEqual => 10,
        IrBinaryOp::Greater => 11,
        IrBinaryOp::GreaterEqual => 12,
        IrBinaryOp::LogicalAnd => 13,
        IrBinaryOp::LogicalOr => 14,
        IrBinaryOp::NullishCoalesce => 15,
    }
}

fn write_constant(out: &mut Vec<u8>, constant: &Constant) {
    match constant {
        Constant::Number(value) => {
            out.push(0);
            write_string(out, value);
        }
        Constant::String(value) => {
            out.push(1);
            write_string(out, value);
        }
        Constant::Boolean(value) => {
            out.push(2);
            out.push(u8::from(*value));
        }
        Constant::Null => out.push(3),
        Constant::Undefined => out.push(4),
        Constant::Symbol(value) => {
            out.push(5);
            write_string(out, value);
        }
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_u32(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn opcode_to_u8(opcode: Opcode) -> u8 {
    match opcode {
        Opcode::LoadConst => 0,
        Opcode::BuildObject => 1,
        Opcode::BuildArray => 2,
        Opcode::LoadLocal => 3,
        Opcode::StoreLocal => 4,
        Opcode::LoadMember => 5,
        Opcode::StoreMember => 6,
        Opcode::Binary => 7,
        Opcode::Call => 8,
        Opcode::Branch => 9,
        Opcode::Jump => 10,
        Opcode::Return => 11,
    }
}

fn opcode_from_u8(value: u8) -> Option<Opcode> {
    Some(match value {
        0 => Opcode::LoadConst,
        1 => Opcode::BuildObject,
        2 => Opcode::BuildArray,
        3 => Opcode::LoadLocal,
        4 => Opcode::StoreLocal,
        5 => Opcode::LoadMember,
        6 => Opcode::StoreMember,
        7 => Opcode::Binary,
        8 => Opcode::Call,
        9 => Opcode::Branch,
        10 => Opcode::Jump,
        11 => Opcode::Return,
        _ => return None,
    })
}

fn type_tag_to_u8(tag: TypeTag) -> u8 {
    match tag {
        TypeTag::Number => 0,
        TypeTag::String => 1,
        TypeTag::Boolean => 2,
        TypeTag::Null => 3,
        TypeTag::Undefined => 4,
        TypeTag::Object => 5,
        TypeTag::Array => 6,
        TypeTag::Named => 7,
        TypeTag::Unknown => 8,
    }
}

fn type_tag_from_u8(value: u8) -> Option<TypeTag> {
    Some(match value {
        0 => TypeTag::Number,
        1 => TypeTag::String,
        2 => TypeTag::Boolean,
        3 => TypeTag::Null,
        4 => TypeTag::Undefined,
        5 => TypeTag::Object,
        6 => TypeTag::Array,
        7 => TypeTag::Named,
        8 => TypeTag::Unknown,
        _ => return None,
    })
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    cursor: usize,
}

impl<'bytes> Reader<'bytes> {
    fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn read_magic(&mut self) -> Result<[u8; 4], DecodeError> {
        let bytes = self.read_exact(4)?;
        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, DecodeError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_string(&mut self) -> Result<String, DecodeError> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|err| DecodeError {
                message: format!("invalid UTF-8 string: {err}"),
            })
    }

    fn read_constant(&mut self) -> Result<Constant, DecodeError> {
        match self.read_u8()? {
            0 => Ok(Constant::Number(self.read_string()?)),
            1 => Ok(Constant::String(self.read_string()?)),
            2 => Ok(Constant::Boolean(self.read_u8()? != 0)),
            3 => Ok(Constant::Null),
            4 => Ok(Constant::Undefined),
            5 => Ok(Constant::Symbol(self.read_string()?)),
            tag => Err(DecodeError {
                message: format!("invalid constant tag {tag}"),
            }),
        }
    }

    fn read_opcode(&mut self) -> Result<Opcode, DecodeError> {
        let value = self.read_u8()?;
        opcode_from_u8(value).ok_or_else(|| DecodeError {
            message: format!("invalid opcode {value}"),
        })
    }

    fn read_type_tag(&mut self) -> Result<TypeTag, DecodeError> {
        let value = self.read_u8()?;
        type_tag_from_u8(value).ok_or_else(|| DecodeError {
            message: format!("invalid type tag {value}"),
        })
    }

    fn finish(&self) -> Result<(), DecodeError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(DecodeError {
                message: "trailing bytes after bytecode module".into(),
            })
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'bytes [u8], DecodeError> {
        let end = self.cursor.checked_add(len).ok_or_else(|| DecodeError {
            message: "bytecode length overflow".into(),
        })?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or_else(|| DecodeError {
                message: "unexpected end of bytecode".into(),
            })?;
        self.cursor = end;
        Ok(bytes)
    }
}
