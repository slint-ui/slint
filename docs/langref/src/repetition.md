# Repetition

The `for`-`in` syntax can be used to repeat an element.

The syntax look like this: `for name[index] in model : id := Element { ... }`

The _model_ can be of the following type:

-   an integer, in which case the element will be repeated that amount of time
-   an array type or a model declared natively, in which case the element will be instantiated for each element in the array or model.

The _name_ will be available for lookup within the element and is going to be like a pseudo-property set to the
value of the model. The _index_ is optional and will be set to the index of this element in the model.
The _id_ is also optional.
