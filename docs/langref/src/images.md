## Images

The `image` type is a reference to an image. It be initialized with the `@image-url("...")` construct.
The URL within the `@image-url` function need to be known at compile time, and it is looked up
relative to the file. In addition, it will also be looked in the include path specified to load
.slint files via import.

It is possible to access the `width` and `height` of an image.

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
