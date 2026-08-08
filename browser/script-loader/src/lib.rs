#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use tsvm_interop::HostEnvironment;
use tsvm_interpreter::{
    execute_module_graph, execute_source_with_host, ExecuteError, ExecutionOutput, Value,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserExecution {
    pub console: Vec<Value>,
    pub scripts: Vec<ScriptExecution>,
    pub generated_javascript: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptExecution {
    pub kind: ScriptKind,
    pub specifier: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScriptKind {
    External,
    Inline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptLoaderError {
    pub message: String,
    pub source: Option<ExecuteError>,
}

pub fn execute_typescript_scripts(
    document_url: &str,
    html: &str,
    resources: &BTreeMap<String, String>,
) -> Result<BrowserExecution, ScriptLoaderError> {
    execute_typescript_scripts_with_host(document_url, html, resources, &HostEnvironment::new())
}

pub fn execute_typescript_scripts_with_host(
    document_url: &str,
    html: &str,
    resources: &BTreeMap<String, String>,
    host: &HostEnvironment,
) -> Result<BrowserExecution, ScriptLoaderError> {
    let mut output = BrowserExecution {
        console: Vec::new(),
        scripts: Vec::new(),
        generated_javascript: false,
    };

    for script in find_scripts(html) {
        if script_type(script.open_tag).as_deref() != Some("text/typescript") {
            continue;
        }

        if let Some(src) = script_src(script.open_tag) {
            let specifier = resolve_url(document_url, &src);
            if !resources.contains_key(&specifier) {
                return Err(ScriptLoaderError {
                    message: format!("TypeScript script resource `{specifier}` was not provided"),
                    source: None,
                });
            }
            let execution =
                execute_module_graph(&specifier, resources).map_err(|err| ScriptLoaderError {
                    message: format!("failed to execute TypeScript script `{specifier}`"),
                    source: Some(err),
                })?;
            append_execution(&mut output, execution);
            output.scripts.push(ScriptExecution {
                kind: ScriptKind::External,
                specifier,
            });
        } else {
            let execution =
                execute_source_with_host(script.body, host).map_err(|err| ScriptLoaderError {
                    message: "failed to execute inline TypeScript script".into(),
                    source: Some(err),
                })?;
            append_execution(&mut output, execution);
            output.scripts.push(ScriptExecution {
                kind: ScriptKind::Inline,
                specifier: "<inline>".into(),
            });
        }
    }

    Ok(output)
}

fn append_execution(output: &mut BrowserExecution, execution: ExecutionOutput) {
    output.console.extend(execution.console);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScriptSlice<'html> {
    open_tag: &'html str,
    body: &'html str,
}

fn find_scripts(html: &str) -> Vec<ScriptSlice<'_>> {
    let mut scripts = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = html[cursor..].find("<script") {
        let tag_start = cursor + relative_start;
        let Some(relative_tag_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end + 1;
        let Some(relative_close) = html[tag_end..].find("</script>") else {
            break;
        };
        let close_start = tag_end + relative_close;
        scripts.push(ScriptSlice {
            open_tag: &html[tag_start..tag_end],
            body: &html[tag_end..close_start],
        });
        cursor = close_start + "</script>".len();
    }
    scripts
}

fn script_type(open_tag: &str) -> Option<String> {
    attribute(open_tag, "type").map(|value| value.to_ascii_lowercase())
}

fn script_src(open_tag: &str) -> Option<String> {
    attribute(open_tag, "src")
}

fn attribute(open_tag: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=");
    let start = open_tag.find(&pattern)? + pattern.len();
    let value = open_tag.get(start..)?.trim_start();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_owned())
}

fn resolve_url(document_url: &str, src: &str) -> String {
    if src.starts_with('/') {
        return src.into();
    }
    let mut parts = document_url
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.pop();
    for part in src.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}
