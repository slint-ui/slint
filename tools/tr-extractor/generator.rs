// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use super::Message;
use chrono::prelude::*;
use std::io::prelude::*;
use std::path::Path;

/// How much [Message](Message) location information to include in the
/// output.
#[derive(PartialEq, Debug)]
pub enum AddLocation {
    /// Format the locations output as ‘#: filename:line’
    /// This is the default.
    Full,
    /// Format the locations output as ‘#: filename`
    #[allow(unused)]
    File,
    /// Don't include the message locations.
    Never,
}

pub struct OutputDetails {
    pub omit_header: bool,
    pub copyright_holder: Option<String>,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub bugs_address: Option<String>,
    pub charset: String,
    pub add_location: AddLocation,
}

pub fn generate<'a, P: AsRef<Path>>(
    output: P,
    output_details: OutputDetails,
    messages: impl IntoIterator<Item = &'a Message>,
) -> ::std::io::Result<()> {
    let mut output = std::fs::File::create(output)?;

    if !output_details.omit_header {
        let package = output_details.package_name.as_ref().map(|x| x.as_ref()).unwrap_or("PACKAGE");
        write!(
            output,
            r#"# SOME DESCRIPTIVE TITLE.
# Copyright (C) YEAR {copyright}
# This file is distributed under the same license as the {package} package.
# FIRST AUTHOR <EMAIL@ADDRESS>, YEAR.
#
#, fuzzy
msgid ""
msgstr ""
"Project-Id-Version: {package} {version}\n"
"Report-Msgid-Bugs-To: {address}\n"
"POT-Creation-Date: {date}\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\n"
"Language-Team: LANGUAGE <LL@li.org>\n"
"Language: \n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset={charset}\n"
"Content-Transfer-Encoding: 8bit\n"
"#,
            package = package,
            version =
                output_details.package_version.as_ref().map(|x| x.as_ref()).unwrap_or("VERSION"),
            copyright = output_details
                .copyright_holder
                .unwrap_or_else(|| format!("THE {}'S COPYRIGHT HOLDER", package)),
            date = Utc::now().format("%Y-%m-%d %H:%M%z").to_string(),
            address = output_details.bugs_address.unwrap_or_default(),
            charset = output_details.charset,
        )?;
    }

    for m in messages {
        writeln!(output)?;
        if let Some(ref c) = m.comments {
            for c in c.split('\n') {
                writeln!(output, "#. {}", c)?;
            }
        }

        if !m.locations.is_empty() && (output_details.add_location != AddLocation::Never) {
            write!(output, "#:")?;
            for l in &m.locations {
                match output_details.add_location {
                    AddLocation::Full => {
                        write!(output, " {}:{}", l.file.to_string_lossy(), l.line)?;
                    }
                    AddLocation::File => {
                        write!(output, " {}", l.file.to_string_lossy())?;
                    }
                    AddLocation::Never => {}
                }
            }
            writeln!(output)?;
        }
        if let Some(ref c) = m.msgctxt {
            writeln!(output, "msgctxt {}", escape(&c))?;
        }

        writeln!(output, "msgid {}", escape(&m.msgid))?;

        if let Some(ref c) = m.plural {
            writeln!(output, "msgid_plural {}", escape(&c))?;
            writeln!(output, "msgstr[0] \"\"")?;
            writeln!(output, "msgstr[1] \"\"")?;
        } else {
            writeln!(output, "msgstr \"\"")?;
        }
    }
    Ok(())
}

fn escape(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('\"', "\\\"").replace('\n', "\\n\"\n\""))
}
