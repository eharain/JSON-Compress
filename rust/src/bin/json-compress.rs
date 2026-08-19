//! Compresses, restores and measures JSON documents.
//!
//! It reads from a file or from stdin and writes to a file or to stdout, so it
//! drops into a pipeline without ceremony:
//!
//! ```text
//! curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
//! json-compress stats orders.json
//! ```
//!
//! Install it with `cargo install json-compress`.

use std::fs;
use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

use json_compress::{Options, Report};

const USAGE: &str = "json-compress - lossless JSON compression that is still JSON

Usage
  json-compress <command> [input] [options]

Commands
  compress [file]     Compress a JSON document
  decompress [file]   Restore a compressed document
  stats [file]        Report what compression would save, and change nothing
  minify [file]       Remove whitespace only
  beautify [file]     Re-indent
  validate [file]     Check the document parses, and say where it does not

Options
  -o, --out <file>    Write to a file instead of stdout
  -p, --pretty        Indent the output
  -i, --indent <n>    Indent width for --pretty and beautify (default 2)
      --no-strings    Do not build a string-value catalogue
      --no-tables     Do not pack arrays of objects into tables
      --no-constants  Do not hoist unchanging columns out of rows
      --no-dict-columns   Do not encode low-cardinality string columns as indices
      --min-rows <n>  Smallest array that may become a table (default 2)
  -h, --help          Show this text
  -v, --version       Show the version

With no input file, the document is read from stdin.
";

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("json-compress: {message}");
            ExitCode::FAILURE
        }
    }
}

struct Settings {
    command: String,
    input: String,
    out: String,
    pretty: bool,
    indent: usize,
    compress: Options,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            command: String::new(),
            input: String::new(),
            out: String::new(),
            pretty: false,
            indent: 2,
            compress: Options::default(),
        }
    }
}

fn execute() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    let settings = match parse_args(&argv)? {
        Some(settings) => settings,
        None => return Ok(()), // --help or --version already said its piece
    };

    if settings.command.is_empty() {
        print!("{USAGE}");
        return Ok(());
    }

    let text = read_input(&settings.input)?;
    let indent = if settings.pretty { settings.indent } else { 0 };

    match settings.command.as_str() {
        "validate" => {
            let found = json_compress::validate(&text);
            if found.valid {
                println!("valid JSON");
                return Ok(());
            }
            let where_ = if found.line > 0 {
                format!(" at line {}, column {}", found.line, found.column)
            } else {
                String::new()
            };
            Err(format!("invalid JSON{where_}: {}", found.error))
        }

        "minify" => {
            let out = json_compress::minify(&text).map_err(not_json)?;
            write_output(&out, &settings.out)
        }

        "beautify" => {
            let out = json_compress::beautify(&text, settings.indent).map_err(not_json)?;
            write_output(&out, &settings.out)
        }

        "compress" => {
            let value = json_compress::parse(&text).map_err(not_json)?;
            let envelope = json_compress::compress_with(&value, &settings.compress).map_err(say)?;
            write_output(
                &json_compress::format_json_indent(&envelope, indent),
                &settings.out,
            )
        }

        "decompress" => {
            let value = json_compress::parse(&text).map_err(not_json)?;
            if !json_compress::is_compressed(&value) {
                return Err("input is not a compressed document".to_owned());
            }
            let restored = json_compress::decompress(&value).map_err(say)?;
            write_output(
                &json_compress::format_json_indent(&restored, indent),
                &settings.out,
            )
        }

        "stats" => {
            let report =
                json_compress::measure_text_with(&text, &settings.compress).map_err(say)?;
            let name = if settings.input.is_empty() {
                "stdin".to_owned()
            } else {
                base_name(&settings.input)
            };
            println!("{}", stats(&report, &name));
            Ok(())
        }

        other => Err(format!("unknown command \"{other}\". Try --help.")),
    }
}

fn say(error: json_compress::Error) -> String {
    // The library's own messages already name the library.
    error
        .to_string()
        .trim_start_matches("json-compress: ")
        .to_owned()
}

fn not_json(error: json_compress::Error) -> String {
    format!("input is not valid JSON: {}", say(error))
}

/// Returns `None` when the argument list was answered outright, by `--help` or
/// `--version`.
fn parse_args(argv: &[String]) -> Result<Option<Settings>, String> {
    let mut settings = Settings::default();

    let mut at = 0;
    while at < argv.len() {
        let argument = argv[at].as_str();
        let mut value = |flag: &str| -> Result<String, String> {
            at += 1;
            argv.get(at)
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };

        match argument {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-v" | "--version" => {
                println!("{}", json_compress::VERSION);
                return Ok(None);
            }
            "-o" | "--out" => settings.out = value(argument)?,
            "-p" | "--pretty" => settings.pretty = true,
            "-i" | "--indent" => {
                let given = value(argument)?;
                settings.indent = given
                    .parse()
                    .map_err(|_| format!("{argument} must be a number, not \"{given}\""))?;
            }
            "--no-strings" => settings.compress.strings = false,
            "--no-tables" => settings.compress.tables = false,
            "--no-constants" => settings.compress.constants = false,
            "--no-dict-columns" => settings.compress.dict_columns = false,
            "--min-rows" => {
                let given = value(argument)?;
                settings.compress.min_rows = given
                    .parse()
                    .map_err(|_| format!("--min-rows must be a number, not \"{given}\""))?;
            }
            _ if argument.starts_with('-') && argument != "-" => {
                return Err(format!("unknown option {argument}"))
            }
            _ if settings.command.is_empty() => settings.command = argument.to_owned(),
            _ if settings.input.is_empty() => settings.input = argument.to_owned(),
            _ => return Err(format!("unexpected argument {argument}")),
        }

        at += 1;
    }

    Ok(Some(settings))
}

/// What a Windows editor, and PowerShell's own pipeline, put in front of a UTF-8
/// document. JSON has no place for it and every parser in this family refuses
/// it, so the command line drops it rather than making people wonder why their
/// file will not read.
const BYTE_ORDER_MARK: char = '\u{feff}';

fn read_input(path: &str) -> Result<String, String> {
    if !path.is_empty() {
        let data =
            fs::read(path).map_err(|error| format!("cannot read {path} ({})", reason(&error)))?;
        return Ok(strip_mark(String::from_utf8_lossy(&data).into_owned()));
    }

    // Reading from a terminal would hang waiting for something nobody is going
    // to type, so say what is missing instead.
    if std::io::stdin().is_terminal() {
        return Err("no input file given and nothing on stdin".to_owned());
    }

    let mut data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut data)
        .map_err(|error| format!("cannot read stdin ({})", reason(&error)))?;
    Ok(strip_mark(String::from_utf8_lossy(&data).into_owned()))
}

fn strip_mark(text: String) -> String {
    match text.strip_prefix(BYTE_ORDER_MARK) {
        Some(rest) => rest.to_owned(),
        None => text,
    }
}

fn reason(error: &std::io::Error) -> String {
    error.to_string()
}

fn base_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn write_output(text: &str, path: &str) -> Result<(), String> {
    if !path.is_empty() {
        return fs::write(path, text)
            .map_err(|error| format!("cannot write {path} ({})", reason(&error)));
    }
    let mut out = std::io::stdout().lock();
    writeln!(out, "{text}").map_err(|error| reason(&error))
}

fn stats(report: &Report, name: &str) -> String {
    let mut lines = vec![
        name.to_owned(),
        format!(
            "  as given          {}",
            pad(&kilobytes(report.original as i64))
        ),
        format!(
            "  minified          {}",
            pad(&kilobytes(report.minified as i64))
        ),
        format!(
            "  compressed        {}",
            pad(&kilobytes(report.compressed as i64))
        ),
        format!(
            "  saved             {}   {} than minified",
            pad(&kilobytes(report.saved)),
            change(report.percent)
        ),
    ];

    match (report.gzip_minified, report.gzip_compressed) {
        (Some(minified), Some(compressed)) => {
            let saved = minified as i64 - compressed as i64;
            let percent = if minified > 0 {
                saved as f64 / minified as f64 * 100.0
            } else {
                0.0
            };
            lines.push(String::new());
            lines.push(format!(
                "  minified + gzip   {}",
                pad(&kilobytes(minified as i64))
            ));
            lines.push(format!(
                "  compressed + gzip {}",
                pad(&kilobytes(compressed as i64))
            ));
            lines.push(format!(
                "  saved             {}   {} than minified + gzip",
                pad(&kilobytes(saved)),
                change(percent)
            ));
        }
        _ => {
            lines.push(String::new());
            lines.push(
                "  gzip figures need the gzip feature: cargo install json-compress --features gzip"
                    .to_owned(),
            );
        }
    }

    lines.join("\n")
}

fn pad(text: &str) -> String {
    format!("{text:>10}")
}

fn kilobytes(count: i64) -> String {
    if count.abs() < 1024 {
        return format!("{count} B");
    }
    if count.abs() < 1024 * 1024 {
        return format!("{:.1} KB", count as f64 / 1024.0);
    }
    format!("{:.2} MB", count as f64 / (1024.0 * 1024.0))
}

/// Reads as "31.0% smaller" or "31.0% larger" - never a negative percentage.
fn change(percent: f64) -> String {
    if percent < 0.0 {
        return format!("{:.1}% larger", -percent);
    }
    format!("{percent:.1}% smaller")
}
