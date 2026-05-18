// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::Parser;
use i_slint_formatter::Formatter;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Minimal Slint formatter harness")]
struct Args {
    /// Format the given file instead of reading from stdin.
    file: Option<PathBuf>,

    /// Check whether the input would change.
    #[arg(long)]
    check: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let formatter = match Formatter::new() {
        Ok(formatter) => formatter,
        Err(err) => return report_error(&err),
    };

    let result = match args.file {
        Some(path) => match formatter.format_path(&path) {
            Ok(result) => result,
            Err(err) => return report_error(&err),
        },
        None => match read_stdin().map(|source| formatter.format_str(&source)) {
            Ok(Ok(result)) => result,
            Ok(Err(err)) => return report_error(&err),
            Err(err) => return report_error(&err),
        },
    };

    if args.check {
        return if result.changed { ExitCode::from(1) } else { ExitCode::SUCCESS };
    }

    if let Err(err) = io::stdout().write_all(result.text.as_bytes()) {
        return report_error(&err);
    }

    ExitCode::SUCCESS
}

fn read_stdin() -> io::Result<String> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    Ok(source)
}

fn report_error(err: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("{err}");
    ExitCode::from(1)
}
