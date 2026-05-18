// FIXME: Double check we construct object properties in order.

use core::cell::RefCell;
use js_sys::{Array, Error, Function, JsString, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

trait JsValueExt {
    type Value;
    fn lift_error(self) -> Result<Self::Value, JsError>;
}

impl<T> JsValueExt for Result<T, JsValue> {
    type Value = T;

    fn lift_error(self) -> Result<Self::Value, JsError> {
        self.map_err(|err| {
            let message = match err.dyn_into::<Error>() {
                Ok(error) => error.message(),
                Err(value) => JsString::from(value),
            };
            JsError::new(&String::from(message))
        })
    }
}

#[cfg(feature = "node")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "global")]
    static GLOBAL: Object;
}

#[cfg(feature = "node")]
#[wasm_bindgen(module = "web-tree-sitter-wasm-bindgen")]
extern "C" {
    fn initialize_tree_sitter() -> Promise;
}

thread_local! {
    // Ensure `web-tree-sitter` is only initialized once
    static TREE_SITTER_INITIALIZED: RefCell<bool> = const { RefCell::new(false) };
}

pub struct TreeSitter;

impl TreeSitter {
    #[cfg(feature = "node")]
    pub async fn init() -> Result<(), JsError> {
        #![allow(non_snake_case)]

        // Exit early if `web-tree-sitter` is already initialized
        if TREE_SITTER_INITIALIZED.with(|cell| *cell.borrow()) {
            return Ok(());
        }

        JsFuture::from(initialize_tree_sitter())
            .await
            .lift_error()?;

        // Set `web-tree-sitter` to initialized
        TREE_SITTER_INITIALIZED.with(|cell| cell.replace(true));

        Ok(())
    }

    #[cfg(feature = "web-sys")]
    pub async fn init() -> Result<(), JsError> {
        #![allow(non_snake_case)]

        // Exit early if `web-tree-sitter` is already initialized
        if TREE_SITTER_INITIALIZED.with(|cell| *cell.borrow()) {
            return Ok(());
        }

        let scope = &web_sys::window().unwrap_throw();

        let tree_sitter = Reflect::get(scope, &"TreeSitter".into()).and_then(|property| {
            if property.is_undefined() {
                let message = "window.TreeSitter is not defined; load the tree-sitter javascript module first";
                Err(JsError::new(message).into())
            } else {
                Ok(property)
            }
        })
        .lift_error()?;

        // Call `init` from `web-tree-sitter` to initialize emscripten
        let init = Reflect::get(&tree_sitter, &"init".into())
            .lift_error()?
            .unchecked_into::<Function>();
        JsFuture::from(
            init.call0(&JsValue::UNDEFINED)
                .lift_error()?
                .unchecked_into::<Promise>(),
        )
        .await
        .lift_error()?;

        let Language = Reflect::get(&tree_sitter, &"Language".into()).lift_error()?;
        let Parser = tree_sitter;

        // Manually define `Parser` and `Language` in a fashion that works with wasm-bindgen
        Reflect::set(scope, &"Parser".into(), &Parser).lift_error()?;
        Reflect::set(scope, &"Language".into(), &Language).lift_error()?;

        // Set `web-tree-sitter` to initialized
        TREE_SITTER_INITIALIZED.with(|cell| cell.replace(true));

        Ok(())
    }

    fn init_guard() {
        if !TREE_SITTER_INITIALIZED.with(|cell| *cell.borrow()) {
            wasm_bindgen::throw_str("TreeSitter::init must be called to initialize the library");
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type Edit;

    // Instance Properties

    #[wasm_bindgen(method, getter, js_name = newEndIndex)]
    pub fn new_end_index(this: &Edit) -> u32;

    #[wasm_bindgen(method, getter, js_name = newEndPosition)]
    pub fn new_end_position(this: &Edit) -> Point;

    #[wasm_bindgen(method, getter, js_name = oldEndIndex)]
    pub fn old_end_index(this: &Edit) -> u32;

    #[wasm_bindgen(method, getter, js_name = oldEndPosition)]
    pub fn old_end_position(this: &Edit) -> Point;

    #[wasm_bindgen(method, getter, js_name = startIndex)]
    pub fn start_index(this: &Edit) -> u32;

    #[wasm_bindgen(method, getter, js_name = startPosition)]
    pub fn start_position(this: &Edit) -> Point;
}

impl Edit {
    pub fn new(
        start_index: u32,
        old_end_index: u32,
        new_end_index: u32,
        start_position: &Point,
        old_end_position: &Point,
        new_end_position: &Point,
    ) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"startIndex".into(), &start_index.into()).unwrap();
        Reflect::set(&obj, &"oldEndIndex".into(), &old_end_index.into()).unwrap();
        Reflect::set(&obj, &"newEndIndex".into(), &new_end_index.into()).unwrap();
        Reflect::set(&obj, &"startPosition".into(), &start_position.into()).unwrap();
        Reflect::set(&obj, &"oldEndPosition".into(), &old_end_position.into()).unwrap();
        Reflect::set(&obj, &"newEndPosition".into(), &new_end_position.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

impl Default for Edit {
    fn default() -> Self {
        let start_index = Default::default();
        let old_end_index = Default::default();
        let new_end_index = Default::default();
        let start_position = &Default::default();
        let old_end_position = &Default::default();
        let new_end_position = &Default::default();
        Self::new(
            start_index,
            old_end_index,
            new_end_index,
            start_position,
            old_end_position,
            new_end_position,
        )
    }
}

pub type Input = Function;

pub type InputClosureType = dyn FnMut(u32, Option<Point>, Option<u32>) -> Option<JsString>;

pub type Logger = Function;

pub type LoggerClosureType = dyn FnMut(JsString, LoggerParams, JsString);

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type LoggerParams;

    #[wasm_bindgen(method, indexing_getter)]
    fn get(this: &LoggerParams, key: &JsString) -> JsString;

    #[wasm_bindgen(method, indexing_setter)]
    fn set(this: &LoggerParams, val: &JsString);

    #[wasm_bindgen(method, indexing_deleter)]
    fn delete(this: &LoggerParams, val: &JsString);
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, PartialEq)]
    pub type Language;

    // Static Methods

    #[wasm_bindgen(static_method_of = Language, js_name = load)]
    fn __load_bytes(bytes: &Uint8Array) -> Promise;

    #[wasm_bindgen(static_method_of = Language, js_name = load)]
    fn __load_path(path: &str) -> Promise;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn version(this: &Language) -> u32;

    #[wasm_bindgen(method, getter, js_name = fieldCount)]
    pub fn field_count(this: &Language) -> u16;

    #[wasm_bindgen(method, getter, js_name = nodeTypeCount)]
    pub fn node_kind_count(this: &Language) -> u16;

    // Instance Methods

    #[wasm_bindgen(method, js_name = fieldNameForId)]
    pub fn field_name_for_id(this: &Language, field_id: u16) -> Option<String>;

    #[wasm_bindgen(method, js_name = fieldIdForName)]
    pub fn field_id_for_name(this: &Language, field_name: &str) -> Option<u16>;

    #[wasm_bindgen(method, js_name = idForNodeType)]
    pub fn id_for_node_kind(this: &Language, kind: &str, named: bool) -> u16;

    #[wasm_bindgen(method, js_name = nodeTypeForId)]
    pub fn node_kind_for_id(this: &Language, kind_id: u16) -> Option<String>;

    #[wasm_bindgen(method, js_name = nodeTypeIsNamed)]
    pub fn node_kind_is_named(this: &Language, kind_id: u16) -> bool;

    #[wasm_bindgen(method, js_name = nodeTypeIsVisible)]
    pub fn node_kind_is_visible(this: &Language, kind_id: u16) -> bool;

    #[wasm_bindgen(catch, method)]
    pub fn query(this: &Language, source: &JsString) -> Result<Query, QueryError>;
}

impl Language {
    pub async fn load_bytes(bytes: &Uint8Array) -> Result<Language, LanguageError> {
        TreeSitter::init_guard();
        JsFuture::from(Language::__load_bytes(bytes))
            .await
            .map(JsCast::unchecked_into)
            .map_err(JsCast::unchecked_into)
    }

    pub async fn load_path(path: &str) -> Result<Language, LanguageError> {
        TreeSitter::init_guard();
        JsFuture::from(Language::__load_path(path))
            .await
            .map(JsCast::unchecked_into)
            .map_err(JsCast::unchecked_into)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Error)]
    pub type LanguageError;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type ParseOptions;

    // Instance Properties

    // -> Range[]
    #[wasm_bindgen(method, getter, js_name = includedRanges)]
    pub fn included_ranges(this: &ParseOptions) -> Option<Array>;
}

impl ParseOptions {
    pub fn new(included_ranges: Option<&Array>) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"includedRanges".into(), &included_ranges.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        let included_ranges = Default::default();
        Self::new(included_ranges)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    #[wasm_bindgen(extends = Object)]
    pub type Point;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn column(this: &Point) -> u32;

    #[wasm_bindgen(method, getter)]
    pub fn row(this: &Point) -> u32;
}

impl Point {
    pub fn new(row: u32, column: u32) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"row".into(), &row.into()).unwrap();
        Reflect::set(&obj, &"column".into(), &column.into()).unwrap();
        JsCast::unchecked_into(obj)
    }

    #[inline(always)]
    fn spread(&self) -> (u32, u32) {
        (self.row(), self.column())
    }
}

impl Default for Point {
    fn default() -> Self {
        let row = Default::default();
        let column = Default::default();
        Self::new(row, column)
    }
}

impl Eq for Point {}

impl std::hash::Hash for Point {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let this = self.spread();
        this.hash(state);
    }
}

impl Ord for Point {
    fn cmp(&self, that: &Self) -> std::cmp::Ordering {
        let this = self.spread();
        let that = that.spread();
        this.cmp(&that)
    }
}

impl PartialEq for Point {
    fn eq(&self, that: &Self) -> bool {
        let this = self.spread();
        let that = that.spread();
        this.eq(&that)
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, that: &Point) -> Option<std::cmp::Ordering> {
        Some(self.cmp(that))
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type PredicateOperand;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn name(this: &PredicateOperand) -> JsString;

    #[wasm_bindgen(method, getter)]
    pub fn type_(this: &PredicateOperand) -> JsString;
}

impl PredicateOperand {
    pub fn new(type_: &JsString, name: &JsString) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"type".into(), &type_.into()).unwrap();
        Reflect::set(&obj, &"name".into(), &name.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

impl Default for PredicateOperand {
    fn default() -> Self {
        let name = &<String as Default>::default().into();
        let type_ = &<String as Default>::default().into();
        Self::new(name, type_)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type PredicateResult;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn operator(this: &PredicateResult) -> JsString;

    // -> PredicateOperand[]
    #[wasm_bindgen(method, getter)]
    pub fn operands(this: &PredicateResult) -> Box<[JsValue]>;
}

impl PredicateResult {
    pub fn new(operator: &JsString, operands: &Array) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"operator".into(), &operator.into()).unwrap();
        Reflect::set(&obj, &"operands".into(), &operands.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

impl Default for PredicateResult {
    fn default() -> Self {
        let operator = &<String as Default>::default().into();
        let operands = &<Vec<JsValue> as Default>::default().into_iter().collect();
        Self::new(operator, operands)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type Query;

    // Instance Properties

    // -> JsString[]
    #[wasm_bindgen(method, getter, js_name = captureNames)]
    pub fn capture_names(this: &Query) -> Box<[JsValue]>;

    // Instance Methods

    #[wasm_bindgen(method)]
    pub fn delete(this: &Query);

    // -> QueryMatch[]
    #[wasm_bindgen(method)]
    pub fn matches(
        this: &Query,
        node: &SyntaxNode,
        start_position: Option<&Point>,
        end_position: Option<&Point>,
    ) -> Box<[JsValue]>;

    // -> QueryCapture[]
    #[wasm_bindgen(method)]
    pub fn captures(
        this: &Query,
        node: &SyntaxNode,
        start_position: Option<&Point>,
        end_position: Option<&Point>,
    ) -> Box<[JsValue]>;

    // -> PredicateResult[]
    #[wasm_bindgen(method, js_name = predicatesForPattern)]
    pub fn predicates_for_pattern(this: &Query, pattern_index: usize) -> Box<[JsValue]>;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type QueryCapture;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn name(this: &QueryCapture) -> JsString;

    #[wasm_bindgen(method, getter)]
    pub fn node(this: &QueryCapture) -> SyntaxNode;
}

impl QueryCapture {
    pub fn new(name: &JsString, node: &SyntaxNode) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"name".into(), &name.into()).unwrap();
        Reflect::set(&obj, &"node".into(), &node.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Error)]
    pub type QueryError;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type QueryMatch;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn pattern(this: &QueryMatch) -> u32;

    // -> QueryCapture[]
    #[wasm_bindgen(method, getter)]
    pub fn captures(this: &QueryMatch) -> Box<[JsValue]>;
}

impl QueryMatch {
    pub fn new(pattern: u32, captures: &Array) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"pattern".into(), &pattern.into()).unwrap();
        Reflect::set(&obj, &"captures".into(), &captures.into()).unwrap();
        JsCast::unchecked_into(obj)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type QueryPredicate;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn operator(this: &QueryPredicate) -> JsString;

    // -> QueryPredicateArg[]
    #[wasm_bindgen(method, getter)]
    pub fn operands(this: &QueryPredicate) -> Box<[JsValue]>;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Object)]
    pub type QueryPredicateArg;

    // Instance Properties

    #[wasm_bindgen(method, getter)]
    pub fn value(this: &QueryPredicateArg) -> JsString;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    #[wasm_bindgen(extends = Object)]
    pub type Range;

    // Instance Properties

    #[wasm_bindgen(method, getter, js_name = endIndex)]
    pub fn end_index(this: &Range) -> u32;

    #[wasm_bindgen(method, getter, js_name = endPosition)]
    pub fn end_position(this: &Range) -> Point;

    #[wasm_bindgen(method, getter, js_name = startIndex)]
    pub fn start_index(this: &Range) -> u32;

    #[wasm_bindgen(method, getter, js_name = startPosition)]
    pub fn start_position(this: &Range) -> Point;
}

impl Range {
    pub fn new(
        start_position: &Point,
        end_position: &Point,
        start_index: u32,
        end_index: u32,
    ) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"startPosition".into(), &start_position.into()).unwrap();
        Reflect::set(&obj, &"endPosition".into(), &end_position.into()).unwrap();
        Reflect::set(&obj, &"startIndex".into(), &start_index.into()).unwrap();
        Reflect::set(&obj, &"endIndex".into(), &end_index.into()).unwrap();
        JsCast::unchecked_into(obj)
    }

    #[inline(always)]
    fn spread(&self) -> (u32, u32, Point, Point) {
        (
            self.start_index(),
            self.end_index(),
            self.start_position(),
            self.end_position(),
        )
    }
}

impl Default for Range {
    fn default() -> Range {
        let start_position = Default::default();
        let end_position = Default::default();
        let start_index = Default::default();
        let end_index = Default::default();
        Self::new(&start_position, &end_position, start_index, end_index)
    }
}

impl Eq for Range {}

impl std::hash::Hash for Range {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let this = self.spread();
        this.hash(state)
    }
}

impl Ord for Range {
    fn cmp(&self, that: &Self) -> std::cmp::Ordering {
        let this = self.spread();
        let that = that.spread();
        this.cmp(&that)
    }
}

impl PartialEq<Range> for Range {
    fn eq(&self, that: &Self) -> bool {
        let this = self.spread();
        let that = that.spread();
        this.eq(&that)
    }
}

impl PartialOrd<Range> for Range {
    fn partial_cmp(&self, that: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(that))
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type SyntaxNode;

    // Instance Properties

    #[wasm_bindgen(method, getter, js_name = childCount)]
    pub fn child_count(this: &SyntaxNode) -> u32;

    #[wasm_bindgen(method, getter)]
    pub fn children(this: &SyntaxNode) -> Box<[JsValue]>;

    #[wasm_bindgen(method, getter, js_name = endIndex)]
    pub fn end_index(this: &SyntaxNode) -> u32;

    #[wasm_bindgen(method, getter, js_name = endPosition)]
    pub fn end_position(this: &SyntaxNode) -> Point;

    #[wasm_bindgen(method, getter, js_name = firstChild)]
    pub fn first_child(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = firstNamedChild)]
    pub fn first_named_child(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter)]
    pub fn id(this: &SyntaxNode) -> u32;

    #[wasm_bindgen(method, getter, js_name = lastChild)]
    pub fn last_child(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = lastNamedChild)]
    pub fn last_named_child(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = namedChildCount)]
    pub fn named_child_count(this: &SyntaxNode) -> u32;

    #[wasm_bindgen(method, getter, js_name = namedChildren)]
    pub fn named_children(this: &SyntaxNode) -> Box<[JsValue]>;

    #[wasm_bindgen(method, getter, js_name = nextNamedSibling)]
    pub fn next_named_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = nextSibling)]
    pub fn next_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter)]
    pub fn parent(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = previousNamedSibling)]
    pub fn previous_named_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = previousSibling)]
    pub fn previous_sibling(this: &SyntaxNode) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, getter, js_name = startIndex)]
    pub fn start_index(this: &SyntaxNode) -> u32;

    #[wasm_bindgen(method, getter, js_name = startPosition)]
    pub fn start_position(this: &SyntaxNode) -> Point;

    #[wasm_bindgen(method, getter)]
    pub fn text(this: &SyntaxNode) -> JsString;

    #[wasm_bindgen(method, getter)]
    pub fn tree(this: &SyntaxNode) -> Tree;

    #[wasm_bindgen(method, getter, js_name = type)]
    pub fn type_(this: &SyntaxNode) -> JsString;

    #[wasm_bindgen(method, getter, js_name = typeId)]
    pub fn type_id(this: &SyntaxNode) -> u16;

    // Instance Methods

    #[wasm_bindgen(method)]
    pub fn child(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = childForFieldId)]
    pub fn child_for_field_id(this: &SyntaxNode, field_id: u16) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = childForFieldName)]
    pub fn child_for_field_name(this: &SyntaxNode, field_name: &str) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = descendantForIndex)]
    pub fn descendant_for_index(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = descendantForIndex)]
    pub fn descendant_for_index_range(
        this: &SyntaxNode,
        start_index: u32,
        end_index: u32,
    ) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = descendantForPosition)]
    pub fn descendant_for_position(this: &SyntaxNode, position: &Point) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = descendantForPosition)]
    pub fn descendant_for_position_range(
        this: &SyntaxNode,
        start_position: &Point,
        end_position: &Point,
    ) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = descendantsOfType)]
    pub fn descendants_of_type_array(
        this: &SyntaxNode,
        type_: Box<[JsValue]>,
        start_position: Option<&Point>,
        end_position: Option<&Point>,
    ) -> Box<[JsValue]>;

    // -> SyntaxNode[]
    #[wasm_bindgen(method, js_name = descendantsOfType)]
    pub fn descendants_of_type_string(
        this: &SyntaxNode,
        type_: &str,
        start_position: Option<&Point>,
        end_position: Option<&Point>,
    ) -> Box<[JsValue]>;

    #[wasm_bindgen(method)]
    pub fn equals(this: &SyntaxNode, other: &SyntaxNode) -> bool;

    #[wasm_bindgen(method, js_name = hasChanges)]
    pub fn has_changes(this: &SyntaxNode) -> bool;

    #[wasm_bindgen(method, js_name = hasError)]
    pub fn has_error(this: &SyntaxNode) -> bool;

    #[wasm_bindgen(method, js_name = isMissing)]
    pub fn is_missing(this: &SyntaxNode) -> bool;

    #[wasm_bindgen(method, js_name = isNamed)]
    pub fn is_named(this: &SyntaxNode) -> bool;

    #[wasm_bindgen(method, js_name = namedChild)]
    pub fn named_child(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = namedDescendantForIndex)]
    pub fn named_descendant_for_index(this: &SyntaxNode, index: u32) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = namedDescendantForIndex)]
    pub fn named_descendant_for_index_range(
        this: &SyntaxNode,
        start_index: u32,
        end_index: u32,
    ) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = namedDescendantForPosition)]
    pub fn named_descendant_for_position(this: &SyntaxNode, position: &Point)
    -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = namedDescendantForPosition)]
    pub fn named_descendant_for_position_range(
        this: &SyntaxNode,
        start_position: &Point,
        end_position: &Point,
    ) -> Option<SyntaxNode>;

    #[wasm_bindgen(method, js_name = toString)]
    pub fn to_string(this: &SyntaxNode) -> JsString;

    #[wasm_bindgen(method)]
    pub fn walk(this: &SyntaxNode) -> TreeCursor;
}

impl PartialEq<SyntaxNode> for SyntaxNode {
    fn eq(&self, other: &SyntaxNode) -> bool {
        self.equals(other)
    }
}

impl Eq for SyntaxNode {}

impl std::hash::Hash for SyntaxNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug)]
    pub type Tree;

    // Instance Properties

    #[wasm_bindgen(method, getter, js_name = rootNode)]
    pub fn root_node(this: &Tree) -> SyntaxNode;

    // Instance Methods

    #[wasm_bindgen(method)]
    pub fn copy(this: &Tree) -> Tree;

    #[wasm_bindgen(method)]
    pub fn delete(this: &Tree);

    #[wasm_bindgen(method)]
    pub fn edit(this: &Tree, delta: &Edit) -> Tree;

    #[wasm_bindgen(method)]
    pub fn walk(this: &Tree) -> TreeCursor;

    // -> Range[]
    #[wasm_bindgen(method, js_name = getChangedRanges)]
    pub fn get_changed_ranges(this: &Tree, other: &Tree) -> Box<[JsValue]>;

    #[wasm_bindgen(method, js_name = getLanguage)]
    pub fn get_language(this: &Tree) -> Language;
}

impl Clone for Tree {
    fn clone(&self) -> Tree {
        self.copy()
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type TreeCursor;

    // Instance Properties

    #[wasm_bindgen(method, getter, js_name = endIndex)]
    pub fn end_index(this: &TreeCursor) -> u32;

    #[wasm_bindgen(method, getter, js_name = endPosition)]
    pub fn end_position(this: &TreeCursor) -> Point;

    #[wasm_bindgen(method, getter, js_name = nodeIsNamed)]
    pub fn node_is_named(this: &TreeCursor) -> bool;

    #[wasm_bindgen(method, getter, js_name = nodeText)]
    pub fn node_text(this: &TreeCursor) -> JsString;

    #[wasm_bindgen(method, getter, js_name = nodeType)]
    pub fn node_type(this: &TreeCursor) -> JsString;

    #[wasm_bindgen(method, getter, js_name = startIndex)]
    pub fn start_index(this: &TreeCursor) -> u32;

    #[wasm_bindgen(method, getter, js_name = startPosition)]
    pub fn start_position(this: &TreeCursor) -> Point;

    // Instance Methods

    #[wasm_bindgen(method, js_name = currentFieldId)]
    pub fn current_field_id(this: &TreeCursor) -> Option<u16>;

    #[wasm_bindgen(method, js_name = currentFieldName)]
    pub fn current_field_name(this: &TreeCursor) -> Option<JsString>;

    #[wasm_bindgen(method, js_name = currentNode)]
    pub fn current_node(this: &TreeCursor) -> SyntaxNode;

    #[wasm_bindgen(method)]
    pub fn delete(this: &TreeCursor);

    #[wasm_bindgen(method, js_name = gotoFirstChild)]
    pub fn goto_first_child(this: &TreeCursor) -> bool;

    #[wasm_bindgen(method, js_name = gotoNextSibling)]
    pub fn goto_next_sibling(this: &TreeCursor) -> bool;

    #[wasm_bindgen(method, js_name = gotoParent)]
    pub fn goto_parent(this: &TreeCursor) -> bool;

    #[wasm_bindgen(method)]
    pub fn reset(this: &TreeCursor, node: &SyntaxNode);
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type Parser;

    // Static Methods

    // Constructor

    #[wasm_bindgen(catch, constructor)]
    fn __new() -> Result<Parser, ParserError>;

    // Instance Methods

    #[wasm_bindgen(method)]
    pub fn delete(this: &Parser);

    #[wasm_bindgen(method, js_name = getLanguage)]
    pub fn get_language(this: &Parser) -> Option<Language>;

    #[wasm_bindgen(method, js_name = getLogger)]
    pub fn get_logger(this: &Parser) -> Option<Logger>;

    #[wasm_bindgen(method, js_name = getTimeoutMicros)]
    pub fn get_timeout_micros(this: &Parser) -> f64;

    #[wasm_bindgen(catch, method, js_name = parse)]
    pub fn parse_with_function(
        this: &Parser,
        input: &Input,
        previous_tree: Option<&Tree>,
        options: Option<&ParseOptions>,
    ) -> Result<Option<Tree>, ParserError>;

    #[wasm_bindgen(catch, method, js_name = parse)]
    pub fn parse_with_string(
        this: &Parser,
        input: &JsString,
        previous_tree: Option<&Tree>,
        options: Option<&ParseOptions>,
    ) -> Result<Option<Tree>, ParserError>;

    #[wasm_bindgen(method)]
    pub fn reset(this: &Parser);

    #[wasm_bindgen(catch, method, js_name = setLanguage)]
    pub fn set_language(this: &Parser, language: Option<&Language>) -> Result<(), LanguageError>;

    #[wasm_bindgen(method, js_name = setLogger)]
    pub fn set_logger(this: &Parser, logger: Option<&Logger>);

    #[wasm_bindgen(method, js_name = setTimeoutMicros)]
    pub fn set_timeout_micros(this: &Parser, timeout_micros: f64);
}

impl Parser {
    pub fn new() -> Result<Parser, ParserError> {
        TreeSitter::init_guard();
        let result = Parser::__new()?;
        Ok(result)
    }
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug, Eq, PartialEq)]
    #[wasm_bindgen(extends = Error)]
    pub type ParserError;
}
