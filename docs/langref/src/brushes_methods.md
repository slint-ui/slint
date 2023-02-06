### Methods

All colors and brushes have the following methods defined:

-   **`brighter(factor: float) -> Brush`**

    Returns a new color derived from this color but has its brightness increased by the specified factor.
    For example if the factor is 0.5 (or for example 50%) the returned color is 50% brighter. Negative factors
    decrease the brightness.

-   **`darker(factor: float) -> Brush`**

    Returns a new color derived from this color but has its brightness decreased by the specified factor.
    For example if the factor is .5 (or for example 50%) the returned color is 50% darker. Negative factors
    increase the brightness.
