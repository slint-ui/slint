<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Translations

Slint's translation infrastructure makes your application available in different languages.

## Overview

:::tip Prerequisite

Install the `slint-tr-extractor` tool to extract translatable strings from `.slint` files:

```sh
cargo install slint-tr-extractor
```

:::

Complete the following steps to translate your application:


1. Identify all user visible strings that need translation and annotate them with the `@tr()` macro.
2. Extract annotated strings by running the `slint-tr-extractor` tool and generate `.pot` files.
3. Use a third-party tool to translate the strings into a target language, as `.po` files.
4. Use [gettext's `msgfmt`](https://www.gnu.org/software/gettext/manual/gettext.html) tool to convert `.po` files into run-time loadable `.mo` files.
5. Use Slint's API to select and load `.mo` files at run-time, based on the user's locale settings.

At this point, all strings marked for translation are automatically rendered in the target language.

## Annotating Translatable Strings

Use the `@tr` macro in `.slint` files to mark that a string for translation. This macro
takes care of both the translation and the formatting by replacing `{}` placeholders.

The first argument must be a plain string literal, followed by the arguments:

```slint,no-preview
export component Example {
    property <string> name;
    Text {
        text: @tr("Hello, {}", name);
    }
}
```

### Formatting

The `@tr` macro replaces each `{}` placeholder in the string marked for translation with the corresponding argument.
It's also possible to re-order the arguments using `{0}`, `{1}`, and so on. Translators can use ordered
placeholders even if the original string did not.

You can include the literal characters `{` and `}` in a string by preceding them with the same character.
For example, escaping the `{` character with `{{` and the `}` character with `}}`.

### Plurals

Use plural formatting when the translation of text involving a variable number of elements should change
depending on whether there is a single element or multiple.

Given `count` and an expression that represents the count of something, form the plural with the `|` and `%` symbols like so:

`@tr("I have {n} item" | "I have {n} items" % count)`.

Use `{n}` in the format string to access the expression after the `%`.

```slint,no-preview
export component Example inherits Text {
    in property <int> score;
    in property <int> name;
    text: @tr("Hello {0}, you have one point" | "Hello {0}, you have {n} point" % score, name);
}
```

### Context

Disambiguate translations for strings with the same source text but different contextual meanings by adding a context
to the `@tr(...)` macro using the `"..." =>` syntax.

Use the context to provide additional context information to translators, ensuring accurate and contextually appropriate translations.

The context must be a plain string literal and it appears as `msgctx` in the `.pot` files. If not specified, the context defaults
to the name of the surrounding component.

```slint,no-preview
export component MenuItem {
    property <string> name : @tr("Default Name"); // Default: `MenuItem` will be the context.
    property <string> tooltip : @tr("ToolTip" => "ToolTip for {}", name); // Specified: The context will be `ToolTip`.
}
```

## Extract Translatable Strings

Use the `slint-tr-extractor` tool to generate a `.pot` file from `.slint` files.
You can run it like so:

```sh
find -name \*.slint | xargs slint-tr-extractor -o MY_PROJECT.pot
```

This creates a file called `MY_PROJECT.pot`. Replace "MY_PROJECT" with your actual project name.
To learn how the project name affects the lookup of translations, read the sections below.

:::info

`.pot` files are [Gettext](https://www.gnu.org/software/gettext/) template files.

:::

## Translate the Strings

Start a new translation by creating a `.po` file from a `.pot` file. Both file formats are identical.
You can either copy the file manually or use a tool like Gettext's `msginit` to start a new `.po` file.

The `.po` file contains the strings in a target language.

`.po` and `.pot` files are plain text files that you can edit with a text editor. We recommend
using a dedicated translation tool for working with them, such as the following:

-   [poedit](https://poedit.net/)
-   [OmegaT](https://omegat.org/)
-   [Lokalize](https://userbase.kde.org/Lokalize)
-   [Transifex](https://www.transifex.com/) (web interface)

## Convert `.po` Files to `.mo` Files

Convert the human readable `.po` files into machine-friendly `.mo` files, which are a binary representation
that is more efficient to read by code.

Use [Gettext](https://www.gnu.org/software/gettext/)'s `msgfmt` command line tool to convert `.po` files to `.mo`
files:

```sh
msgfmt translation.po -o translation.mo
```

## Select and Load `.mo` Files at Run-Time

Slint uses the [Gettext](https://www.gnu.org/software/gettext/) library to load translations at run-time.
Gettext expects translation files - called message catalogs - in following directory hierarchy:

```
dir_name/locale/LC_MESSAGES/domain_name.mo
```

-   `dir_name`: the base directory that you can choose freely.
-   `locale`: The name of the user's locale for a given target language, such as `fr` for French, or `de` for German.

    The locale is typically determined using environment variables that your operating system sets.

-   `domain_name`: Selected based on the programming language you're using Slint with.

:::info

Read the [Gettext documentation](https://www.gnu.org/software/gettext/manual/gettext.html#Locating-Catalogs) for more information.

:::

### Select and Load Translations with Rust

First, enable the `gettext` feature of the `slint` crate in the `features` section to gain access to the translations API
and activate run-time translation support.

Next, use the `slint::init_translations!` macro to specify the base location of your `.mo` files. This is
the `dir_name` in the scheme of the previous section. Slint expects the `.mo` files to be in the
corresponding sub-directories and their file name - `domain_name` - must match the package name
in your `Cargo.toml`. This is often the same as the crate name.

For example:

```rust
slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
```

For example, if your `Cargo.toml` contains the following lines and the user's locale is `fr`:

```toml
[package]
name = "gallery"
```

With these settings, Slint looks for `gallery.mo` in the `lang/fr/LC_MESSAGES/gallery.mo`.

### Select and Load Translations with C++

First, enable the `SLINT_FEATURE_GETTEXT` cmake option when compiling Slint to gain access to
the translations API and activate run-time translation support.

In C++ applications using cmake, the `domain_name` is the CMake target name.

Next, bind the text domain to a path using the standard gettext library.

To do so, add this in your `CMakeLists.txt` file:

```cmake
find_package(Intl)
if(Intl_FOUND)
    target_compile_definitions(my_application PRIVATE HAVE_GETTEXT SRC_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
    target_link_libraries(my_application PRIVATE Intl::Intl)
endif()
```

You can then setup the locale and the text domain

```c++
#ifdef HAVE_GETTEXT
#    include <locale>
#    include <libintl.h>
#endif

int main()
{
#ifdef HAVE_GETTEXT
    bindtextdomain("my_application", SRC_DIR "/lang/");
    std::locale::global(std::locale(""));
#endif
   //...
}
```

For example, if you're using the above and the user's locale is `fr`,
Slint looks for `my_application.mo` in the `lang/fr/LC_MESSAGES/` directory.

## Previewing Translations with `slint-viewer`

:::tip Prerequisite

Use `slint-viewer` to preview translations when previewing `.slint` files:

```sh
cargo install slint-viewer
```

:::

1. Enable the `gettext` feature when compiling `slint-viewer`.
2. Use the `--translation-domain` and `translation-dir` command line options to
   load translations and display them based on the current locale.
