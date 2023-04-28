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

**TODO**

## Context

**TODO**

## Extracting the String from the files

**TODO**

## Doing the translations

**TODO**