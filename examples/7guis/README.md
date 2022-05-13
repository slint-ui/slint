# Slint's 7GUIs implementation

[7GUIs](https://eugenkiss.github.io/7guis/) is a "GUI Programming Benchmark".
However rather than benchmarking performance, it offers 7 GUI related tasks that aim to make UI Toolkits comparable.

These 7 challenges have implementations for multiple frameworks already and the following are ours:

## [Counter](https://eugenkiss.github.io/7guis/tasks#counter)
Just a Button that increases a value in a text field.

<!-- ![Screenshot of the 7GUIs Counter](counter.png "Counter") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/counter.slint)

## [Temperature Converter](https://eugenkiss.github.io/7guis/tasks/#temp)
Converts Celsius to Fahrenheit and vice versa.

<!-- ![Screenshot of the 7GUIs Temperature Converter](tempconv.png "Temperature Converter") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/tempconv.slint)

## [Flight Booker](https://eugenkiss.github.io/7guis/tasks/#flight)
Performs some validation checking on dates.
Does not actually book flights.

<!-- ![Screenshot of the 7GUIs Flight Booker](booker.png "Flight Booker") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/booker.slint)
(Not functional on its own, logic in the Rust file)

## [Timer](https://eugenkiss.github.io/7guis/tasks/#timer)
A simple timer. The time is adjustable while running.

<!-- ![Screenshot of the 7GUIs Timer](timer.png "Timer") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/timer.slint)
(Not functional on its own, logic in the Rust file)

## [CRUD](https://eugenkiss.github.io/7guis/tasks/#crud)
Lets you create, read, update and delete names from a list as well as filter them by prefix.
Our implementation makes use of `MapModel` and `FilterModel` to achieve this.

<!-- ![Screenshot of the 7GUIs CRUD](crud.png "CRUD") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/crud.slint)
(Not functional on its own, logic in the Rust file)

## [Circle Drawer](https://eugenkiss.github.io/7guis/tasks/#circle)
Draw some circles on a canvas and change their sizes. It has undo and redo capabilities.

<!-- ![Screenshot of the 7GUIs Circle Drawer](circledraw.png "Circle Drawer") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/circledraw.slint)
(Not functional on its own, logic in the Rust file)

## [Cells](https://eugenkiss.github.io/7guis/tasks/#cells)
Almost MS Excel. It uses nested models to create the table.

<!-- ![Screenshot of the 7GUIs Cells](cells.png "Cells") -->

[`.slint` code in web editor](https://slint-ui.com/editor/?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/7guis/cells.slint)
(Not functional on its own, logic in the Rust file)