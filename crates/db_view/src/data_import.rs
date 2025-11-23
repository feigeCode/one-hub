//! Data import utilities for SQL editor example.
//!
//! Supports importing data from common formats into a tabular structure
//! that can be previewed or converted to SQL INSERT statements using
//! `data_export` helpers if desired.
//!
//! Covered formats:
//! - CSV (RFC 4180 style quoting)
//! - JSON (array of objects/arrays, or NDJSON)
//! - SQL (raw script; no parsing/validation performed)
//!
//! This module is standalone and not wired into `lib` by default.
//! To use it later, add `mod data_import;` and call the functions below.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use db::QueryResult;

/// Import source format.
#[derive(Debug, Clone)]
pub enum ImportFormat {
    Csv(CsvImportOptions),
    Json(JsonImportOptions),
    Sql,
}

#[derive(Debug, Clone)]
pub struct CsvImportOptions {
    pub delimiter: char,
    /// Whether the first row contains column headers.
    pub has_headers: bool,
    /// Trim whitespace around unquoted fields.
    pub trim_fields: bool,
}

impl Default for CsvImportOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            has_headers: true,
            trim_fields: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonImportOptions {
    /// When top-level is array of objects, choose keys from:
    /// - First object only (stable columns), or
    /// - Union of all keys (fill missing with empty string)
    pub key_mode: JsonKeyMode,
}

#[derive(Debug, Clone, Copy)]
pub enum JsonKeyMode {
    FirstObject,
    UnionAll,
}

impl Default for JsonImportOptions {
    fn default() -> Self {
        Self { key_mode: JsonKeyMode::FirstObject }
    }
}

/// Result of an import operation.
#[derive(Debug, Clone)]
pub enum ImportData {
    /// Tabular data ready to preview or convert to SQL INSERTs.
    Table(QueryResult),
    /// Raw SQL script (multiple statements allowed).
    SqlScript(String),
}

/// Import from a file path using the selected format.
pub fn import_from_path(path: impl AsRef<Path>, format: ImportFormat) -> Result<ImportData> {
    let p = path.as_ref();
    let bytes = fs::read(p)?;
    import_from_bytes(&bytes, format)
}

/// Import from raw bytes using the selected format. Assumes UTF-8 for text.
pub fn import_from_bytes(bytes: &[u8], format: ImportFormat) -> Result<ImportData> {
    match format {
        ImportFormat::Csv(opts) => {
            let text = String::from_utf8_lossy(bytes);
            let table = parse_csv(&text, &opts)?;
            Ok(ImportData::Table(table))
        }
        ImportFormat::Json(opts) => {
            let text = String::from_utf8_lossy(bytes);
            let table = parse_json(&text, &opts)?;
            Ok(ImportData::Table(table))
        }
        ImportFormat::Sql => {
            let text = String::from_utf8_lossy(bytes).to_string();
            Ok(ImportData::SqlScript(text))
        }
    }
}

/// Convenience: guess import format from filename extension and return defaults.
pub fn guess_format_from_path(path: impl AsRef<Path>) -> Option<ImportFormat> {
    let ext = path
        .as_ref()
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("csv") => Some(ImportFormat::Csv(CsvImportOptions::default())),
        Some("json") | Some("jsonl") | Some("ndjson") => {
            Some(ImportFormat::Json(JsonImportOptions::default()))
        }
        Some("sql") => Some(ImportFormat::Sql),
        _ => None,
    }
}

// === CSV ===

fn parse_csv(text: &str, opts: &CsvImportOptions) -> Result<QueryResult> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in CsvLineIter::new(text) {
        let rec = parse_csv_record(&line, opts.delimiter, opts.trim_fields)?;
        rows.push(rec);
    }

    if rows.is_empty() {
        return Ok(QueryResult::empty());
    }

    // Determine headers and data rows
    let (headers, data_rows) = if opts.has_headers {
        let headers = normalize_headers(&rows[0]);
        (headers, rows.into_iter().skip(1).collect::<Vec<_>>())
    } else {
        let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let headers = (0..max_cols).map(|i| format!("col_{}", i + 1)).collect::<Vec<_>>();
        (headers, rows)
    };

    // Normalize row lengths by padding with empty strings.
    let col_count = headers.len();
    let data_rows = data_rows
        .into_iter()
        .map(|mut r| {
            if r.len() < col_count {
                r.resize(col_count, String::new());
            }
            r
        })
        .collect::<Vec<_>>();

    Ok(QueryResult {
        headers,
        rows: data_rows,
        message: None,
    })
}

fn normalize_headers(hs: &[String]) -> Vec<String> {
    hs.iter()
        .enumerate()
        .map(|(i, h)| {
            let h = h.trim();
            if h.is_empty() { format!("col_{}", i + 1) } else { h.to_string() }
        })
        .collect()
}

/// Iterate logical CSV lines, preserving embedded newlines in quoted fields by
/// concatenating physical lines until quotes are balanced. This is a minimal,
/// pragmatic approach and not a full CSV state machine; good enough for common cases.
struct CsvLineIter<'a> {
    lines: std::str::Lines<'a>,
    carry: Option<String>,
}

impl<'a> CsvLineIter<'a> {
    fn new(text: &'a str) -> Self { Self { lines: text.lines(), carry: None } }
}

impl<'a> Iterator for CsvLineIter<'a> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        let mut current = self.carry.take().unwrap_or_default();
        while let Some(phys) = self.lines.next() {
            if !current.is_empty() { current.push('\n'); }
            current.push_str(phys);
            if balanced_quotes(&current) { return Some(current); }
        }
        if !current.is_empty() { Some(current) } else { None }
    }
}

fn balanced_quotes(s: &str) -> bool {
    let mut in_quote = false;
    let mut i = 0;
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] == b'"' {
            // count consecutive quotes
            let mut q = 1usize;
            let mut j = i + 1;
            while j < b.len() && b[j] == b'"' { q += 1; j += 1; }
            if q % 2 == 1 { in_quote = !in_quote; }
            i = j;
        } else {
            i += 1;
        }
    }
    !in_quote
}

fn parse_csv_record(line: &str, delimiter: char, trim_fields: bool) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' => {
                    if matches!(chars.peek(), Some('"')) { // escaped quote
                        field.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                }
                _ => field.push(ch),
            }
        } else {
            match ch {
                '"' => in_quotes = true,
                c if c == delimiter => {
                    out.push(if trim_fields { field.trim().to_string() } else { field.clone() });
                    field.clear();
                }
                _ => field.push(ch),
            }
        }
    }

    out.push(if trim_fields { field.trim().to_string() } else { field });
    Ok(out)
}

// === JSON ===

fn parse_json(text: &str, opts: &JsonImportOptions) -> Result<QueryResult> {
    // Try direct JSON first
    let mut value = match serde_json::from_str::<Value>(text) {
        Ok(v) => v,
        Err(_) => {
            // Try NDJSON (one JSON document per line)
            let mut arr = Vec::<Value>::new();
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                let v: Value = serde_json::from_str(line)
                    .map_err(|e| anyhow!("Invalid JSON/NDJSON: {}", e))?;
                arr.push(v);
            }
            Value::Array(arr)
        }
    };

    match &mut value {
        Value::Array(items) => parse_json_array(items, opts),
        Value::Object(obj) => parse_single_object(obj),
        _ => Err(anyhow!("Unsupported JSON structure: expected array or object")),
    }
}

fn parse_json_array(items: &mut [Value], opts: &JsonImportOptions) -> Result<QueryResult> {
    if items.is_empty() { return Ok(QueryResult::empty()); }

    // If array of objects
    if items.iter().all(|v| v.is_object()) {
        let headers = match opts.key_mode {
            JsonKeyMode::FirstObject => items[0]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            JsonKeyMode::UnionAll => {
                let mut order: Vec<String> = Vec::new();
                let mut seen = std::collections::HashSet::<String>::new();
                for v in items.iter() {
                    for k in v.as_object().unwrap().keys() {
                        if seen.insert(k.clone()) { order.push(k.clone()); }
                    }
                }
                order
            }
        };

        let mut rows = Vec::with_capacity(items.len());
        for v in items.iter() {
            let obj = v.as_object().unwrap();
            let row = headers
                .iter()
                .map(|k| obj.get(k).map(json_to_string).unwrap_or_default())
                .collect::<Vec<_>>();
            rows.push(row);
        }

        return Ok(QueryResult { headers, rows, message: None });
    }

    // If array of arrays
    if items.iter().all(|v| v.is_array()) {
        let max_cols = items
            .iter()
            .map(|v| v.as_array().unwrap().len())
            .max()
            .unwrap_or(0);
        let headers = (0..max_cols).map(|i| format!("col_{}", i + 1)).collect::<Vec<_>>();
        let mut rows = Vec::with_capacity(items.len());
        for v in items.iter() {
            let arr = v.as_array().unwrap();
            let mut row = Vec::with_capacity(max_cols);
            for i in 0..max_cols {
                row.push(arr.get(i).map(json_to_string).unwrap_or_default());
            }
            rows.push(row);
        }
        return Ok(QueryResult { headers, rows, message: None });
    }

    // Mixed types: stringify each item into a single column
    let headers = vec!["value".to_string()];
    let rows = items
        .iter()
        .map(|v| vec![json_to_string(v)])
        .collect::<Vec<_>>();
    Ok(QueryResult { headers, rows, message: None })
}

fn parse_single_object(obj: &serde_json::Map<String, Value>) -> Result<QueryResult> {
    let headers = obj.keys().cloned().collect::<Vec<_>>();
    let row = headers
        .iter()
        .map(|k| obj.get(k).map(json_to_string).unwrap_or_default())
        .collect::<Vec<_>>();
    Ok(QueryResult { headers, rows: vec![row], message: None })
}

fn json_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}

/// Utility: map an `ImportData::Table` to SQL INSERT statements using
/// the same rules as export's `to_sql_inserts`. If input is a SQL script,
/// returns it unchanged.
pub fn to_sql_insert_script(import: ImportData, table: &str, null_when_empty: bool) -> Result<String> {
    match import {
        ImportData::SqlScript(sql) => Ok(sql),
        ImportData::Table(qr) => {
            // Reuse the export module for identical SQL rendering, if available.
            // Fall back to a minimal inline implementation if not linked.
            Ok(render_inserts_inline(&qr, table, null_when_empty))
        }
    }
}

fn render_inserts_inline(result: &QueryResult, table: &str, null_when_empty: bool) -> String {
    fn ident(id: &str) -> String {
        if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            id.to_string()
        } else {
            format!("`{}`", id.replace('`', "``"))
        }
    }
    fn val(v: &str, null_when_empty: bool) -> String {
        if null_when_empty && v.is_empty() { return "NULL".to_string(); }
        format!("'{}'", v.replace('\'', "''"))
    }

    let t = ident(table);
    let mut out = String::new();
    if !result.headers.is_empty() {
        let cols = result.headers.iter().map(|h| ident(h)).collect::<Vec<_>>().join(", ");
        for row in &result.rows {
            let values = row.iter().map(|v| val(v, null_when_empty)).collect::<Vec<_>>().join(", ");
            out.push_str(&format!("INSERT INTO {} ({}) VALUES ({});\n", t, cols, values));
        }
    } else {
        for row in &result.rows {
            let values = row.iter().map(|v| val(v, null_when_empty)).collect::<Vec<_>>().join(", ");
            out.push_str(&format!("INSERT INTO {} VALUES ({});\n", t, values));
        }
    }
    out
}

