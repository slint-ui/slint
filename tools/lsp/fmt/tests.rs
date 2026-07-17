// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! End-to-end tests for the formatter, ported from the old imperative formatter's test suite.
//! Where the two formatters disagree, the expected output follows the query-based formatter,
//! so this suite doubles as the record of the behavior differences.

use super::rules::format_document_query;
use super::writer::FileWriter;
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::syntax_nodes;

/// Assert the formatted output, and that formatting is idempotent
/// (formatting the output again changes nothing).
#[track_caller]
fn assert_formatting(unformatted: &str, formatted: &str) {
    assert_eq!(format_once(unformatted), formatted);
    assert_eq!(format_once(formatted), formatted, "formatting is not idempotent");
}

fn format_once(source: &str) -> String {
    let syntax_node = i_slint_compiler::parser::parse(
        String::from(source),
        None,
        &mut BuildDiagnostics::default(),
    );
    let document = syntax_nodes::Document::new(syntax_node).unwrap();
    let mut output = Vec::new();
    format_document_query(document, &mut FileWriter { file: &mut output }).unwrap();
    String::from_utf8(output).unwrap()
}

#[test]
fn basic_formatting() {
    assert_formatting(
        "A:=Text{}",
        r#"A := Text {}
"#,
    );
}

#[test]
fn components() {
    assert_formatting(
        "component   A   {}  export component  B  inherits  Text {  }",
        r#"component A {}
export component B inherits Text {}
"#,
    );
}

#[test]
fn with_comments() {
    assert_formatting(
        r#"component /* */ Foo // aaa
            inherits  /* x */  // bbb
            Window // ccc
            /*y*/ {   // c
              Foo /*aa*/ /*bb*/ {}
            }
        "#,
        r#"component /* */ Foo // aaa
inherits /* x */ // bbb
Window // ccc
/*y*/ { // c
    Foo /*aa*/ /*bb*/ {}
}
"#,
    );

    assert_formatting(
        r#"//xxx
component  C1 {
    // before a property
    property< int> p1;
    property<int > p2; // on the same line of a property
    property <int > p3;
    // After a property
    // ...
}
"#,
        r#"//xxx
component C1 {
    // before a property
    property <int> p1;
    property <int> p2; // on the same line of a property
    property <int> p3;
    // After a property
    // ...
}
"#,
    );
}

#[test]
fn complex_formatting() {
    assert_formatting(
        r#"
Main :=Window{callback some-fn(string,string)->bool;some-fn(a, b)=>{a<=b} property<bool>prop-x;
    VerticalBox {      combo:=ComboBox{}    }
    pure   callback   some-fn  ({x: int},string);  in property <  int >  foo: 42; }
            "#,
        r#"Main := Window {
    callback some-fn(string, string) -> bool;
    some-fn(a, b) => { a <= b }
    property <bool> prop-x;
    VerticalBox { combo := ComboBox {} }
    pure callback some-fn({ x: int }, string);
    in property <int> foo: 42;
}
"#,
    );
}

#[test]
fn callback_declaration() {
    assert_formatting(
        r#"
component W inherits Window{
    callback hello( with-name :int , { x: int, y: float} , foo: string );
    callback world   (  )  -> string;
    callback another_callback ;
}
"#,
        r#"component W inherits Window {
    callback hello(with-name: int, { x: int, y: float }, foo: string);
    callback world() -> string;
    callback another_callback;
}
"#,
    );
}

#[test]
fn parent_access() {
    assert_formatting(
        r#"
Main := Parent{            Child{
    some-prop: parent.foo - 60px;
}}"#,
        r#"Main := Parent {
    Child {
        some-prop: parent.foo - 60px;
    }
}
"#,
    );
}

#[test]
fn space_with_braces() {
    assert_formatting(
        "Main := Window{}",
        r#"Main := Window {}
"#,
    );
    // Also in a child
    assert_formatting(
        r#"
Main := Window{Child{}}"#,
        r#"Main := Window { Child {} }
"#,
    );
    assert_formatting(
        r#"
Main := VerticalLayout{HorizontalLayout{prop:3;}}"#,
        r#"Main := VerticalLayout { HorizontalLayout { prop: 3; } }
"#,
    );
}

#[test]
fn binary_expressions() {
    assert_formatting(
        r#"
Main := Some{
    a:3+2;  b:4-7;  c:3*7;  d:3/9;
    e:3==4; f:3!=4; g:3<4;  h:3<=4;
    i:3>4;  j:3>=4; k:3&&4; l:3||4;
}"#,
        r#"Main := Some {
    a: 3 + 2;
    b: 4 - 7;
    c: 3 * 7;
    d: 3 / 9;
    e: 3 == 4;
    f: 3 != 4;
    g: 3 < 4;
    h: 3 <= 4;
    i: 3 > 4;
    j: 3 >= 4;
    k: 3 && 4;
    l: 3 || 4;
}
"#,
    );

    assert_formatting(
        r#"
Main := Some{
    m: 3 + 8;
    m:3 + 8;
    m: 3+ 8;
    m:3+ 8;
    m: 3 +8;
    m:3 +8;
    m: 3+8;
    m:3+8;
}"#,
        r#"Main := Some {
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
    m: 3 + 8;
}
"#,
    );
}

#[test]
fn file_with_an_import() {
    assert_formatting(
        r#"
import { Some } from "./here.slint";

A := Some{    padding-left: 10px;    Text{        x: 3px;    }}"#,
        r#"import { Some } from "./here.slint";

A := Some { padding-left: 10px; Text { x: 3px; } }
"#,
    );
}

#[test]
fn children() {
    // Regression test - children was causing additional newlines
    assert_formatting(
        r#"
A := B {
    C {
        @children
    }
}"#,
        r#"A := B {
    C {
        @children
    }
}
"#,
    );
}

#[test]
fn for_in() {
    assert_formatting(
        r#"
A := B {  for c   in root.d: T  { e: c.attr; } }
        "#,
        r#"A := B { for c in root.d: T { e: c.attr; } }
"#,
    );
}

#[test]
fn for_in_index() {
    assert_formatting(
        r#"
A := B {
    for number   [  index
         ]  in [1,2,3]: C { d:number*index; }
}
        "#,
        r#"A := B {
    for number[index] in [1, 2, 3]: C { d: number * index; }
}
"#,
    );
}

#[test]
fn if_element() {
    assert_formatting(
        r#"
component A {  if condition : Text {  }  }
        "#,
        r#"component A { if condition: Text {} }
"#,
    );
}

#[test]
fn array() {
    assert_formatting(
        r#"
A := B { c: [1,2,3]; }
"#,
        r#"A := B { c: [1, 2, 3]; }
"#,
    );
    assert_formatting(
        r#"
A := B { c: [    1    ]; }
"#,
        r#"A := B { c: [1]; }
"#,
    );
    assert_formatting(
        r#"
A := B { c:   [    ]  ; }
"#,
        r#"A := B { c: []; }
"#,
    );
    assert_formatting(
        r#"
A := B { c:   [

1,

2

  ]  ; }
"#,
        r#"A := B {
    c: [
        1,
        2
    ];
}
"#,
    );
}

#[test]
fn states() {
    assert_formatting(
        r#"
component FooBar {
    states [
        dummy1    when    a == true   :{

        }
    ]
}
"#,
        r#"component FooBar {
    states [
        dummy1 when a == true: {
        }
    ]
}
"#,
    );

    assert_formatting(
        r#"
component ABC {
    in-out property <bool> b: false;
    in-out property <int> a: 1;
    states[
        is-selected when root.b == root.b: {
            b:false;
        root.a:1;
        }
        is-not-selected when root.b!=root.b: {
            root.a: 1;
        }
    ]    foo := Rectangle { }
}
"#,
        r#"component ABC {
    in-out property <bool> b: false;
    in-out property <int> a: 1;
    states [
        is-selected when root.b == root.b: {
            b: false;
            root.a: 1;
        }
        is-not-selected when root.b != root.b: {
            root.a: 1;
        }
    ]
    foo := Rectangle {}
}
"#,
    );
}

#[test]
fn state_issue_4850() {
    // #4850
    assert_formatting(
        "export component LspCrashMvp { states [ active: { } inactive: { } ] }",
        r#"export component LspCrashMvp { states [ active: { } inactive: { } ] }
"#,
    );
}

#[test]
fn states_transitions() {
    assert_formatting(
        r#"
component FooBar {
    states [
        //comment
        s1 when true: {
    vv: 0;  in { animate vv {  duration: 400ms; }}  out { animate /*...*/  vv { duration:    400ms;   } animate dd { duration: 100ms+400ms ;  easing: ease-out; }   }  }
    ]
}
"#,
        r#"component FooBar {
    states [
        //comment
        s1 when true: {
            vv: 0;
            in { animate vv { duration: 400ms; } }
            out { animate /*...*/ vv { duration: 400ms; } animate dd { duration: 100ms + 400ms; easing: ease-out; } }
        }
    ]
}
"#,
    );

    assert_formatting(
        "component FooBar {states[foo:{in{animate x{duration:1ms;}}x:0;}]}",
        r#"component FooBar { states [ foo: { in { animate x { duration: 1ms; } } x: 0; } ] }
"#,
    );
}

#[test]
fn if_else() {
    assert_formatting(
        r#"
A := B { c: true  ||false?  45 + 6:34+ 1; }
"#,
        r#"A := B { c: true || false ? 45 + 6 : 34 + 1; }
"#,
    );
    assert_formatting(
        r#"A := B { c => { if(!abc){nothing}else   if  (true){if (0== 8) {}    } else{  } } } "#,
        r#"A := B { c => { if (!abc) { nothing } else if (true) { if (0 == 8) {} } else {} } }
"#,
    );
    assert_formatting(
        r#"A := B { c => { if !abc{nothing}else   if  true{if 0== 8 {}    } else{  } } } "#,
        r#"A := B { c => { if !abc { nothing } else if true { if 0 == 8 {} } else {} } }
"#,
    );

    assert_formatting(
        "component A { c => { if( a == 1 ){b+=1;} else if (a==2)\n{b+=2;} else if a==3{\nb+=3;\n} else\n if(a==4){ a+=4} return 0;  } }",
        r"component A {
    c => {
        if (a == 1) { b += 1; } else if (a == 2) { b += 2; } else if a == 3 {
            b += 3;
        } else if (a == 4) { a += 4 }
        return 0;
    }
}
",
    );
}

#[test]
fn code_block() {
    assert_formatting(
        r#"
component ABC {
    in-out property <bool> logged_in: false;
    function clicked() -> bool {
        if (logged_in) { foo();
            logged_in = false;
        return
        true;
        } else {
            logged_in = false; return false;
        }
    }
}
"#,
        r#"component ABC {
    in-out property <bool> logged_in: false;
    function clicked() -> bool {
        if (logged_in) {
            foo();
            logged_in = false;
            return true;
        } else {
            logged_in = false;
            return false;
        }
    }
}
"#,
    );
}

#[test]
fn trailing_comma_array() {
    assert_formatting(
        r#"
component ABC {
    in-out property <[int]> ar: [1, ];
    in-out property <[int]> ar: [1, 2, 3, 4, 5,];
    in-out property <[int]> ar2: [1, 2, 3, 4, 5];
}
"#,
        r#"component ABC {
    in-out property <[int]> ar: [1,];
    in-out property <[int]> ar: [1, 2, 3, 4, 5,];
    in-out property <[int]> ar2: [1, 2, 3, 4, 5];
}
"#,
    );
}

#[test]
fn large_array() {
    assert_formatting(
        r#"
component ABC {
    in-out property <[string]> large: ["first string", "second string", "third string", "fourth string", "fifth string"];
    in property <[int]> model: [
                                    1,
                                    2
    ];
}
"#,
        r#"component ABC {
    in-out property <[string]> large: ["first string", "second string", "third string", "fourth string", "fifth string"];
    in property <[int]> model: [
        1,
        2
    ];
}
"#,
    );
}

#[test]
fn property_animation() {
    assert_formatting(
        r#"
export component MainWindow inherits Window {
    animate background { duration: 800ms;}
    animate x { duration: 100ms; easing: ease-out-bounce; }
    Rectangle {}
}
"#,
        r#"export component MainWindow inherits Window {
    animate background { duration: 800ms; }
    animate x { duration: 100ms; easing: ease-out-bounce; }
    Rectangle {}
}
"#,
    );
}

#[test]
fn object_literal() {
    assert_formatting(
        r#"
export component MainWindow inherits Window {
    in property <[TileData]> memory-tiles : [
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false,},
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345 },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345},
        { image: @image-url("icons/balance-scale.png") },
    ];
}
"#,
        r#"export component MainWindow inherits Window {
    in property <[TileData]> memory-tiles: [
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345 },
        { image: @image-url("icons/at.png"), image-visible: false, solved: false, some_other_property: 12345 },
        { image: @image-url("icons/balance-scale.png") },
    ];
}
"#,
    );
}

#[test]
fn struct_declaration() {
    assert_formatting(
        r#"
export struct ParsedMarkdown {  string :string , span:{start:int,end:int,}, }
"#,
        r#"export struct ParsedMarkdown { string: string, span: { start: int, end: int, }, }
"#,
    );

    assert_formatting(
        r#"
struct ParsedMarkdown {  string :string , span:{start: int, end:int} , x: int }
"#,
        r#"struct ParsedMarkdown { string: string, span: { start: int, end: int }, x: int }
"#,
    );

    // issue 10647
    assert_formatting(
        r#"
struct PrinterQueueItem {
    status: string,
    progress: int,
    title: string,
    owner: string,
    pages: int,
    size: string, // number instead and format in .slint?
    submission-date: string}

            "#,
        r#"struct PrinterQueueItem {
    status: string,
    progress: int,
    title: string,
    owner: string,
    pages: int,
    size: string, // number instead and format in .slint?
    submission-date: string
}
"#,
    );
}

#[test]
fn struct_declaration_preserves_comments_and_top_level_spacing() {
    assert_formatting(
        r#"
export struct AttributeData {
    key: string,
    key-error: string,
    type: int,
    value: string,
    value-valid: bool,
    unit: int,
    action: int,  // Some comment
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
        r#"export struct AttributeData {
    key: string,
    key-error: string,
    type: int,
    value: string,
    value-valid: bool,
    unit: int,
    action: int, // Some comment
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
    );

    assert_formatting(
        r#"
export struct AttributeData {
    action: int,  // Some comment
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
        r#"export struct AttributeData {
    action: int, // Some comment
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
    );

    assert_formatting(
        r#"
export struct AttributeData {
    key: string, // Set by UI, reset by backend
    key-error: string,
    type: int,
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
        r#"export struct AttributeData {
    key: string, // Set by UI, reset by backend
    key-error: string,
    type: int,
}

export struct LineEditData {
    enabled: bool,
    text: string,
    placeholder: string,
    suggestions: [string],
}
"#,
    );
}

#[test]
fn multiline_binary_expression() {
    assert_formatting(
        "component A {\n    x: a && b && c;\n}\n",
        "component A {\n    x: a && b && c;\n}\n",
    );
    assert_formatting(
        "component A {\n    x: a &&\nb &&\nc;\n}\n",
        r#"component A {
    x: a && b && c;
}
"#,
    );
    assert_formatting(
        "component A {\n    x: a\n&& b\n&& c;\n}\n",
        r#"component A {
    x: a && b && c;
}
"#,
    );
}

#[test]
fn multiline_ternary_expression() {
    assert_formatting(
        "component A {\n    x: a ? b : c;\n}\n",
        "component A {\n    x: a ? b : c;\n}\n",
    );
    assert_formatting(
        "component A {\n    x: a\n? b\n: c;\n}\n",
        r#"component A {
    x: a ? b : c;
}
"#,
    );
    assert_formatting(
        "component A {\n    x: cond1 ? 45\n        : cond2 ? 89\n        : 32;\n}\n",
        r#"component A {
    x: cond1 ? 45 : cond2 ? 89 : 32;
}
"#,
    );
}

#[test]
fn enum_declaration() {
    assert_formatting("enum Foo {  a,   b,   c }\n", "enum Foo { a, b, c }\n");
    assert_formatting("export enum Foo {  a,   b,   c }\n", "export enum Foo { a, b, c }\n");
    assert_formatting(
        "enum Foo { a, b, c, }\n",
        r#"enum Foo { a, b, c, }
"#,
    );
}

#[test]
fn export_list() {
    assert_formatting("export {Foo,Bar}\n", "export { Foo, Bar }\n");
    assert_formatting("export {Foo  as   Bar}\n", "export { Foo as Bar }\n");
    assert_formatting(
        "export { SuperLongTypeName, AnotherVeryLongTypeName, YetAnotherExtremelyLongTypeName }\n",
        r#"export { SuperLongTypeName, AnotherVeryLongTypeName, YetAnotherExtremelyLongTypeName }
"#,
    );
    assert_formatting(
        "export { Foo, }\n",
        r#"export { Foo, }
"#,
    );
}

#[test]
fn preserve_empty_lines() {
    assert_formatting(
        r#"
export component MainWindow inherits Rectangle {
    in property <bool> open-curtain;
    callback clicked;

    border-radius: 8px;


    animate background { duration: 800ms; }

    Image {
        y: 8px;
    }


    Image {
        y: 8px;
    }
}
"#,
        r#"export component MainWindow inherits Rectangle {
    in property <bool> open-curtain;
    callback clicked;

    border-radius: 8px;

    animate background { duration: 800ms; }

    Image {
        y: 8px;
    }

    Image {
        y: 8px;
    }
}
"#,
    );
}

#[test]
fn preserve_empty_lines_between_top_level_declarations() {
    assert_formatting(
        r#"
struct Foo {}

struct Bar {
    dummy: int,
}

component HelloWorld {
    // ...
}
"#,
        r#"struct Foo {}

struct Bar {
    dummy: int,
}

component HelloWorld {
    // ...
}
"#,
    );
}

#[test]
fn preserve_top_level_comment_spacing() {
    assert_formatting(
        r#"
component RoundButton inherits Image {
    property <int> x;
}
// From UpAndDownButton.cpp
component UpAndDownButton inherits Rectangle {
    callback changed(int);
}
"#,
        r#"component RoundButton inherits Image {
    property <int> x;
}
// From UpAndDownButton.cpp
component UpAndDownButton inherits Rectangle {
    callback changed(int);
}
"#,
    );
}

#[test]
fn preserve_blank_line_before_top_level_comment() {
    assert_formatting(
        r#"
component RoundButton inherits Image {
    callback clicked;
}

// From UpAndDownButton.cpp
component UpAndDownButton inherits Rectangle {
    callback changed(int);
}
"#,
        r#"component RoundButton inherits Image {
    callback clicked;
}

// From UpAndDownButton.cpp
component UpAndDownButton inherits Rectangle {
    callback changed(int);
}
"#,
    );
}

#[test]
fn preserve_blank_lines_between_top_level_items() {
    assert_formatting(
        r#"
struct Palette {}

global Skin {
    // ...
}

export component Clock {
}
"#,
        r#"struct Palette {}

global Skin {
    // ...
}

export component Clock {
}
"#,
    );
}

#[test]
fn multiple_property_animation() {
    assert_formatting(
        r#"
export component MainWindow inherits Rectangle {
    animate x , y { duration: 170ms; easing: cubic-bezier(0.17,0.76,0.4,1.75); }
    animate x , y { duration: 170ms;}
}
"#,
        r#"export component MainWindow inherits Rectangle {
    animate x, y { duration: 170ms; easing: cubic-bezier(0.17, 0.76, 0.4, 1.75); }
    animate x, y { duration: 170ms; }
}
"#,
    );
}

#[test]
fn empty_array() {
    assert_formatting(
        r#"
export component MainWindow2 inherits Rectangle {
    in property <[string]> model: [ ];
}
"#,
        r#"export component MainWindow2 inherits Rectangle {
    in property <[string]> model: [];
}
"#,
    );
}

#[test]
fn two_way_binding() {
    assert_formatting(
        "export component Foobar{foo<=>xx.bar ; property<int\n>xx   <=>   ff . mm  ; callback doo <=> moo\n;\nproperty  e-e<=>f-f; }",
        r#"export component Foobar {
    foo <=> xx.bar;
    property <int> xx <=> ff.mm;
    callback doo <=> moo;
    property e-e <=> f-f;
}
"#,
    );
}

#[test]
fn callback_connection() {
    assert_formatting(
        "export component Foobar{ init=>{  debug (1 );} \n\nfoo=>{}  clicked =>   debug(2) ; TouchArea { clicked => root.clicked(); moved=>{debug(3)};\n\n//some comment\n        bar=>{} }  }",
        r#"export component Foobar {
    init => { debug(1); }

    foo => {}
    clicked => debug(2);
    TouchArea {
        clicked => root.clicked();
        moved => { debug(3) };

//some comment
        bar => {}
    }
}
"#,
    );
}

#[test]
fn code_block_single_line() {
    // A code block that fits on one line in the source stays on one line
    assert_formatting(
        "component A { c => { root.foo = true; } click2 => {} f() => { a; b; c; } }\n",
        r#"component A { c => { root.foo = true; } click2 => {} f() => { a; b; c; } }
"#,
    );
    // A code block with a newline in the source is expanded
    assert_formatting(
        "component A {\n  c => {\n    root.foo = true;\n  }\n}\n",
        r#"component A {
    c => {
        root.foo = true;
    }
}
"#,
    );
}

#[test]
fn function() {
    assert_formatting(
        "export component Foo-bar{ pure\nfunction\n(x  :  int,y:string)->int{ self.y=0;\n\nif(true){return(45); a=0;} return x;  } function a(){/* ddd */}}",
        r#"export component Foo-bar {
    pure function(x: int, y: string) -> int {
        self.y = 0;

        if (true) { return (45); a = 0; }
        return x;
    }
    function a() {/* ddd */}
}
"#,
    );
}

#[test]
fn changed() {
    assert_formatting(
        "component X { changed   width=>{ x+=1;  }    changed/*-*/height     =>     {y+=1;} }",
        r#"component X { changed width => { x += 1; } changed /*-*/ height => { y += 1; } }
"#,
    );
}

#[test]
fn access_member() {
    assert_formatting(
        "component X { expr: 42   .log(x) + 41 . log(y) + foo . bar +  21.0.log(0) + 54.   .log(8) ; x: 42px.max(42px . min (0.px)); }",
        r#"component X { expr: 42 .log(x) + 41 .log(y) + foo.bar + 21.0.log(0) + 54..log(8); x: 42px.max(42px.min(0.px)); }
"#,
    );
}

#[test]
fn let_statement() {
    assert_formatting(
        "component X { function foo() { let bar=42; } }",
        r#"component X { function foo() { let bar = 42; } }
"#,
    );
}

#[test]
fn let_statement_type_annotation() {
    assert_formatting(
        "component X { function foo() { let bar : int=42; } }",
        r#"component X { function foo() { let bar: int = 42; } }
"#,
    );
}

#[test]
fn comment_in_nest() {
    assert_formatting(
        r#"component X {
    // function foo() {
    // }
    }
"#,
        r#"component X {
    // function foo() {
    // }
}
"#,
    );
}

// cspell:disable
#[test]
fn import_line_too_long() {
    assert_formatting(
        r#"import { SuperFooooooooooooooooooooooooooooooooooooooooooooooooo } from "./here.slint";"#,
        r#"import { SuperFooooooooooooooooooooooooooooooooooooooooooooooooo } from "./here.slint";
"#,
    );
}

#[test]
fn import_line_is_too_long_because_of_trailing_spaces_but_no_format_happens() {
    // The input is a parse error (an unterminated string), so the
    // trailing tokens are loose error-recovery children of the
    // document: they keep their line, only re-spaced.
    assert_formatting(
        r#"import { SuperFoooooooooooooooooooooooooo } from "./here.slint;                     "#,
        r#"import { SuperFoooooooooooooooooooooooooo } from "./ here.slint;
"#,
    );
}

#[test]
fn single_import_space() {
    assert_formatting(
        r#"import {Foo} from "./here.slint";"#,
        r#"import { Foo } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import { Foo} from "./here.slint";"#,
        r#"import { Foo } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo } from "./here.slint";"#,
        r#"import { Foo } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {     Foo     } from "./here.slint";"#,
        r#"import { Foo } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo as FooBar } from "./here.slint";"#,
        r#"import { Foo as FooBar } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo as FooBar} from "./here.slint";"#,
        r#"import { Foo as FooBar } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foooooooooooooooooooooooooooooooo as FooooooooooooooooooooooooooooooooBar} from "./here.slint";"#,
        r#"import { Foooooooooooooooooooooooooooooooo as FooooooooooooooooooooooooooooooooBar } from "./here.slint";
"#,
    );
}

// cspell:enable

#[test]
/// format_import_identifier
fn import_comma_new_line() {
    // A trailing comma stays inline; only an input newline breaks the list.
    assert_formatting(
        r#"import {Foo,} from "./here.slint";"#,
        r#"import { Foo, } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {  Foo,} from "./here.slint";"#,
        r#"import { Foo, } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo,  } from "./here.slint";"#,
        r#"import { Foo, } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo as Fur,  } from "./here.slint";"#,
        r#"import { Foo as Fur, } from "./here.slint";
"#,
    );
}

#[test]
/// format_import_identifier
fn multiple_imports_behavior() {
    assert_formatting(
        r#"import {Foo, Bar} from "./here.slint";"#,
        r#"import { Foo, Bar } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo,Bar,} from "./here.slint";"#,
        r#"import { Foo, Bar, } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo,Bar  } from "./here.slint";"#,
        r#"import { Foo, Bar } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {Foo,Bar as BarBer } from "./here.slint";"#,
        r#"import { Foo, Bar as BarBer } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import { Foo, Bar} from "./here.slint";"#,
        r#"import { Foo, Bar } from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {
    Foo,
    Bar
} from "./here.slint";"#,
        r#"import {
    Foo,
    Bar
} from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {
    Foo as Fur,
    Bar
} from "./here.slint";"#,
        r#"import {
    Foo as Fur,
    Bar
} from "./here.slint";
"#,
    );
}

#[test]
fn import_new_line_with_comments() {
    assert_formatting(
        r#"import {
    Foo, // comment foo
    Bar   // comment bar
} from "./here.slint";"#,
        r#"import {
    Foo, // comment foo
    Bar // comment bar
} from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {
    Foo, // comment foo
    Bar,   // comment bar
} from "./here.slint";"#,
        r#"import {
    Foo, // comment foo
    Bar, // comment bar
} from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import {
    Foo, // comment foo
    Bar as BarBer   // comment bar
} from "./here.slint";"#,
        r#"import {
    Foo, // comment foo
    Bar as BarBer // comment bar
} from "./here.slint";
"#,
    );
}

#[test]
fn import_many() {
    assert_formatting(
        r#"import {
            Baz, Quz,
    Foo, // comment foo
    Bar as BarBer,   // comment bar
    Snaf,Tar, // comment
    Jar
} from "./here.slint";"#,
        r#"import {
    Baz,
    Quz,
    Foo, // comment foo
    Bar as BarBer, // comment bar
    Snaf,
    Tar, // comment
    Jar
} from "./here.slint";
"#,
    );

    assert_formatting(
        r#"import { Foo, Bar as BarBer, Quz, Baz, Snaf, Tatta, Tar, Jar } from "./here.slint";"#,
        r#"import { Foo, Bar as BarBer, Quz, Baz, Snaf, Tatta, Tar, Jar } from "./here.slint";
"#,
    );
}

// The remaining two tests are carried over from the query formatter's own
// suite: they cover leafed constructs the old formatter's tests never
// exercised, where a rule regression would silently corrupt code.

#[test]
fn rust_attr_interior_is_left_verbatim() {
    // The odd spacing around the colon inside `@rust-attr(...)` is
    // preserved (it is the opaque-Rust leaf), while everything outside the
    // leaf — the attribute punctuation and the struct field's colon —
    // takes the ruleset's formatting.
    assert_formatting(
        "@rust-attr(a : b)
struct S { foo :int }",
        "@rust-attr (a : b) struct S { foo: int }\n",
    );
}

#[test]
fn string_template_interpolation_is_left_verbatim() {
    // The colon of the ternary interpolated into the template stays
    // verbatim; the binding's own colon is respaced.
    assert_formatting(
        "component A { x :\"a\\{ c ? d : e }f\"; }",
        "component A { x: \"a\\{ c ? d : e }f\"; }\n",
    );
}
