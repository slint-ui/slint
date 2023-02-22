## Images

The `image` type is a reference to an image. It's defined using the `@image-url("...")` construct.
The address within the `@image-url` function must be known at compile time.

Slint looks for images in the following places:

1. The absolute path or the path relative to the current `.slint` file.
2. The include path used by the compiler to look up `.slint` files.

Access an `image`'s dimension using its `width` and `height` properties.

```slint
export component Example inherits Window {
    preferred-width: 150px;
    preferred-height: 50px;

    in property <image> some_image: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");

    Text {
        text: "The image is " + some_image.width + "x" + some_image.height;
    }
}
```
