#![forbid(unsafe_code)]

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsvm_bytecode::{
    compile_source, verify_module, BytecodeBlock, BytecodeFunction, BytecodeModule, Constant,
    Instruction, Opcode, VerifyError,
};
use tsvm_semantic::SemanticDiagnostic;

#[derive(Debug, Clone)]
pub struct ExecutionOutput {
    pub console: Vec<Value>,
    pub return_value: Value,
}

impl PartialEq for ExecutionOutput {
    fn eq(&self, other: &Self) -> bool {
        self.console == other.console && self.return_value == other.return_value
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object(Rc<RefCell<BTreeMap<String, Value>>>),
    Array(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(left), Value::Number(right)) => left == right,
            (Value::String(left), Value::String(right)) => left == right,
            (Value::Boolean(left), Value::Boolean(right)) => left == right,
            (Value::Null, Value::Null) | (Value::Undefined, Value::Undefined) => true,
            (Value::Array(left), Value::Array(right)) => left == right,
            (Value::Object(left), Value::Object(right)) => left.borrow().eq(&right.borrow()),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecuteError {
    Compile(Vec<SemanticDiagnostic>),
    Verify(Vec<VerifyError>),
    Runtime(String),
}

pub fn execute_source(source: &str) -> Result<ExecutionOutput, ExecuteError> {
    let compiled = compile_source(source);
    let Some(module) = compiled.module else {
        return Err(ExecuteError::Compile(compiled.diagnostics));
    };
    execute_module(&module)
}

pub fn execute_module(module: &BytecodeModule) -> Result<ExecutionOutput, ExecuteError> {
    verify_module(module).map_err(ExecuteError::Verify)?;
    Interpreter::new(module).execute()
}

struct Interpreter<'module> {
    module: &'module BytecodeModule,
    console: Vec<Value>,
}

impl<'module> Interpreter<'module> {
    fn new(module: &'module BytecodeModule) -> Self {
        Self {
            module,
            console: Vec::new(),
        }
    }

    fn execute(mut self) -> Result<ExecutionOutput, ExecuteError> {
        let return_value = self.call_function("__entry", Vec::new())?;
        Ok(ExecutionOutput {
            console: self.console,
            return_value,
        })
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, ExecuteError> {
        if name == "console.log" {
            self.console.extend(args);
            return Ok(Value::Undefined);
        }

        let function = self
            .module
            .functions
            .iter()
            .find(|function| function.name == name)
            .ok_or_else(|| ExecuteError::Runtime(format!("unknown function `{name}`")))?;
        let mut frame = Frame::new(function, args);
        self.execute_frame(function, &mut frame)
    }

    fn execute_frame(
        &mut self,
        function: &BytecodeFunction,
        frame: &mut Frame,
    ) -> Result<Value, ExecuteError> {
        let mut block_id = 0_u32;

        loop {
            let block = find_block(function, block_id)?;
            let mut jumped = false;
            for instruction in &block.instructions {
                match self.execute_instruction(instruction, frame)? {
                    ControlFlow::Continue => {}
                    ControlFlow::Jump(next) => {
                        block_id = next;
                        jumped = true;
                        break;
                    }
                    ControlFlow::Return(value) => return Ok(value),
                }
            }

            if !jumped {
                return Ok(Value::Undefined);
            }
        }
    }

    fn execute_instruction(
        &mut self,
        instruction: &Instruction,
        frame: &mut Frame,
    ) -> Result<ControlFlow, ExecuteError> {
        match instruction.opcode {
            Opcode::LoadConst => {
                let constant = self.constant(instruction.operands[0])?;
                frame.push_value(value_from_constant(constant)?);
            }
            Opcode::BuildObject => {
                let count = instruction.operands[0] as usize;
                let mut object = BTreeMap::new();
                for index in 0..count {
                    let name_constant = self.constant(instruction.operands[1 + index * 2])?;
                    let value_id = instruction.operands[2 + index * 2];
                    let name = symbol_name(name_constant)?;
                    object.insert(name.to_owned(), frame.value(value_id)?.clone());
                }
                frame.push_value(Value::Object(Rc::new(RefCell::new(object))));
            }
            Opcode::BuildArray => {
                let count = instruction.operands[0] as usize;
                let mut values = Vec::with_capacity(count);
                for operand in instruction.operands.iter().skip(1) {
                    values.push(frame.value(*operand)?.clone());
                }
                frame.push_value(Value::Array(values));
            }
            Opcode::LoadLocal => {
                frame.push_value(frame.local(instruction.operands[0])?.clone());
            }
            Opcode::StoreLocal => {
                let value = frame.value(instruction.operands[1])?.clone();
                frame.store_local(instruction.operands[0], value);
            }
            Opcode::LoadMember => {
                let object = frame.value(instruction.operands[0])?;
                let property = symbol_name(self.constant(instruction.operands[1])?)?;
                frame.push_value(load_member(object, property)?);
            }
            Opcode::StoreMember => {
                let object = frame.value(instruction.operands[0])?.clone();
                let property = symbol_name(self.constant(instruction.operands[1])?)?;
                let value = frame.value(instruction.operands[2])?.clone();
                store_member(&object, property, value)?;
            }
            Opcode::Binary => {
                let left = frame.value(instruction.operands[1])?.clone();
                let right = frame.value(instruction.operands[2])?.clone();
                frame.push_value(binary(instruction.operands[0], &left, &right)?);
            }
            Opcode::Call => {
                let callee = symbol_name(self.constant(instruction.operands[0])?)?.to_owned();
                let argc = instruction.operands[1] as usize;
                let mut args = Vec::with_capacity(argc);
                for operand in instruction.operands.iter().skip(2).take(argc) {
                    args.push(frame.value(*operand)?.clone());
                }
                let value = self.call_function(&callee, args)?;
                frame.push_value(value);
            }
            Opcode::Branch => {
                let condition = frame.value(instruction.operands[0])?;
                let target = if truthy(condition) {
                    instruction.operands[1]
                } else {
                    instruction.operands[2]
                };
                return Ok(ControlFlow::Jump(target));
            }
            Opcode::Jump => return Ok(ControlFlow::Jump(instruction.operands[0])),
            Opcode::Return => {
                let value = instruction
                    .operands
                    .first()
                    .map_or(Ok(Value::Undefined), |value| frame.value(*value).cloned())?;
                return Ok(ControlFlow::Return(value));
            }
        }

        Ok(ControlFlow::Continue)
    }

    fn constant(&self, index: u32) -> Result<&Constant, ExecuteError> {
        self.module
            .constants
            .get(index as usize)
            .ok_or_else(|| ExecuteError::Runtime(format!("constant {index} missing")))
    }
}

struct Frame {
    locals: Vec<Value>,
    values: Vec<Value>,
}

impl Frame {
    fn new(function: &BytecodeFunction, args: Vec<Value>) -> Self {
        let mut locals = args;
        locals.resize(function.params.len(), Value::Undefined);
        Self {
            locals,
            values: Vec::new(),
        }
    }

    fn local(&self, index: u32) -> Result<&Value, ExecuteError> {
        self.locals
            .get(index as usize)
            .ok_or_else(|| ExecuteError::Runtime(format!("local {index} missing")))
    }

    fn store_local(&mut self, index: u32, value: Value) {
        let index = index as usize;
        if self.locals.len() <= index {
            self.locals.resize(index + 1, Value::Undefined);
        }
        self.locals[index] = value;
    }

    fn value(&self, index: u32) -> Result<&Value, ExecuteError> {
        self.values
            .get(index as usize)
            .ok_or_else(|| ExecuteError::Runtime(format!("value {index} missing")))
    }

    fn push_value(&mut self, value: Value) {
        self.values.push(value);
    }
}

enum ControlFlow {
    Continue,
    Jump(u32),
    Return(Value),
}

fn find_block(function: &BytecodeFunction, id: u32) -> Result<&BytecodeBlock, ExecuteError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or_else(|| ExecuteError::Runtime(format!("block {id} missing")))
}

fn value_from_constant(constant: &Constant) -> Result<Value, ExecuteError> {
    Ok(match constant {
        Constant::Number(value) => Value::Number(value.parse().map_err(|err| {
            ExecuteError::Runtime(format!("invalid number constant `{value}`: {err}"))
        })?),
        Constant::String(value) | Constant::Symbol(value) => Value::String(value.clone()),
        Constant::Boolean(value) => Value::Boolean(*value),
        Constant::Null => Value::Null,
        Constant::Undefined => Value::Undefined,
    })
}

fn symbol_name(constant: &Constant) -> Result<&str, ExecuteError> {
    match constant {
        Constant::Symbol(value) | Constant::String(value) => Ok(value),
        _ => Err(ExecuteError::Runtime("expected symbol constant".into())),
    }
}

fn load_member(object: &Value, property: &str) -> Result<Value, ExecuteError> {
    match object {
        Value::Object(fields) => Ok(fields
            .borrow()
            .get(property)
            .cloned()
            .unwrap_or(Value::Undefined)),
        _ => Err(ExecuteError::Runtime(format!(
            "cannot read property `{property}` from non-object"
        ))),
    }
}

fn store_member(object: &Value, property: &str, value: Value) -> Result<(), ExecuteError> {
    match object {
        Value::Object(fields) => {
            fields.borrow_mut().insert(property.into(), value);
            Ok(())
        }
        _ => Err(ExecuteError::Runtime(format!(
            "cannot write property `{property}` on non-object"
        ))),
    }
}

fn binary(op: u32, left: &Value, right: &Value) -> Result<Value, ExecuteError> {
    match op {
        0 => match (left, right) {
            (Value::String(left), right) => Ok(Value::String(format!("{left}{}", display(right)))),
            (left, Value::String(right)) => Ok(Value::String(format!("{}{right}", display(left)))),
            _ => Ok(Value::Number(number(left)? + number(right)?)),
        },
        1 => Ok(Value::Number(number(left)? - number(right)?)),
        2 => Ok(Value::Number(number(left)? * number(right)?)),
        3 => Ok(Value::Number(number(left)? / number(right)?)),
        4 => Ok(Value::Number(number(left)? % number(right)?)),
        5 => Ok(Value::Boolean(left == right)),
        6 => Ok(Value::Boolean(left == right)),
        7 => Ok(Value::Boolean(left != right)),
        8 => Ok(Value::Boolean(left != right)),
        9 => Ok(Value::Boolean(number(left)? < number(right)?)),
        10 => Ok(Value::Boolean(number(left)? <= number(right)?)),
        11 => Ok(Value::Boolean(number(left)? > number(right)?)),
        12 => Ok(Value::Boolean(number(left)? >= number(right)?)),
        13 => Ok(Value::Boolean(truthy(left) && truthy(right))),
        14 => Ok(Value::Boolean(truthy(left) || truthy(right))),
        15 => {
            if matches!(left, Value::Null | Value::Undefined) {
                Ok(right.clone())
            } else {
                Ok(left.clone())
            }
        }
        _ => Err(ExecuteError::Runtime(format!("unknown binary op {op}"))),
    }
}

fn number(value: &Value) -> Result<f64, ExecuteError> {
    match value {
        Value::Number(value) => Ok(*value),
        _ => Err(ExecuteError::Runtime(format!(
            "expected number, found {}",
            display(value)
        ))),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Boolean(value) => *value,
        Value::Null | Value::Undefined => false,
        Value::Number(value) => *value != 0.0,
        Value::String(value) => !value.is_empty(),
        Value::Object(_) | Value::Array(_) => true,
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Boolean(value) => value.to_string(),
        Value::Null => "null".into(),
        Value::Undefined => "undefined".into(),
        Value::Object(_) => "[object Object]".into(),
        Value::Array(values) => values.iter().map(display).collect::<Vec<_>>().join(","),
    }
}
