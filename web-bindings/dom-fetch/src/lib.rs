#![forbid(unsafe_code)]

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsvm_interop::{HostEnvironment, InteropError, InteropValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    nodes: BTreeMap<String, Element>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn from_text_nodes<const N: usize>(nodes: [(&str, &str); N]) -> Self {
        let mut document = Self::new();
        for (selector, text) in nodes {
            document.set_text(selector, text);
        }
        document
    }

    pub fn text(&self, selector: &str) -> Option<String> {
        self.nodes.get(selector).map(|node| node.text.clone())
    }

    pub fn set_text(&mut self, selector: &str, text: &str) {
        self.nodes
            .insert(selector.into(), Element { text: text.into() });
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchService {
    origin: String,
    resources: BTreeMap<String, String>,
}

impl FetchService {
    pub fn new(origin: impl Into<String>, resources: BTreeMap<String, String>) -> Self {
        Self {
            origin: origin.into(),
            resources,
        }
    }

    pub fn text(&self, url: &str) -> Result<String, InteropError> {
        let path = self.same_origin_path(url)?;
        self.resources
            .get(&path)
            .cloned()
            .ok_or_else(|| InteropError::new(format!("fetch resource `{path}` not found")))
    }

    fn same_origin_path(&self, url: &str) -> Result<String, InteropError> {
        if url.starts_with('/') {
            return Ok(url.into());
        }
        let Some(path) = url.strip_prefix(&self.origin) else {
            return Err(InteropError::new(format!(
                "cross-origin fetch blocked for `{url}`"
            )));
        };
        if path.starts_with('/') {
            Ok(path.into())
        } else {
            Err(InteropError::new(format!("invalid fetch URL `{url}`")))
        }
    }
}

#[derive(Clone)]
pub struct BrowserBindings {
    document: Rc<RefCell<Document>>,
    fetch: Rc<FetchService>,
}

impl BrowserBindings {
    pub fn new(document: Document, fetch: FetchService) -> Self {
        Self {
            document: Rc::new(RefCell::new(document)),
            fetch: Rc::new(fetch),
        }
    }

    pub fn document(&self) -> Document {
        self.document.borrow().clone()
    }

    pub fn host_environment(&self) -> HostEnvironment {
        let document_for_text = Rc::clone(&self.document);
        let document_for_set_text = Rc::clone(&self.document);
        let fetch = Rc::clone(&self.fetch);

        HostEnvironment::new()
            .with_function("domText", move |args| {
                let selector = expect_string(args, 0, "domText selector")?;
                let text = document_for_text
                    .borrow()
                    .text(selector)
                    .unwrap_or_default();
                Ok(InteropValue::String(text))
            })
            .with_function("domSetText", move |args| {
                let selector = expect_string(args, 0, "domSetText selector")?;
                let text = expect_string(args, 1, "domSetText text")?;
                document_for_set_text.borrow_mut().set_text(selector, text);
                Ok(InteropValue::Undefined)
            })
            .with_function("fetchText", move |args| {
                let url = expect_string(args, 0, "fetchText URL")?;
                fetch.text(url).map(InteropValue::String)
            })
    }
}

fn expect_string<'value>(
    args: &'value [InteropValue],
    index: usize,
    label: &str,
) -> Result<&'value str, InteropError> {
    match args.get(index) {
        Some(InteropValue::String(value)) => Ok(value),
        _ => Err(InteropError::new(format!("expected string for {label}"))),
    }
}
