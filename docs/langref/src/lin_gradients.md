### Linear Gradients

Gradients allow creating smooth colorful surfaces. They are specified using an angle and a series of
color stops. The colors will be linearly interpolated between the stops, aligned to an imaginary line
that is rotated by the specified angle. This is called a linear gradient and is specified using the
`@linear-gradient` macro with the following signature:

**`@linear-gradient(angle, color percentage, color percentage, ...)`**

The first parameter to the macro is an angle (see [Types](#types)). The gradient line's starting point
will be rotated by the specified value.

Following the initial angle is one or multiple color stops, describe as a space separated pair of a
`color` value and a `percentage`. The color specifies which value the linear color interpolation should
reach at the specified percentage along the axis of the gradient.

The following example shows a rectangle that's filled with a linear gradient that starts with a light blue
color, interpolates to a very light shade in the center and finishes with an orange tone:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        background: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%);
    }
}
```
