#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use tsvm_bytecode::{
    compile_source, verify_module, BytecodeBlock, BytecodeFunction, BytecodeModule, Constant,
    Instruction, Opcode, VerifyError,
};
use tsvm_heap::{CollectionReport, GcHeap, HeapHandle, Trace, Tracer};
use tsvm_interop::{HostEnvironment, InteropError, InteropValue};
use tsvm_modules::{bundle_module_graph, ModuleDiagnostic};
use tsvm_semantic::SemanticDiagnostic;

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionOutput {
    pub console: Vec<Value>,
    pub return_value: Value,
    pub heap: HeapStats,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct HeapStats {
    pub live_objects: usize,
    pub allocated_slots: usize,
    pub last_collection: CollectionReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object(BTreeMap<String, Value>),
    Array(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecuteError {
    Module(Vec<ModuleDiagnostic>),
    Compile(Vec<SemanticDiagnostic>),
    Verify(Vec<VerifyError>),
    Interop(InteropError),
    Runtime(String),
}

pub fn execute_source(source: &str) -> Result<ExecutionOutput, ExecuteError> {
    execute_source_with_host(source, &HostEnvironment::new())
}

pub fn execute_source_with_host(
    source: &str,
    host: &HostEnvironment,
) -> Result<ExecutionOutput, ExecuteError> {
    let compiled = compile_source(source);
    let Some(module) = compiled.module else {
        return Err(ExecuteError::Compile(compiled.diagnostics));
    };
    execute_module_with_host(&module, host)
}

pub fn execute_module_graph(
    entry: &str,
    sources: &BTreeMap<String, String>,
) -> Result<ExecutionOutput, ExecuteError> {
    let graph = bundle_module_graph(entry, sources).map_err(ExecuteError::Module)?;
    execute_source(&graph.bundled_source)
}

pub fn execute_module(module: &BytecodeModule) -> Result<ExecutionOutput, ExecuteError> {
    execute_module_with_host(module, &HostEnvironment::new())
}

pub fn execute_module_with_host(
    module: &BytecodeModule,
    host: &HostEnvironment,
) -> Result<ExecutionOutput, ExecuteError> {
    verify_module(module).map_err(ExecuteError::Verify)?;
    Interpreter::new(module, host).execute()
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedModule {
    module: BytecodeModule,
}

impl PreparedModule {
    pub fn from_source(source: &str) -> Result<Self, ExecuteError> {
        let compiled = compile_source(source);
        let Some(module) = compiled.module else {
            return Err(ExecuteError::Compile(compiled.diagnostics));
        };
        verify_module(&module).map_err(ExecuteError::Verify)?;
        Ok(Self { module })
    }

    pub fn call_function(
        &self,
        name: &str,
        args: &[InteropValue],
        host: &HostEnvironment,
    ) -> Result<InteropValue, ExecuteError> {
        let mut interpreter = Interpreter::new(&self.module, host);
        let runtime_args = args
            .iter()
            .map(|value| runtime_from_interop(value, &mut interpreter.heap))
            .collect::<RuntimeValues>()?;
        let result = interpreter.call_function(name, runtime_args)?;
        let mut roots = Vec::new();
        extend_roots(&result, &mut roots);
        interpreter.heap.collect(roots);
        let value = materialize(&result, &interpreter.heap)?;
        Ok(interop_from_value(value))
    }
}

type RuntimeValues = Result<Vec<RuntimeValue>, ExecuteError>;

struct Interpreter<'module, 'host> {
    module: &'module BytecodeModule,
    host: &'host HostEnvironment,
    heap: GcHeap<HeapValue>,
    console: Vec<RuntimeValue>,
}

impl<'module, 'host> Interpreter<'module, 'host> {
    fn new(module: &'module BytecodeModule, host: &'host HostEnvironment) -> Self {
        Self {
            module,
            host,
            heap: GcHeap::new(),
            console: Vec::new(),
        }
    }

    fn execute(mut self) -> Result<ExecutionOutput, ExecuteError> {
        let return_value = self.call_function("__entry", Vec::new())?;
        let mut roots = runtime_roots(&self.console);
        extend_roots(&return_value, &mut roots);
        let last_collection = self.heap.collect(roots);

        let console = self
            .console
            .iter()
            .map(|value| materialize(value, &self.heap))
            .collect::<Result<Vec<_>, _>>()?;
        let return_value = materialize(&return_value, &self.heap)?;
        let heap = HeapStats {
            live_objects: self.heap.live_len(),
            allocated_slots: self.heap.capacity(),
            last_collection,
        };

        Ok(ExecutionOutput {
            console,
            return_value,
            heap,
        })
    }

    fn call_function(
        &mut self,
        name: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, ExecuteError> {
        if name == "console.log" {
            self.console.extend(args);
            return Ok(RuntimeValue::Undefined);
        }
        if let Some(result) = self.call_host(name, &args)? {
            return Ok(result);
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

    fn call_host(
        &mut self,
        name: &str,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, ExecuteError> {
        let args = args
            .iter()
            .map(|value| materialize(value, &self.heap).map(interop_from_value))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(result) = self.host.call(name, &args) else {
            return Ok(None);
        };
        result
            .and_then(|value| {
                runtime_from_interop(&value, &mut self.heap).map_err(|err| match err {
                    ExecuteError::Interop(err) => err,
                    ExecuteError::Runtime(message) => InteropError::new(message),
                    _ => InteropError::new("unexpected interop conversion error"),
                })
            })
            .map(Some)
            .map_err(ExecuteError::Interop)
    }

    fn execute_frame(
        &mut self,
        function: &BytecodeFunction,
        frame: &mut Frame,
    ) -> Result<RuntimeValue, ExecuteError> {
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
                return Ok(RuntimeValue::Undefined);
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
                let handle = self.heap.allocate(HeapValue::Object(object));
                frame.push_value(RuntimeValue::Object(handle));
            }
            Opcode::BuildArray => {
                let count = instruction.operands[0] as usize;
                let mut values = Vec::with_capacity(count);
                for operand in instruction.operands.iter().skip(1) {
                    values.push(frame.value(*operand)?.clone());
                }
                let handle = self.heap.allocate(HeapValue::Array(values));
                frame.push_value(RuntimeValue::Array(handle));
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
                frame.push_value(load_member(&self.heap, object, property)?);
            }
            Opcode::StoreMember => {
                let object = frame.value(instruction.operands[0])?.clone();
                let property = symbol_name(self.constant(instruction.operands[1])?)?.to_owned();
                let value = frame.value(instruction.operands[2])?.clone();
                store_member(&mut self.heap, &object, &property, value)?;
            }
            Opcode::Binary => {
                let left = frame.value(instruction.operands[1])?.clone();
                let right = frame.value(instruction.operands[2])?.clone();
                frame.push_value(binary(instruction.operands[0], &self.heap, &left, &right)?);
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
                    .map_or(Ok(RuntimeValue::Undefined), |value| {
                        frame.value(*value).cloned()
                    })?;
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

#[derive(Debug, Clone, PartialEq)]
enum RuntimeValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object(HeapHandle),
    Array(HeapHandle),
}

impl Trace for RuntimeValue {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self {
            RuntimeValue::Object(handle) | RuntimeValue::Array(handle) => tracer.mark(*handle),
            RuntimeValue::Number(_)
            | RuntimeValue::String(_)
            | RuntimeValue::Boolean(_)
            | RuntimeValue::Null
            | RuntimeValue::Undefined => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum HeapValue {
    Object(BTreeMap<String, RuntimeValue>),
    Array(Vec<RuntimeValue>),
}

impl Trace for HeapValue {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self {
            HeapValue::Object(fields) => {
                for value in fields.values() {
                    value.trace(tracer);
                }
            }
            HeapValue::Array(values) => {
                for value in values {
                    value.trace(tracer);
                }
            }
        }
    }
}

struct Frame {
    locals: Vec<RuntimeValue>,
    values: Vec<RuntimeValue>,
}

impl Frame {
    fn new(function: &BytecodeFunction, args: Vec<RuntimeValue>) -> Self {
        let mut locals = args;
        locals.resize(function.params.len(), RuntimeValue::Undefined);
        Self {
            locals,
            values: Vec::new(),
        }
    }

    fn local(&self, index: u32) -> Result<&RuntimeValue, ExecuteError> {
        self.locals
            .get(index as usize)
            .ok_or_else(|| ExecuteError::Runtime(format!("local {index} missing")))
    }

    fn store_local(&mut self, index: u32, value: RuntimeValue) {
        let index = index as usize;
        if self.locals.len() <= index {
            self.locals.resize(index + 1, RuntimeValue::Undefined);
        }
        self.locals[index] = value;
    }

    fn value(&self, index: u32) -> Result<&RuntimeValue, ExecuteError> {
        self.values
            .get(index as usize)
            .ok_or_else(|| ExecuteError::Runtime(format!("value {index} missing")))
    }

    fn push_value(&mut self, value: RuntimeValue) {
        self.values.push(value);
    }
}

enum ControlFlow {
    Continue,
    Jump(u32),
    Return(RuntimeValue),
}

fn find_block(function: &BytecodeFunction, id: u32) -> Result<&BytecodeBlock, ExecuteError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or_else(|| ExecuteError::Runtime(format!("block {id} missing")))
}

fn value_from_constant(constant: &Constant) -> Result<RuntimeValue, ExecuteError> {
    Ok(match constant {
        Constant::Number(value) => RuntimeValue::Number(value.parse().map_err(|err| {
            ExecuteError::Runtime(format!("invalid number constant `{value}`: {err}"))
        })?),
        Constant::String(value) | Constant::Symbol(value) => RuntimeValue::String(value.clone()),
        Constant::Boolean(value) => RuntimeValue::Boolean(*value),
        Constant::Null => RuntimeValue::Null,
        Constant::Undefined => RuntimeValue::Undefined,
    })
}

fn symbol_name(constant: &Constant) -> Result<&str, ExecuteError> {
    match constant {
        Constant::Symbol(value) | Constant::String(value) => Ok(value),
        _ => Err(ExecuteError::Runtime("expected symbol constant".into())),
    }
}

fn load_member(
    heap: &GcHeap<HeapValue>,
    object: &RuntimeValue,
    property: &str,
) -> Result<RuntimeValue, ExecuteError> {
    match object {
        RuntimeValue::Object(handle) => match heap.get(*handle) {
            Some(HeapValue::Object(fields)) => Ok(fields
                .get(property)
                .cloned()
                .unwrap_or(RuntimeValue::Undefined)),
            _ => Err(ExecuteError::Runtime("stale object handle".into())),
        },
        _ => Err(ExecuteError::Runtime(format!(
            "cannot read property `{property}` from non-object"
        ))),
    }
}

fn store_member(
    heap: &mut GcHeap<HeapValue>,
    object: &RuntimeValue,
    property: &str,
    value: RuntimeValue,
) -> Result<(), ExecuteError> {
    match object {
        RuntimeValue::Object(handle) => match heap.get_mut(*handle) {
            Some(HeapValue::Object(fields)) => {
                fields.insert(property.into(), value);
                Ok(())
            }
            _ => Err(ExecuteError::Runtime("stale object handle".into())),
        },
        _ => Err(ExecuteError::Runtime(format!(
            "cannot write property `{property}` on non-object"
        ))),
    }
}

fn binary(
    op: u32,
    heap: &GcHeap<HeapValue>,
    left: &RuntimeValue,
    right: &RuntimeValue,
) -> Result<RuntimeValue, ExecuteError> {
    match op {
        0 => match (left, right) {
            (RuntimeValue::String(left), right) => Ok(RuntimeValue::String(format!(
                "{left}{}",
                display(heap, right)?
            ))),
            (left, RuntimeValue::String(right)) => Ok(RuntimeValue::String(format!(
                "{}{right}",
                display(heap, left)?
            ))),
            _ => Ok(RuntimeValue::Number(number(left)? + number(right)?)),
        },
        1 => Ok(RuntimeValue::Number(number(left)? - number(right)?)),
        2 => Ok(RuntimeValue::Number(number(left)? * number(right)?)),
        3 => Ok(RuntimeValue::Number(number(left)? / number(right)?)),
        4 => Ok(RuntimeValue::Number(number(left)? % number(right)?)),
        5 => Ok(RuntimeValue::Boolean(left == right)),
        6 => Ok(RuntimeValue::Boolean(left == right)),
        7 => Ok(RuntimeValue::Boolean(left != right)),
        8 => Ok(RuntimeValue::Boolean(left != right)),
        9 => Ok(RuntimeValue::Boolean(number(left)? < number(right)?)),
        10 => Ok(RuntimeValue::Boolean(number(left)? <= number(right)?)),
        11 => Ok(RuntimeValue::Boolean(number(left)? > number(right)?)),
        12 => Ok(RuntimeValue::Boolean(number(left)? >= number(right)?)),
        13 => Ok(RuntimeValue::Boolean(truthy(left) && truthy(right))),
        14 => Ok(RuntimeValue::Boolean(truthy(left) || truthy(right))),
        15 => {
            if matches!(left, RuntimeValue::Null | RuntimeValue::Undefined) {
                Ok(right.clone())
            } else {
                Ok(left.clone())
            }
        }
        _ => Err(ExecuteError::Runtime(format!("unknown binary op {op}"))),
    }
}

fn number(value: &RuntimeValue) -> Result<f64, ExecuteError> {
    match value {
        RuntimeValue::Number(value) => Ok(*value),
        _ => Err(ExecuteError::Runtime("expected number".into())),
    }
}

fn truthy(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::Boolean(value) => *value,
        RuntimeValue::Null | RuntimeValue::Undefined => false,
        RuntimeValue::Number(value) => *value != 0.0,
        RuntimeValue::String(value) => !value.is_empty(),
        RuntimeValue::Object(_) | RuntimeValue::Array(_) => true,
    }
}

fn display(heap: &GcHeap<HeapValue>, value: &RuntimeValue) -> Result<String, ExecuteError> {
    Ok(match value {
        RuntimeValue::Number(value) => value.to_string(),
        RuntimeValue::String(value) => value.clone(),
        RuntimeValue::Boolean(value) => value.to_string(),
        RuntimeValue::Null => "null".into(),
        RuntimeValue::Undefined => "undefined".into(),
        RuntimeValue::Object(_) => "[object Object]".into(),
        RuntimeValue::Array(handle) => match heap.get(*handle) {
            Some(HeapValue::Array(values)) => values
                .iter()
                .map(|value| display(heap, value))
                .collect::<Result<Vec<_>, _>>()?
                .join(","),
            _ => return Err(ExecuteError::Runtime("stale array handle".into())),
        },
    })
}

fn runtime_roots(values: &[RuntimeValue]) -> Vec<HeapHandle> {
    let mut roots = Vec::new();
    for value in values {
        extend_roots(value, &mut roots);
    }
    roots
}

fn extend_roots(value: &RuntimeValue, roots: &mut Vec<HeapHandle>) {
    match value {
        RuntimeValue::Object(handle) | RuntimeValue::Array(handle) => roots.push(*handle),
        RuntimeValue::Number(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Boolean(_)
        | RuntimeValue::Null
        | RuntimeValue::Undefined => {}
    }
}

fn materialize(value: &RuntimeValue, heap: &GcHeap<HeapValue>) -> Result<Value, ExecuteError> {
    Ok(match value {
        RuntimeValue::Number(value) => Value::Number(*value),
        RuntimeValue::String(value) => Value::String(value.clone()),
        RuntimeValue::Boolean(value) => Value::Boolean(*value),
        RuntimeValue::Null => Value::Null,
        RuntimeValue::Undefined => Value::Undefined,
        RuntimeValue::Object(handle) => match heap.get(*handle) {
            Some(HeapValue::Object(fields)) => {
                let mut materialized = BTreeMap::new();
                for (name, value) in fields {
                    materialized.insert(name.clone(), materialize(value, heap)?);
                }
                Value::Object(materialized)
            }
            _ => return Err(ExecuteError::Runtime("stale object handle".into())),
        },
        RuntimeValue::Array(handle) => match heap.get(*handle) {
            Some(HeapValue::Array(values)) => Value::Array(
                values
                    .iter()
                    .map(|value| materialize(value, heap))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            _ => return Err(ExecuteError::Runtime("stale array handle".into())),
        },
    })
}

fn runtime_from_interop(
    value: &InteropValue,
    heap: &mut GcHeap<HeapValue>,
) -> Result<RuntimeValue, ExecuteError> {
    Ok(match value {
        InteropValue::Number(value) => RuntimeValue::Number(*value),
        InteropValue::String(value) => RuntimeValue::String(value.clone()),
        InteropValue::Boolean(value) => RuntimeValue::Boolean(*value),
        InteropValue::Null => RuntimeValue::Null,
        InteropValue::Undefined => RuntimeValue::Undefined,
        InteropValue::Object(fields) => {
            let mut runtime_fields = BTreeMap::new();
            for (name, value) in fields {
                runtime_fields.insert(name.clone(), runtime_from_interop(value, heap)?);
            }
            RuntimeValue::Object(heap.allocate(HeapValue::Object(runtime_fields)))
        }
        InteropValue::Array(values) => {
            let runtime_values = values
                .iter()
                .map(|value| runtime_from_interop(value, heap))
                .collect::<RuntimeValues>()?;
            RuntimeValue::Array(heap.allocate(HeapValue::Array(runtime_values)))
        }
    })
}

fn interop_from_value(value: Value) -> InteropValue {
    match value {
        Value::Number(value) => InteropValue::Number(value),
        Value::String(value) => InteropValue::String(value),
        Value::Boolean(value) => InteropValue::Boolean(value),
        Value::Null => InteropValue::Null,
        Value::Undefined => InteropValue::Undefined,
        Value::Object(fields) => InteropValue::Object(
            fields
                .into_iter()
                .map(|(name, value)| (name, interop_from_value(value)))
                .collect(),
        ),
        Value::Array(values) => {
            InteropValue::Array(values.into_iter().map(interop_from_value).collect())
        }
    }
}
