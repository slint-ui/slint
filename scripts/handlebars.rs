#!/usr/bin/env -S cargo +nightly -Zscript -q
---
[package]
edition = "2024"

[profile.dev]
debug = false
opt-level = 3
strip = "symbols"

[dependencies]
clap = { version = "4.6", features = ["derive"] }
serde = "1.0"
serde_json = "1.0"
serde_yaml = { package = "yaml_serde", version = "0.10" }
toml = "1.1.3"
handlebars = "6.4"
---

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read},
    path::Path,
    sync::{LazyLock, Mutex},
};

use clap::Parser;
use handlebars::{Handlebars, Template, handlebars_helper};
use serde_json::{Map, Value, map::Entry};

#[derive(Debug, Parser)]
struct Args {
    /// `-v foo=bar`, can be specified multiple times
    ///
    /// The left-hand side can be a path such as `foo.bar.baz`, in which case objects
    /// will be recursively created to allow the path to be specified. If a prefix of
    /// the path points to an already-existing value which is not an object, then an
    /// error will be emitted.
    ///
    /// If the right-hand side of the `=` can be interpreted as a JSON value
    /// (e.g. true, false, 123, 123.456, { a: 1, b: 2 }) then it will be interpreted
    /// as one. Otherwise, it will be interpreted as a string. For example, `-v a=b`
    /// will create the object { a: "b" }.
    #[arg(long = "value", short)]
    values: Vec<String>,

    /// A file containing a value to pass to the template
    ///
    /// Can be combined with `-v/--value`, which will let you override (or add to) the
    /// object in this file.
    ///
    /// The value file can be in JSON, YAML or TOML format, determined by the file extension.
    /// If the file extension is not understood, then JSON will be used as the default.
    ///
    /// If unspecified or `-`, will use stdin
    #[arg(long, short = 'f')]
    value_file: Option<String>,

    /// Input `.hbs` template
    ///
    /// If unspecified or `-`, will use stdin
    #[arg(short, long)]
    input: Option<String>,

    /// Output file to emit
    ///
    /// If unspecified or `-`, will use stdout
    #[arg(short, long)]
    output: Option<String>,
}

enum ValueFormat {
    Json,
    Yaml,
    Toml,
}

fn main() {
    let args = Args::parse();

    match (args.value_file.as_deref(), args.input.as_deref()) {
        (Some("-") | None, Some("-") | None) => panic!(
            "At least one of `-i` or `-f` must be specified, as otherwise they will both read from stdin"
        ),
        _ => {}
    }

    let value_paths = args.values.iter().map(|pair| {
        let (key_path, value) = pair
            .split_once('=')
            .expect("A value must be specified like `-v path.to.something=value`");
        let value: Value =
            serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.into()));

        (key_path, value)
    });

    let mut input_values: Value = args
        .value_file
        .as_ref()
        .map(|value_file| {
            let format = match Path::new(value_file).extension().and_then(|ext| ext.to_str()) {
                Some("yaml") | Some("yml") => ValueFormat::Yaml,
                Some("toml") => ValueFormat::Toml,
                _ => ValueFormat::Json,
            };
            let mut stdin;
            let mut file_out;
            let value_file: &mut dyn io::Read = match args.value_file.as_deref() {
                Some("-") | None => {
                    stdin = BufReader::new(io::stdin());

                    &mut stdin
                }
                Some(out_path) => {
                    file_out =
                        BufReader::new(File::open(out_path).expect("Could not open value file"));

                    &mut file_out
                }
            };

            match format {
                ValueFormat::Json => {
                    serde_json::from_reader(value_file).expect("Could not read value file")
                }
                ValueFormat::Yaml => {
                    serde_yaml::from_reader(value_file).expect("Could not read value file")
                }
                ValueFormat::Toml => {
                    let mut bytes = Vec::new();
                    value_file.read_to_end(&mut bytes).expect("Could not read value file");
                    toml::from_slice(&bytes).expect("Could not read value file")
                }
            }
        })
        .unwrap_or(Map::new().into());

    for (path, value) in value_paths {
        let mut path_components = path.split('.').peekable();

        let mut cur_map =
            input_values.as_object_mut().expect("Could not access fields of non-object");

        loop {
            let Some(component) = path_components.next().map(ToString::to_string) else {
                break;
            };
            if path_components.peek().is_none() {
                cur_map.insert(component, value);
                break;
            }

            match cur_map.entry(component) {
                Entry::Vacant(vacant_entry) => {
                    let new_map = Value::Object(Map::new());

                    cur_map = vacant_entry.insert(new_map).as_object_mut().unwrap();
                }
                Entry::Occupied(occupied_entry) => {
                    // TODO: No way to get a borrowed key and a mutable value
                    let error_message = format!(
                        "Could not set key {component} of non-object {entry}",
                        component = occupied_entry.key(),
                        entry = occupied_entry.get(),
                    );
                    let entry = occupied_entry.into_mut();
                    cur_map = entry.as_object_mut().unwrap_or_else(|| panic!("{error_message}"));
                }
            }
        }
    }

    let (name, template) = match args.input.as_deref() {
        Some("-") | None => {
            let mut template = String::new();
            io::stdin().read_to_string(&mut template).expect("Could not read template from stdin");
            ("-".into(), Template::compile(&template).expect("Could not compile template"))
        }
        Some(template_path) => {
            let template = fs::read_to_string(template_path).expect("Could not read template");

            (
                template_path.to_string(),
                Template::compile_with_name(&template, template_path.to_string())
                    .expect("Could not compile template"),
            )
        }
    };

    let mut registry = Handlebars::new();

    // This is only ok because we know this script renders a single template at a time
    static VARS: LazyLock<Mutex<HashMap<String, Value>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    handlebars_helper!(var: |name: String, value: Value| {
        VARS.lock().unwrap().insert(name, value);
    });

    handlebars_helper!(val: |name: String| {
        VARS.lock().unwrap().get(&name).expect("Unknown var").clone()
    });

    registry.register_helper("var", Box::new(var));
    registry.register_helper("val", Box::new(val));

    registry.register_template(&name, template);

    let mut stdout;
    let mut file_out;
    let writer: &mut dyn io::Write = match args.output.as_deref() {
        Some("-") | None => {
            stdout = BufWriter::new(io::stdout());

            &mut stdout
        }
        Some(out_path) => {
            file_out = BufWriter::new(File::create(out_path).expect("Could not open output file"));

            &mut file_out
        }
    };

    registry.render_to_write(&name, &input_values, writer).expect("Could not render template");
}
