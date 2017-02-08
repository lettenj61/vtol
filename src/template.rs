use std::collections::HashMap;
use std::convert::From;
use std::io::{self, Write};
use std::path::Path;

use toml::{Table, Value};
use super::format::{self, Formatter};
use super::fsutils;
use super::parser;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Style {
    Tera,
    ST,
    Path,
}

impl Default for Style {
    fn default() -> Style {
        Style::Tera
    }
}

impl Style {
    fn arg_sep(&self) -> char {
        match self {
            &Style::Tera => '|',
            &Style::ST => ',',
            &Style::Path => '_',
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Placeholder {
    name: String,
    args: Vec<Formatter>,
}

impl Placeholder {
    pub fn new(name: &str, arg_expr: Option<String>, style: Style) -> Placeholder {
        let sep = style.arg_sep();
        Placeholder {
            name: name.into(),
            args: arg_expr.map(|expr| {
                expr.split(sep)
                    .map(|s| Formatter::from(s.as_ref()))
                    .filter(|f| *f != Formatter::Ident)
                    .collect()
            })
            .unwrap_or(Vec::new()),
        }
    }

    pub fn no_format(name: &str) -> Placeholder {
        Placeholder::new(name, None, Style::ST)
    }

    /// Apply formatting on the placeholder with given context, and returns formatted `String`.
    pub fn format_with(&self, params: &HashMap<String, String>) -> String {
        if let Some(v) = params.get(&self.name) {
            self.args.iter().fold(v.clone(), |ref s, f| format::format(&s, *f))
        } else {
            self.name.clone()
        }
    }
}

/// Minimal template for any kind of plain text.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pub style: Style,
    pub body: String,
}

impl Template {
    /// Create `Template` object from given `str`.
    pub fn read_str<S: AsRef<str>>(style: Style, template: S) -> Template {
        Template {
            style: style,
            body: String::from(template.as_ref()),
        }
    }

    /// Create `Template` from contents of the file at given `Path`.
    pub fn read_file<P: AsRef<Path>>(style: Style, src: P) -> Result<Template, io::Error> {
        fsutils::read_file(src.as_ref()).map(|s| Template::read_str(style, s))
    }

    /// Utility to create giter8 style template instantly.
    pub fn new_g8<S: AsRef<str>>(template: S) -> Template {
        Template::read_str(Style::ST, template)
    }

    /// Process template with given `params`, and write result into `writer`.
    pub fn write_to<'a, W: Write>(&mut self,
                                 writer: &'a mut W,
                                 params: &HashMap<String, String>)
                                 -> Result<&'a mut W, io::Error> {

        let mut progress = parser::parse_template(self.body.as_ref(), &self.style);
        while let Ok((raw, maybe_ph, rest)) = progress {

            if !raw.is_empty() {
                writer.write(raw.as_bytes()).unwrap();
            }
    
            if let Some(ph) = maybe_ph {
                let value = ph.format_with(&params);
                writer.write(value.as_bytes()).unwrap();
            }
    
            if rest.is_empty() {
                writer.flush().unwrap();
                break;
            } else {
                progress = parser::parse_template(rest, &self.style);
            }
        }
        writer.flush().unwrap();

        Ok(writer)
    }

    /// Create template from given `str`, and instantly write it.
    pub fn write_once<'a, S, W>(writer: &'a mut W,
                                style: Style,
                                template: S,
                                params: &HashMap<String, String>)
                                -> Result<&'a mut W, io::Error>
        where S: AsRef<str>,
              W: Write
    {
        let mut template = Template::read_str(style, template);
        Template::write_to(&mut template, writer, params)
    }
}

/// Wrapper arround map-type collection to use as resolved parameters in project generation.
#[derive(Debug, Clone)]
pub struct Params {
    pub param_map: HashMap<String, String>,
}

impl Params {
    pub fn from_map(map: HashMap<String, String>) -> Params {
        Params { param_map: map }
    }

    pub fn convert_toml(toml: &Table) -> Params {
        let mut raw_values = HashMap::new();
        for (k, tv) in toml {
            if let Some(v) = convert(tv) {
                raw_values.insert(k.clone(), v);
            }
        }
        Params { param_map: raw_values }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.param_map.get(key)
    }
}

// FIXME: should return `Result<String, errors::Error>` to tell we won't accept table / array?
fn convert(value: &Value) -> Option<String> {
    match *value {
        Value::String(_) => value.as_str().map(|s| s.to_owned()),
        Value::Datetime(_) => value.as_datetime().map(|s| s.to_owned()),
        Value::Integer(_) => value.as_integer().map(|i| i.to_string()),
        Value::Float(_) => value.as_float().map(|f| f.to_string()),
        Value::Boolean(_) => value.as_bool().map(|b| b.to_string()),
        _ => None,
    }
}
