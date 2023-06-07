# Translations

Translations of text in Slint can be done with the `@tr` macro.
That macro will take care both of the translation, and formatting (replacements of `{}` placeholders).
The first argument must be a plain string  literal, and the arguments to be formatted follows


```slint,no-preview
export component Example {
    property <string> name;
    Text {
        text: @tr("Hello, {}", name);
    }
}
```

## Formatting

The `@tr` macro will by replacing placeholders in the translated string with corresponding values.
Each `{}` is replaced by the corresponding argument.
It is also possible to re-order the arguments using `{0}`, `{1}` and so on.
The translator can use ordered placeholder even if the original string did not.

The literal characters `{` and `}` may be included in a string by preceding them with the same character. For example, the `{` character is escaped with `{{` and the `}` character is escaped with `}}`.

## Plurals

A special kind of formatting is the plural formatting, because the string may change depending on if there is a single element or several.

Given `count` and an expression that represents the count of something, we can form the plural with the `|` and `%` symbols like so:
`@tr("I have {n} item" | "I have {n} items" % count)`.
Use `{n}` in the format string to access the expression after the `%`.

```slint,no-preview
export component Example inherits Text {
    in property <int> score;
    in property <int> name;
    text: @tr("Hello {0}, you have one point" | "Hello {0}, you have {n} point" % score, name);
}
```

## Context

It is possible to add a context in the `@tr(...)` macro using the `"..." =>`.

The context provides a mechanism to disambiguate translations for strings with the same source text but different contextual meanings.
Use the context to provide additional context information to translators, ensuring accurate and contextually appropriate translations.

The context must be a plain string literal.


```slint,no-preview
export component MenuItem {
    property <string> name : @tr("Name" => "Default Name");
    property <string> tooltip : @tr("ToolTip" => "ToolTip for {}", name);
}
```

## Extracting the String from the files

Use the `slint-tr-extractor` tool to generate a .po file from .slint files.
You can run it like so:

```sh
find -name \*.slint | xargs slint-tr-extractor -o MY_PROJECT.pot
```

This will create a file called `MY_PROJECT.pot`. Replace MY_PROJECT with your actual project name. To learn how the project name affects the lookup of translations, see the sections below.

## Translating Your Application

`.pot` file are [Gettext](https://www.gnu.org/software/gettext/) template files. It is the same as a `.po` file, but doesn't contain actual
translations. They can be edited by hand with a text editor, or there are a few tools you can use to translate them, option include:
 - [poedit](https://poedit.net/)
 - [OmegaT](https://omegat.org/)
 - [Lokalize](https://userbase.kde.org/Lokalize)
 - [Transifex](https://www.transifex.com/) (web interface)

## Loading the Translations at Run-Time

[Gettext](https://www.gnu.org/software/gettext/) is used at runtime to get the translations.

So the first thing to do is to convert the `.po` files in `.mo` files that the gettext runtime can open.
This can be done with the `msgfmt` command line tool from the gettext package.

Then, gettext will locate the translation file in the following location:

```
dir_name/locale/LC_MESSAGES/domain_name.mo
```

See the [Gettext documentation](https://www.gnu.org/software/gettext/manual/gettext.html#Locating-Catalogs) for more info.

The locale is determined with environment variables.

The dir_name and domain_name are optained depending on with which programming language slint is used:

### Rust

You must enable the `gettext` feature of the `slint` create.

When used from Rust, either via a build.rs script or using a `slint!` macro, the `domain_name`
is the same as the package name from the Cargo.toml (This is often the same as the crate name)

You must specify `dir_name` with the init_translation macro.
For example, it may look like this:

```rust
slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
```

### C++

In C++ application using cmake, the `domain_name` is the CMake target name.

You will also need to bind the domain to a path using the standard gettext library.

To do so, you can add this in your CMakeLists.txt

```cmake
find_package(Intl)
if(Intl_FOUND)
    target_compile_definitions(gallery PRIVATE HAVE_GETTEXT SRC_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
    target_link_libraries(gallery PRIVATE Intl::Intl)
endif()
```

You can then setup the locale and the bindtext domain

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

### `slint-viewer`

When previewing files with the `slint-viewer` binary, you can pass the `--translation-domain` and `--translation-dir`.
option to the viewer
