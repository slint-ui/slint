# Slint's 7GUIs implementation

[7GUIs](https://7guis.github.io/7guis/) is a "GUI Programming Benchmark".
However rather than benchmarking performance, it offers 7 GUI related tasks that aim to make UI Toolkits comparable.

These 7 challenges have implementations for multiple frameworks already and the following are ours:

## [Counter](https://7guis.github.io/7guis/tasks#counter)
Just a Button that increases a value in a text field.

![Screenshot of the 7GUIs Counter](https://user-images.githubusercontent.com/22800467/168557310-60219332-4774-4ebc-8584-7a973c7918c0.png "Counter")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/counter.slint)

## [Temperature Converter](https://7guis.github.io/7guis/tasks/#temp)
Converts Celsius to Fahrenheit and vice versa.

![Screenshot of the 7GUIs Temperature Converter](https://user-images.githubusercontent.com/22800467/168557382-d00e22e5-c65b-430a-a6a4-72665445f98d.png "Temperature Converter")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/tempconv.slint)

## [Flight Booker](https://7guis.github.io/7guis/tasks/#flight)
Performs some validation checking on dates.
Does not actually book flights.

![Screenshot of the 7GUIs Flight Booker](https://user-images.githubusercontent.com/22800467/168557449-769df1cd-f967-4e14-bc5c-d8eeccc33305.png "Flight Booker")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/booker.slint)
(Note that the logic for date validation and date comparison is implemented in [Rust](./booker.rs).)

## [Timer](https://7guis.github.io/7guis/tasks/#timer)
A simple timer where the duration is adjustable while running.

![Screenshot of the 7GUIs Timer](https://user-images.githubusercontent.com/22800467/168557131-68382191-9228-4d58-9683-6648ab5e7efd.png "Timer")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/timer.slint)
(Note that the actual timer functionality is implemented in [Rust](./timer.rs).)

## [CRUD](https://7guis.github.io/7guis/tasks/#crud)
Lets you create, read, update and delete names from a list as well as filter them by prefix.
Our implementation makes use of `MapModel` and `FilterModel` to achieve this.

![Screenshot of the 7GUIs CRUD](https://user-images.githubusercontent.com/22800467/168557502-93c87141-3eb5-410c-9b83-4b7342727e37.png "CRUD")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/crud.slint)
(Note that the changes to the data model and filtering is implemented in [Rust](./crud.rs).)

## [Circle Drawer](https://7guis.github.io/7guis/tasks/#circle)
Draw some circles on a canvas and change their sizes. It has undo and redo capabilities.

![Screenshot of the 7GUIs Circle Drawer](https://user-images.githubusercontent.com/22800467/168557533-7632efba-3b3b-459d-a8c0-6f166fa42e23.png "Circle Drawer")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/circledraw.slint)
(Note that the undo-redo stack is implemented in [Rust](./circledraw.rs).)

## [Cells](https://7guis.github.io/7guis/tasks/#cells)
Almost MS Excel. It uses nested models to create the table.

![Screenshot of the 7GUIs Cells](https://user-images.githubusercontent.com/22800467/168557595-95ad3255-006c-416a-bccd-8f5251adebd7.png "Cells")

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/cells.slint)
(Note that the cell model, expression evaluation and dependency handling is implemented in [Rust](./cells.rs).)
