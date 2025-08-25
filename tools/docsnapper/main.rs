// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_os = "android"))]

mod headless;

use i_slint_compiler::ComponentSelection;
use slint_interpreter::ComponentHandle;

use clap::Parser;
use itertools::Itertools;

use std::{
    collections::HashMap,
    ffi::OsString,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

struct Error(Box<dyn std::error::Error>);
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the Display impl of the error instead of the error
        write!(f, "{}", self.0)
    }
}

impl<T> From<T> for Error
where
    T: Into<Box<dyn std::error::Error>> + 'static,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Include path for other .slint files or images
    #[arg(short = 'I', value_name = "include path", number_of_values = 1, action)]
    include_paths: Vec<std::path::PathBuf>,

    /// Specify Library location of the '@library' in the form 'library=/path/to/library'
    #[arg(short = 'L', value_name = "library=path", number_of_values = 1, action)]
    library_paths: Vec<String>,

    /// The .slint file to load ('-' for stdin)
    #[arg(name = "docs-folder", action)]
    docs_folder: std::path::PathBuf,

    /// The style name ('native' or 'fluent')
    #[arg(long, value_name = "style name", action)]
    style: Option<String>,

    /// Write over existing files
    #[arg(long = "overwrite", default_value = "false")]
    overwrite_files: bool,

    /// The name of the component to view. If unset, the last exported component of the file is used.
    /// If the component name is not in the .slint file , nothing will be shown
    #[arg(long, value_name = "component name", action)]
    component: Option<String>,
}

fn print_error(stream: &mut termcolor::StandardStream, msg: &str) {
    use termcolor::WriteColor;

    let _ = write!(stream, "    ");
    let _ = stream.set_color(termcolor::ColorSpec::new().set_fg(Some(termcolor::Color::Red)));
    let _ = write!(stream, "[error]");
    let _ = stream.set_color(&termcolor::ColorSpec::new());
    let _ = writeln!(stream, ": {msg}");
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let choice = if std::io::stderr().is_terminal() {
        termcolor::ColorChoice::Auto
    } else {
        termcolor::ColorChoice::Never
    };

    let mut stderr = termcolor::StandardStream::stderr(choice);

    let project_root = match find_project_root(&args.docs_folder) {
        Ok(project_path) => project_path,
        Err(e) => {
            print_error(&mut stderr, &format!("{e:?}"));
            std::process::exit(1);
        }
    };

    let _ = writeln!(&mut stderr, "project_root is {project_root:?}");

    headless::init();

    let mut error_count = 0;

    for entry in walkdir::WalkDir::new(args.docs_folder.clone()).sort_by_file_name().into_iter() {
        match &entry {
            Err(err) => {
                print_error(&mut stderr, &format!("    [error]: File error {err:?}"));
                error_count += 1;
            }
            Ok(entry) => {
                let path = entry.path();
                let ext = path.extension();

                if ext == Some(&OsString::from("md")) || ext == Some(&OsString::from("mdx")) {
                    if let Err(e) = process_doc_file(path, &project_root, &args) {
                        print_error(&mut stderr, &format!("{e:?}"));
                        error_count += 1;
                    }
                }
            }
        }
    }

    std::process::exit(error_count);
}

fn wrap_code(code: &str, size: Option<(usize, usize)>) -> String {
    let sizing_lines = if let Some((w, h)) = size {
        format!("width: {w}px;\nheight: {h}px;\n    ")
    } else {
        String::new()
    };
    format!(
        r#"export component ScreenShotThis inherits Window {{
    background: #0000;
    {sizing_lines}VerticalLayout {{
        Rectangle {{
        {code}
        }}
    }}
}}"#
    )
}

fn parse_attribute(attributes: &str) -> Result<HashMap<String, String>> {
    let mut result: HashMap<String, String> = Default::default();

    let mut key = String::new();
    let mut value = String::new();
    let mut next_is_quote = false;
    let mut escape_next = false;
    let mut quote = None;

    for c in attributes.chars() {
        if c == '=' {
            if quote.is_some() {
                value.push(c);
            } else if next_is_quote {
                return Err("Too many '=' in attribute of CodeSnippetMd tag".into());
            } else if key.is_empty() {
                return Err("Missing key before '=' in attribute of CodeSnippetMd tag".into());
            } else {
                next_is_quote = true;
                continue;
            }
        } else if c == '"' || c == '\'' {
            if escape_next {
                value.push(c);
                escape_next = false;
            } else if next_is_quote {
                quote = Some(c);
                next_is_quote = false;
            } else if quote.is_some() && Some(c) == quote {
                quote = None;
                result.insert(key, value);
                key = String::new();
                value = String::new();
            } else {
                value.push(c);
            }
        } else if c == '\\' {
            if next_is_quote {
                return Err("quote expected after = in attributes of CodeSnippetMd tag".into());
            } else if quote.is_some() {
                value.push(c);
                escape_next = true;
            } else {
                return Err("Trying to escape a character outside of a quoted string in attribute of CodeSnippetMd tag".into());
            }
        } else if c.is_whitespace() {
            if next_is_quote {
                return Err("whitespace after = in attributes of CodeSnippetMd tag".into());
            } else if quote.is_some() {
                value.push(c);
            } else if !key.is_empty() {
                result.insert(key, value);
                key = String::new();
                value = String::new();
                quote = None;
            }
        } else if next_is_quote {
            return Err("whitespace after = in attributes of CodeSnippetMd tag".into());
        } else if quote.is_some() {
            value.push(c);
        } else {
            key.push(c);
        }
    }

    Ok(result)
}

#[test]
fn test_parse_attributes() {
    let result =
        parse_attribute(r#"    foo="bar"  baz='baz' abc='test"a23"' d="test'123'xyz" hello "#)
            .unwrap();
    assert_eq!(result.len(), 5);
    assert_eq!(result.get("foo"), Some(&"bar".to_string()));
    assert_eq!(result.get("baz"), Some(&"baz".to_string()));
    assert_eq!(result.get("abc"), Some(&"test\"a23\"".to_string()));
    assert_eq!(result.get("d"), Some(&"test'123'xyz".to_string()));
    assert_eq!(result.get("hello"), Some(&String::new()));

    assert!(parse_attribute(r#"foo= "bar" "#).is_err());

    assert!(parse_attribute(r#"foo=\"bar" "#).is_err());
    assert!(parse_attribute(r#"foo=bar "#).is_err());
    assert!(parse_attribute(r#"foo=="bar" "#).is_err());
    assert!(parse_attribute(r#"   ="bar" "#).is_err());
    assert!(parse_attribute(r#"="bar" "#).is_err());
}

fn extract_code_from_text(text: &str, size: Option<(usize, usize)>) -> Result<String> {
    let without_leading_backticks = text.trim_start_matches('`');
    let number_of_backticks = text.len() - without_leading_backticks.len();
    if number_of_backticks < 3 {
        return Err(
            "text in CodeSnippetMD tag does not start with enough backticks to be a code block"
                .into(),
        );
    }
    let without_leading = without_leading_backticks.trim_start();
    let Some(without_leading) = without_leading.strip_prefix("slint") else {
        return Err("text in CodeSnippetMD tag is not a slint code block".into());
    };

    if !without_leading.starts_with(' ') && !without_leading.starts_with('\n') {
        return Err("text in CodeSnippetMD tag is not a slint code block".into());
    }

    let Some(first_line_end) = without_leading.find('\n') else {
        return Err("text in CodeSnippetMD tag is one line only, so not a proper code block".into());
    };

    let code = &without_leading[first_line_end..];

    let backticks = {
        let mut tmp = String::new();
        for _i in 0..number_of_backticks {
            tmp.push('`');
        }
        tmp
    };

    let Some(code) = code.strip_suffix(&backticks) else {
        return Err(
            "text in CodeSnippetMD tag does not end with the expected number of backticks".into()
        );
    };

    let code = if code.contains("component") { code.to_string() } else { wrap_code(code, size) };

    Ok(code)
}

#[test]
fn test_extract_code_from_text() {
    assert_eq!(
        extract_code_from_text(
            r#"```slint foo=bar
```"#,
            Some((100, 200))
        )
        .unwrap(),
        wrap_code("\n", Some((100, 200)))
    );
    assert_eq!(
        extract_code_from_text(
            r#"```````````slint foo=bar
```````````"#,
            None
        )
        .unwrap(),
        wrap_code("\n", None)
    );
    assert_eq!(
        extract_code_from_text(
            r#"```  slint foo=bar
    ```"#,
            None
        )
        .unwrap(),
        wrap_code("\n    ", None)
    );
    assert!(extract_code_from_text(
        r#"```````````slint foo=bar
``````````"#,
        None
    )
    .is_err());
    assert!(extract_code_from_text(
        r#"``slint foo=bar
``"#,
        None
    )
    .is_err());
    assert!(extract_code_from_text(
        r#"```slintfoo
```"#,
        None
    )
    .is_err());
    assert!(extract_code_from_text(
        r#"Some Text
```slint foo
```"#,
        None
    )
    .is_err());
    assert!(extract_code_from_text(
        r#"```slint foo
```
Some text"#,
        None
    )
    .is_err());
}

fn process_tag(
    attributes: &str,
    text: &str,
    file_path: &Path,
    project_root: &Path,
    args: &Cli,
) -> Result<()> {
    let attr = parse_attribute(attributes)?;
    if attr.contains_key("noScreenShot") {
        return Ok(());
    }

    let Some(path) = attr.get("imagePath") else {
        // No image path, no need to save anything...
        return Ok(());
    };

    let screenshot_path = if let Some(p) = path.strip_prefix('/') {
        project_root.join(p).to_path_buf()
    } else {
        let Some(current_dir) = file_path.parent() else {
            return Err(format!("Could not find directory containing {file_path:?}").into());
        };
        current_dir.join(path)
    };

    let width = attr.get("imageWidth").and_then(|w| w.parse::<usize>().ok());
    let height = attr.get("imageHeight").and_then(|h| h.parse::<usize>().ok());

    let size = width.and_then(|w| height.map(|h| (w, h)));

    let scale_factor = attr.get("scale").and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);

    let code = extract_code_from_text(text, size)?;

    build_and_snapshot(args, size, scale_factor, file_path, code.to_string(), &screenshot_path)
}

fn process_doc_file(file: &Path, project_root: &Path, args: &Cli) -> Result<()> {
    let file = file.canonicalize()?;
    eprintln!("Looking into {file:?}");
    let content = std::fs::read_to_string(&file)?;

    let mut content = &content[..];
    let tag_start = "<CodeSnippetMD";
    let tag_end = "</CodeSnippetMD>";
    while let Some(position) = content.find(tag_start) {
        let mut start_offset = position + tag_start.len();
        if let Some(tag_content_end_pos) = content[start_offset..].find('>') {
            let tag_content = &content[start_offset..start_offset + tag_content_end_pos];
            start_offset += tag_content_end_pos + 1;

            let tag_content = tag_content.trim();

            if !tag_content.ends_with('/') {
                // We need an end_tag...
                if let Some(end_tag_pos) = content[start_offset..].find(tag_end) {
                    let text = &content[start_offset..start_offset + end_tag_pos].trim();
                    start_offset += end_tag_pos + 1;
                    process_tag(tag_content, text, &file, project_root, args)?;
                } else {
                    return Err(
                        "No </CodeSnippetMD> tag found after having seen an opening tag".into()
                    );
                }
            }
        }
        content = &content[start_offset..];
    }
    Ok(())
}

fn find_project_root(docs_folder: &Path) -> Result<PathBuf> {
    let mut path = Some(docs_folder.canonicalize()?);

    while let Some(d) = path {
        if d.join("astro.config.mjs").exists() || d.join("astro.config.ts").exists() {
            return Ok(d);
        }
        path = d.parent().map(|p| p.to_path_buf());
    }

    Err(format!("No project root found for doc_folder {docs_folder:?}").into())
}

fn build_and_snapshot(
    args: &Cli,
    size: Option<(usize, usize)>,
    scale_factor: f32,
    doc_file_path: &Path,
    source: String,
    screenshot_path: &Path,
) -> Result<()> {
    let compiler = init_compiler(args);
    let r = spin_on::spin_on(compiler.build_from_source(source, doc_file_path.to_path_buf()));
    r.print_diagnostics();
    if r.has_errors() {
        return Err("Compile error".into());
    }
    let Some(c) = r.components().next() else {
        match &args.component {
            Some(name) => {
                eprintln!("Component '{name}' not found in file '{doc_file_path:?}'");
            }
            None => {
                eprintln!("No component found in file '{doc_file_path:?}'");
            }
        }
        return Err("Component error".into());
    };

    let component = c.create()?;

    // FIXME: The scale factor needs to be set before the size is set!
    headless::set_window_scale_factor(component.window(), scale_factor);

    if let Some((x, y)) = size {
        component.window().set_size(i_slint_core::api::LogicalSize::new(x as f32, y as f32));
    } else {
        component.window().set_size(i_slint_core::api::LogicalSize::new(200.0, 200.0));
    }

    component.show()?;

    let screen_dump = component.window().take_snapshot()?;

    {
        if let Some(dir) = screenshot_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
    }

    let overwrite_tag = if screenshot_path.exists() {
        if !args.overwrite_files {
            return Err(format!("{screenshot_path:?} already exists, aborting").into());
        }
        " [overwrite]"
    } else {
        ""
    };

    let scale_factor = component.window().scale_factor();
    let scale_str = if (0.99..=1.01).contains(&scale_factor) {
        String::new()
    } else {
        format!("@{scale_factor}x")
    };

    eprintln!(
        "    Saving image with {}x{}{scale_str} pixels to {screenshot_path:?}{overwrite_tag}",
        screen_dump.width(),
        screen_dump.height()
    );

    image::save_buffer(
        screenshot_path,
        screen_dump.as_bytes(),
        screen_dump.width(),
        screen_dump.height(),
        image::ColorType::Rgba8,
    )?;

    Ok(())
}

fn init_compiler(args: &Cli) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();
    compiler.set_include_paths(args.include_paths.clone());
    compiler.set_library_paths(
        args.library_paths
            .iter()
            .filter_map(|entry| entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into())))
            .collect(),
    );
    if let Some(style) = &args.style {
        compiler.set_style(style.clone());
    }

    compiler.compiler_configuration(i_slint_core::InternalToken).components_to_generate =
        match &args.component {
            Some(component) => ComponentSelection::Named(component.clone()),
            None => ComponentSelection::LastExported,
        };

    compiler
}
