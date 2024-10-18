## `Image`

An `Image` can be used to represent an image loaded from a file.

### Properties

-   **`colorize`** (_in_ _brush_): When set, the image is used as an alpha mask and is drawn in the given color (or with the gradient).
-   **`horizontal-alignment`** (_in_ _enum [`ImageHorizontalAlignment`](../language/builtins/enums.md#imagehorizontalalignment)_): The horizontal alignment of the image within the element.
-   **`horizontal-tiling`** (_in_ _enum [`ImageTiling`](../language/builtins/enums.md#imagetiling)_): Whether the image should be tiled on the horizontal axis.
-   **`image-fit`** (_in_ _enum [`ImageFit`](../language/builtins/enums.md#imagefit)_): Specifies how the source image shall be fit into the image element.
    Does not have any effect when used with 9 slice scaled or tiled images.
    (default value: `contain` when the `Image` element is part of a layout, `fill` otherwise)
-   **`image-rendering`** (_in_ _enum [`ImageRendering`](../language/builtins/enums.md#imagerendering)_): Specifies how the source image will be scaled. (default value: `smooth`)
-   **`rotation-angle`** (_in_ _angle_), **`rotation-origin-x`** (_in_ _length_), **`rotation-origin-y`** (_in_ _length_):
    Rotates the image by the given angle around the specified origin point. The default origin point is the center of the element.
    When these properties are set, the `Image` can't have children.
-   **`source`** (_in_ _image_): The image to load. Use the [`@image-url("...")` macro](../language/syntax/types.md#images) to specify the location of the image.
-   **`source-clip-x`**, **`source-clip-y`**, **`source-clip-width`**, **`source-clip-height`** (_in_ _int_): Properties in source
    image coordinates that define the region of the source image that is rendered. By default the entire source image is visible:
    | Property | Default Binding |
    |----------|---------------|
    | `source-clip-x` | `0` |
    | `source-clip-y` | `0` |
    | `source-clip-width` | `source.width - source-clip-x` |
    | `source-clip-height` | `source.height - source-clip-y` |
-   **`vertical-alignment`** (_in_ _enum [`ImageVerticalAlignment`](../language/builtins/enums.md#imageverticalalignment)_): The vertical alignment of the image within the element.
-   **`vertical-tiling`** (_in_ _enum [`ImageTiling`](../language/builtins/enums.md#imagetiling)_): Whether the image should be tiled on the vertical axis.
-   **`width`**, **`height`** (_in_ _length_): The width and height of the image as it appears on the screen.The default values are
    the sizes provided by the **`source`** image. If the `Image` is **not** in a layout and only **one** of the two sizes are
    specified, then the other defaults to the specified value scaled according to the aspect ratio of the **`source`** image.

### Example

```slint
export component Example inherits Window {
    width: 100px;
    height: 100px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            // image-fit default is `contain` when in layout, preserving aspect ratio
        }
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            colorize: red;
        }
    }
}
```

Scaled while preserving the aspect ratio:

```slint
export component Example inherits Window {
    width: 100px;
    height: 150px;
    VerticalLayout {
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            width: 100px;
            // implicit default, preserving aspect ratio:
            // height: self.width * natural_height / natural_width;
        }
    }
}
```

Example using nine-slice:

```slint
export component Example inherits Window {
    width: 100px;
    height: 150px;
    VerticalLayout {
        Image {
            source: @image-url("https://interactive-examples.mdn.mozilla.net/media/examples/border-diamonds.png", nine-slice(30));
        }
    }
}
```