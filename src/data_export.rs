//! Data export utilities for SQL editor example.
//!
//! Provides helpers to export `QueryResult` into multiple formats:
//! - CSV
//! - SQL (INSERT statements)
//! - Markdown table
//! - Excel (HTML table or Excel 2003 XML SpreadsheetML)
//! - Word (RTF table)
//!
//! This module is standalone and not wired into `main.rs` by default.
//! To use it later, add `mod data_export;` and call the functions below.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

/// Bring in the query result type from the example.
/// When you add `mod data_export;` to main, this path resolves.
use db::QueryResult;

/// Export target format.
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Csv(CsvOptions),
    Sql(SqlOptions),
    Markdown,
    /// Excel HTML table (save with .xls extension). Widely supported by Excel.
    ExcelHtml,
    /// Excel 2003 XML (SpreadsheetML). Save with .xml or .xls.
    ExcelXml,
    /// Word RTF document containing a table.
    WordRtf,
    /// JSON array of objects: each row is an object keyed by headers.
    /// Example: [{"col1": "v1", "col2": "v2"}, ...]
    Json,
}

#[derive(Debug, Clone)]
pub struct CsvOptions {
    pub delimiter: char,
    pub include_headers: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            include_headers: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqlOptions {
    /// Table name used in INSERT statements. If `None`, uses `export_table`.
    pub table: Option<String>,
    /// Treat empty strings as NULL.
    pub null_when_empty: bool,
}

impl Default for SqlOptions {
    fn default() -> Self {
        Self {
            table: None,
            null_when_empty: false,
        }
    }
}

/// Export the given result into the selected format and write to `path`.
pub fn export_to_path(result: &QueryResult, format: ExportFormat, path: impl AsRef<Path>) -> Result<()> {
    let bytes = export_to_bytes(result, format)?;
    let p = path.as_ref();
    if let Some(dir) = p.parent() {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
    }
    let mut file = fs::File::create(p)?;
    file.write_all(&bytes)?;
    Ok(())
}

/// Export the given result into the selected format and return UTF-8 bytes.
pub fn export_to_bytes(result: &QueryResult, format: ExportFormat) -> Result<Vec<u8>> {
    match format {
        ExportFormat::Csv(opts) => Ok(to_csv(result, &opts).into_bytes()),
        ExportFormat::Sql(opts) => Ok(to_sql_inserts(result, &opts).into_bytes()),
        ExportFormat::Markdown => Ok(to_markdown_table(result).into_bytes()),
        ExportFormat::ExcelHtml => Ok(to_excel_html(result).into_bytes()),
        ExportFormat::ExcelXml => Ok(to_excel_xml(result).into_bytes()),
        ExportFormat::WordRtf => Ok(to_word_rtf(result).into_bytes()),
        ExportFormat::Json => Ok(to_json(result).into_bytes()),
    }
}

fn to_csv(result: &QueryResult, opts: &CsvOptions) -> String {
    let mut out = String::new();
    if opts.include_headers && !result.headers.is_empty() {
        out.push_str(&join_csv_row(&result.headers, opts.delimiter));
        out.push('\n');
    }
    for row in &result.rows {
        out.push_str(&join_csv_row(row, opts.delimiter));
        out.push('\n');
    }
    out
}

fn join_csv_row(cols: &[String], delimiter: char) -> String {
    let mut parts = Vec::with_capacity(cols.len());
    for c in cols {
        parts.push(escape_csv_field(c, delimiter));
    }
    parts.join(&delimiter.to_string())
}

fn escape_csv_field(s: &str, delimiter: char) -> String {
    let must_quote = s.contains(delimiter) || s.contains('\n') || s.contains('\r') || s.contains('"');
    // Double quotes per RFC 4180: '"' -> '""'
    let mut v = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '"' {
            v.push('"');
            v.push('"');
        } else {
            v.push(ch);
        }
    }
    if must_quote {
        let mut quoted = String::with_capacity(v.len() + 2);
        quoted.push('"');
        quoted.push_str(&v);
        quoted.push('"');
        v = quoted;
    }
    v
}

fn to_sql_inserts(result: &QueryResult, opts: &SqlOptions) -> String {
    let table = opts.table.clone().unwrap_or_else(|| "export_table".to_string());
    let mut out = String::new();
    if !result.headers.is_empty() {
        let cols = result
            .headers
            .iter()
            .map(|h| format_identifier(h))
            .collect::<Vec<_>>()
            .join(", ");
        for row in &result.rows {
            let values = row
                .iter()
                .map(|v| sql_value(v, opts.null_when_empty))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("INSERT INTO {} ({}) VALUES ({});\n", format_identifier(&table), cols, values));
        }
    } else {
        // No headers: simple positional inserts
        for row in &result.rows {
            let values = row
                .iter()
                .map(|v| sql_value(v, opts.null_when_empty))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("INSERT INTO {} VALUES ({});\n", format_identifier(&table), values));
        }
    }
    out
}

fn format_identifier(id: &str) -> String {
    // Quote identifiers simply with backticks for broad compatibility (MySQL-like).
    // Adjust as needed for specific dialects.
    if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' ) {
        id.to_string()
    } else {
        format!("`{}`", id.replace('`', "``"))
    }
}

fn sql_value(v: &str, null_when_empty: bool) -> String {
    if null_when_empty && v.is_empty() {
        "NULL".to_string()
    } else {
        // Escape single quotes by doubling them for broad SQL compatibility.
        let escaped = v.replace('\'', "''");
        format!("'{}'", escaped)
    }
}

fn to_markdown_table(result: &QueryResult) -> String {
    // If there are no headers, synthesize column names.
    let headers = if result.headers.is_empty() {
        let max_cols = result.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        (0..max_cols).map(|i| format!("col_{}", i + 1)).collect::<Vec<_>>()
    } else {
        result.headers.clone()
    };

    let mut out = String::new();
    out.push('|');
    out.push_str(&headers.iter().map(|h| escape_md(h)).collect::<Vec<_>>().join(" | "));
    out.push_str("|\n");
    out.push('|');
    out.push_str(&headers.iter().map(|_| "---".to_string()).collect::<Vec<_>>().join(" | "));
    out.push_str("|\n");
    for row in &result.rows {
        out.push('|');
        out.push_str(&row.iter().map(|c| escape_md(c)).collect::<Vec<_>>().join(" | "));
        out.push_str("|\n");
    }
    out
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}

fn to_excel_html(result: &QueryResult) -> String {
    // HTML table that Excel can open as .xls
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n");
    out.push_str("<html><head><meta charset=\"utf-8\"><title>Export</title></head><body>\n");
    out.push_str("<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n");
    if !result.headers.is_empty() {
        out.push_str("<thead><tr>");
        for h in &result.headers {
            out.push_str("<th>");
            out.push_str(&html_escape(h));
            out.push_str("</th>");
        }
        out.push_str("</tr></thead>\n");
    }
    out.push_str("<tbody>\n");
    for row in &result.rows {
        out.push_str("<tr>");
        for c in row {
            out.push_str("<td>");
            out.push_str(&html_escape(c));
            out.push_str("</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table>\n");
    out.push_str("</body></html>\n");
    out
}

fn to_excel_xml(result: &QueryResult) -> String {
    // Excel 2003 XML SpreadsheetML
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\"?>\n");
    out.push_str("<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" ");
    out.push_str("xmlns:o=\"urn:schemas-microsoft-com:office:office\" ");
    out.push_str("xmlns:x=\"urn:schemas-microsoft-com:office:excel\" ");
    out.push_str("xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\">\n");
    out.push_str("  <Worksheet ss:Name=\"Export\">\n");
    out.push_str("    <Table>\n");
    if !result.headers.is_empty() {
        out.push_str("      <Row>\n");
        for h in &result.headers {
            out.push_str("        <Cell><Data ss:Type=\"String\">");
            out.push_str(&xml_escape(h));
            out.push_str("</Data></Cell>\n");
        }
        out.push_str("      </Row>\n");
    }
    for row in &result.rows {
        out.push_str("      <Row>\n");
        for c in row {
            out.push_str("        <Cell><Data ss:Type=\"String\">");
            out.push_str(&xml_escape(c));
            out.push_str("</Data></Cell>\n");
        }
        out.push_str("      </Row>\n");
    }
    out.push_str("    </Table>\n");
    out.push_str("  </Worksheet>\n");
    out.push_str("</Workbook>\n");
    out
}

fn to_word_rtf(result: &QueryResult) -> String {
    // Minimal RTF with a table. Word will render this as a table.
    // Cell widths are simplistic; adjust if needed.
    let mut out = String::new();
    out.push_str("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Arial;}}\n");
    out.push_str("\\fs20\n");

    let col_count = if !result.headers.is_empty() {
        result.headers.len()
    } else {
        result.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    };
    let cell_width_step = 2000; // twips

    if !result.headers.is_empty() {
        out.push_str("\\trowd\\trgaph108\\trleft0");
        for i in 0..col_count {
            let x = cell_width_step * (i as i32 + 1);
            out.push_str(&format!("\\cellx{}", x));
        }
        out.push('\n');
        for h in &result.headers {
            out.push_str("\\intbl ");
            out.push_str(&rtf_escape(h));
            out.push_str("\\cell");
        }
        out.push_str("\\row\n");
    }
    for row in &result.rows {
        out.push_str("\\trowd\\trgaph108\\trleft0");
        for i in 0..col_count {
            let x = cell_width_step * (i as i32 + 1);
            out.push_str(&format!("\\cellx{}", x));
        }
        out.push('\n');
        for c in row {
            out.push_str("\\intbl ");
            out.push_str(&rtf_escape(c));
            out.push_str("\\cell");
        }
        // fill missing cells
        for _ in row.len()..col_count {
            out.push_str("\\intbl \\cell");
        }
        out.push_str("\\row\n");
    }

    out.push_str("}\n");
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_escape(s: &str) -> String { html_escape(s) }

fn rtf_escape(s: &str) -> String {
    // Escape backslashes, braces, and newlines.
    s.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}").replace('\n', "\\par ")
}

/// Convenience helper to choose a typical file extension for a format.
pub fn suggested_extension(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv(_) => "csv",
        ExportFormat::Sql(_) => "sql",
        ExportFormat::Markdown => "md",
        ExportFormat::ExcelHtml => "xls",
        ExportFormat::ExcelXml => "xml",
        ExportFormat::WordRtf => "rtf",
        ExportFormat::Json => "json",
    }
}

/// Validate that the result appears tabular.
pub fn validate_tabular(result: &QueryResult) -> Result<()> {
    if result.headers.is_empty() && result.rows.is_empty() && result.message.is_some() {
        return Err(anyhow!("Result contains only a message; no table to export"));
    }
    Ok(())
}

fn to_json(result: &QueryResult) -> String {
    // Compressed JSON array of objects, keys from headers.
    let headers: Vec<String> = if result.headers.is_empty() {
        let max_cols = result.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        (0..max_cols).map(|i| format!("col_{}", i + 1)).collect()
    } else {
        result.headers.clone()
    };

    let mut arr: Vec<Value> = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        let mut obj = serde_json::Map::with_capacity(headers.len());
        for (i, h) in headers.iter().enumerate() {
            let val = row.get(i).map(|s| Value::String(s.clone())).unwrap_or(Value::Null);
            obj.insert(h.clone(), val);
        }
        arr.push(Value::Object(obj));
    }
    serde_json::to_string(&Value::Array(arr)).unwrap_or_else(|_| "[]".to_string())
}
