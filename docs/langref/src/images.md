## Images

The `image` type is a reference to an image. It's defined using the `@image-url("...")` construct.
The URL within the `@image-url` function must be known at compile time.

The compiler will look for the image relative to the current `.slint` file and will
finally consult the include path for `.slint` files.

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
