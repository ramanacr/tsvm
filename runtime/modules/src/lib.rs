#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap};

use tsvm_ast::{ImportDeclaration, Statement, StatementKind};
use tsvm_parser::parse_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGraph {
    pub entry: String,
    pub modules: Vec<ModuleRecord>,
    pub bundled_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    pub specifier: String,
    pub imports: Vec<ResolvedImport>,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub names: Vec<String>,
    pub from: String,
    pub resolved: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDiagnostic {
    pub code: ModuleDiagnosticCode,
    pub message: String,
    pub module: String,
    pub modules: Vec<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModuleDiagnosticCode {
    MissingModule,
    UnsupportedSpecifier,
    Cycle,
    Parse,
}

pub fn bundle_module_graph(
    entry: &str,
    sources: &BTreeMap<String, String>,
) -> Result<ModuleGraph, Vec<ModuleDiagnostic>> {
    let mut builder = GraphBuilder {
        sources,
        states: HashMap::new(),
        records: BTreeMap::new(),
        order: Vec::new(),
        stack: Vec::new(),
        diagnostics: Vec::new(),
    };
    let entry = normalize_entry(entry);
    builder.visit(&entry);

    if !builder.diagnostics.is_empty() {
        return Err(builder.diagnostics);
    }

    let modules = builder
        .order
        .iter()
        .filter_map(|specifier| builder.records.get(specifier).cloned())
        .collect::<Vec<_>>();
    let bundled_source = builder.bundle(&modules);

    Ok(ModuleGraph {
        entry,
        modules,
        bundled_source,
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

struct GraphBuilder<'sources> {
    sources: &'sources BTreeMap<String, String>,
    states: HashMap<String, VisitState>,
    records: BTreeMap<String, ModuleRecord>,
    order: Vec<String>,
    stack: Vec<String>,
    diagnostics: Vec<ModuleDiagnostic>,
}

impl GraphBuilder<'_> {
    fn visit(&mut self, module: &str) {
        match self.states.get(module) {
            Some(VisitState::Visited) => return,
            Some(VisitState::Visiting) => {
                self.report_cycle(module);
                return;
            }
            None => {}
        }

        let Some(source) = self.sources.get(module) else {
            self.diagnostics.push(ModuleDiagnostic {
                code: ModuleDiagnosticCode::MissingModule,
                message: format!("module `{module}` was not provided"),
                module: module.into(),
                modules: vec![module.into()],
            });
            return;
        };

        self.states.insert(module.into(), VisitState::Visiting);
        self.stack.push(module.into());

        let parsed = parse_source(source);
        for diagnostic in parsed.diagnostics {
            self.diagnostics.push(ModuleDiagnostic {
                code: ModuleDiagnosticCode::Parse,
                message: diagnostic.message,
                module: module.into(),
                modules: vec![module.into()],
            });
        }

        let mut imports = Vec::new();
        let mut exports = Vec::new();
        for statement in &parsed.program.body {
            match &statement.kind {
                StatementKind::Import(import) => match resolve_import(module, import) {
                    Ok(resolved) => {
                        self.visit(&resolved);
                        imports.push(ResolvedImport {
                            names: import.names.clone(),
                            from: import.from.clone(),
                            resolved,
                        });
                    }
                    Err(message) => self.diagnostics.push(ModuleDiagnostic {
                        code: ModuleDiagnosticCode::UnsupportedSpecifier,
                        message,
                        module: module.into(),
                        modules: vec![module.into()],
                    }),
                },
                StatementKind::Export(inner) => {
                    if let Some(name) = export_name(inner) {
                        exports.push(name);
                    }
                }
                _ => {}
            }
        }

        self.stack.pop();
        self.states.insert(module.into(), VisitState::Visited);
        self.records.insert(
            module.into(),
            ModuleRecord {
                specifier: module.into(),
                imports,
                exports,
            },
        );
        self.order.push(module.into());
    }

    fn report_cycle(&mut self, module: &str) {
        let start = self
            .stack
            .iter()
            .position(|entry| entry == module)
            .unwrap_or(0);
        let mut cycle = self.stack[start..].to_vec();
        cycle.push(module.into());
        self.diagnostics.push(ModuleDiagnostic {
            code: ModuleDiagnosticCode::Cycle,
            message: format!("module cycle detected: {}", cycle.join(" -> ")),
            module: module.into(),
            modules: cycle,
        });
    }

    fn bundle(&self, modules: &[ModuleRecord]) -> String {
        let mut bundled = String::new();
        for module in modules {
            let Some(source) = self.sources.get(&module.specifier) else {
                continue;
            };
            let parsed = parse_source(source);
            for statement in &parsed.program.body {
                match &statement.kind {
                    StatementKind::Import(_) => {}
                    StatementKind::Export(inner) => {
                        push_statement_source(&mut bundled, source, inner);
                    }
                    _ => push_statement_source(&mut bundled, source, statement),
                }
            }
        }
        bundled
    }
}

fn push_statement_source(out: &mut String, source: &str, statement: &Statement) {
    if let Some(slice) = source.get(statement.span.start.byte..statement.span.end.byte) {
        out.push_str(slice.trim());
        out.push('\n');
    }
}

fn export_name(statement: &Statement) -> Option<String> {
    match &statement.kind {
        StatementKind::Interface(item) => Some(item.name.clone()),
        StatementKind::TypeAlias(item) => Some(item.name.clone()),
        StatementKind::Class(item) => Some(item.name.clone()),
        StatementKind::Function(item) => Some(item.name.clone()),
        StatementKind::Variable(item) => Some(item.name.clone()),
        _ => None,
    }
}

fn resolve_import(from_module: &str, import: &ImportDeclaration) -> Result<String, String> {
    if !(import.from.starts_with("./") || import.from.starts_with("../")) {
        return Err(format!(
            "unsupported module specifier `{}`; only local relative TypeScript modules are allowed",
            import.from
        ));
    }
    if !import.from.ends_with(".ts") {
        return Err(format!(
            "unsupported module specifier `{}`; local module specifiers must end in `.ts`",
            import.from
        ));
    }

    let mut parts = from_module
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.pop();
    for part in import.from.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

fn normalize_entry(entry: &str) -> String {
    if entry.starts_with('/') {
        entry.into()
    } else {
        format!("/{entry}")
    }
}
