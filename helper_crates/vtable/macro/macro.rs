/*!
Implementation detail for the vtable crate
*/

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::*;

/// Returns true if the type `ty` is  "Container<Containee>"
fn match_generic_type(ty: &Type, container: &str, containee: &Ident) -> bool {
    if let Type::Path(pat) = ty {
        if let Some(seg) = pat.path.segments.last() {
            if seg.ident != container {
                return false;
            }
            if let PathArguments::AngleBracketed(args) = &seg.arguments {
                if let Some(GenericArgument::Type(Type::Path(arg))) = args.args.last() {
                    return Some(containee) == arg.path.get_ident();
                }
            }
        }
    }
    false
}

/**
This macro need to be applied to a VTable structure

The desing choice is that it is applied to a VTable and not to a trait so that cbindgen
can see the actual vtable struct.

The struct name of which the macro is applied needs to be ending with "VTable",
for example, if it is applied to `struct MyTraitVTable`, it will create:
 - The `MyTrait` trait with all the functions.
 - The `MyTraitConsts` trait for the associated constants, if any
 - `MyTraitVTable_static!` macro.

It will also expose type aliases for convinence
 - `type MyTraitRef<'a> = VRef<'a, MyTraitVTable>`
 - `type MyTraitRefMut<'a> = VRefMut<'a, MyTraitVTable>`
 - `type MyTraitBox = VBox<'a, MyTraitVTable>`

It will also implement the `VTableMeta` and `VTableMetaDrop` so that VRef and so on can work,
allowing to access methods dirrectly from vref.

This macro does the following transformation.

For fields whose type is a function:
 - The ABI is changed to `extern "C"`
 - `unsafe` is added to the signature, since it is unsafe to call these function directly from
  the vtable without having a valid pointer to the actual object. But if the original function was
  marked unsafe, the unsafety is forwared to the trait.
 - If a field is called `drop` it is understood that this is the destructor for a VBox
 - If the first argument of the function is `VRef<MyVTable>`  or `VRefMut<MyVTable>` this is
   understood as a `&self` or `&mut self` argument in the trait.

For the other fields
 - They are considered assotiated const of the MyTraitConsts
 - If they are annotated with he #[offset(FieldType)] attribute, the type of the field must be usize,
   and the const in the trait will be of type `FieldOffset<Self, FieldType>`

The VRef/VRefMut/VBox structure will dereference to a type which has the following associated items:
 - The functions from the vtable that have a VRef or VRefMut first parameter for self.
 - For each offset, a corresponding getter for this field of the same name that return a reference
   to this field, and also a mutable accessor that ends with `_mut` that returns a mutable reference.
 - `as_ptr` returns a `*mut u8`
 - `get_type` Return a reference to the VTable so one can access the associated consts.

The VTable structs gets additional associated items:
 - Functions without self parameter.
 - a `new` function that creates a vtable for any type that implements the generated traits.
*/
#[proc_macro_attribute]
pub fn vtable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);

    let fields = if let Fields::Named(fields) = &mut input.fields {
        fields
    } else {
        return Error::new(
            proc_macro2::Span::call_site(),
            "Only suported with structure with named fields",
        )
        .to_compile_error()
        .into();
    };

    let vtable_name = input.ident.to_string();
    if !vtable_name.ends_with("VTable") {
        return Error::new(input.ident.span(), "The structure does not ends in 'VTable'")
            .to_compile_error()
            .into();
    }

    let trait_name = Ident::new(&vtable_name[..vtable_name.len() - 6], input.ident.span());
    let to_name = quote::format_ident!("{}TO", trait_name);
    let module_name = quote::format_ident!("{}_vtable_mod", trait_name);
    let ref_name = quote::format_ident!("{}Ref", trait_name);
    let refmut_name = quote::format_ident!("{}RefMut", trait_name);
    let box_name = quote::format_ident!("{}Box", trait_name);
    let static_vtable_macro_name = quote::format_ident!("{}_static", vtable_name);

    let vtable_name = input.ident.clone();

    let mut drop_impl = None;

    let mut generated_trait = ItemTrait {
        attrs: input
            .attrs
            .iter()
            .filter(|a| a.path.get_ident().as_ref().map(|i| *i == "doc").unwrap_or(false))
            .cloned()
            .collect(),
        vis: Visibility::Public(VisPublic { pub_token: Default::default() }),
        unsafety: None,
        auto_token: None,
        trait_token: Default::default(),
        ident: trait_name.clone(),
        generics: Generics::default(),
        colon_token: None,
        supertraits: Default::default(),
        brace_token: Default::default(),
        items: Default::default(),
    };

    let additional_doc =
        format!("\nNote: Was generated from the `#[vtable]` macro on `{}`", vtable_name);
    generated_trait
        .attrs
        .append(&mut Attribute::parse_outer.parse2(quote!(#[doc = #additional_doc])).unwrap());

    let mut generated_trait_assoc_const = None;

    let mut generated_to_fn_trait = vec![];
    let mut generated_type_assoc_fn = vec![];
    let mut vtable_ctor = vec![];

    for field in &mut fields.named {
        // The vtable can only be accessed in unsafe code, so it is ok if all its fields are Public
        field.vis = Visibility::Public(VisPublic { pub_token: Default::default() });

        let ident = field.ident.as_ref().unwrap();
        let mut some = None;

        let func_ty = if let Type::BareFn(f) = &mut field.ty {
            Some(f)
        } else if let Type::Path(pat) = &mut field.ty {
            pat.path.segments.last_mut().and_then(|seg| {
                if seg.ident == "Option" {
                    some = Some(quote!(Some));
                    if let PathArguments::AngleBracketed(args) = &mut seg.arguments {
                        if let Some(GenericArgument::Type(Type::BareFn(f))) = args.args.first_mut()
                        {
                            Some(f)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            None
        };

        if let Some(f) = func_ty {
            let mut sig = Signature {
                constness: None,
                asyncness: None,
                unsafety: f.unsafety,
                abi: None,
                fn_token: f.fn_token.clone(),
                ident: ident.clone(),
                generics: Default::default(),
                paren_token: f.paren_token.clone(),
                inputs: Default::default(),
                variadic: None,
                output: f.output.clone(),
            };

            let mut sig_extern = sig.clone();
            sig_extern.abi = Some(parse_str("extern \"C\"").unwrap());
            sig_extern.generics = parse_str(&format!("<T : {}>", trait_name)).unwrap();

            // check parameters
            let mut call_code = None;
            let mut self_call = None;
            let mut forward_code = None;

            #[derive(Default)]
            struct SelfInfo {}

            let mut has_self = false;

            for param in &f.inputs {
                let arg_name = quote::format_ident!("_{}", sig_extern.inputs.len());
                let typed_arg = FnArg::Typed(PatType {
                    attrs: param.attrs.clone(),
                    pat: Box::new(Pat::Path(syn::PatPath {
                        attrs: Default::default(),
                        qself: None,
                        path: arg_name.clone().into(),
                    })),
                    colon_token: Default::default(),
                    ty: Box::new(param.ty.clone()),
                });
                sig_extern.inputs.push(typed_arg.clone());

                // check for the vtable
                if let Type::Ptr(TypePtr { mutability, elem, .. })
                | Type::Reference(TypeReference { mutability, elem, .. }) = &param.ty
                {
                    if let Type::Path(p) = &**elem {
                        if let Some(pointer_to) = p.path.get_ident() {
                            if pointer_to == &vtable_name {
                                if mutability.is_some() {
                                    return Error::new(p.span(), "VTable cannot be mutable")
                                        .to_compile_error()
                                        .into();
                                }
                                if call_code.is_some() || sig.inputs.len() > 0 {
                                    return Error::new(
                                        p.span(),
                                        "VTable pointer need to be the first",
                                    )
                                    .to_compile_error()
                                    .into();
                                }
                                call_code = Some(quote!(vtable as _,));
                                continue;
                            }
                        }
                    }
                }

                // check for self
                if let (true, mutability) = if match_generic_type(&param.ty, "VRef", &vtable_name) {
                    (true, None)
                } else if match_generic_type(&param.ty, "VRefMut", &vtable_name) {
                    (true, Some(Default::default()))
                } else {
                    (false, None)
                } {
                    if sig.inputs.len() > 0 {
                        return Error::new(param.span(), "Self pointer need to be the first")
                            .to_compile_error()
                            .into();
                    }
                    sig.inputs.push(FnArg::Receiver(Receiver {
                        attrs: param.attrs.clone(),
                        reference: Some(Default::default()),
                        mutability,
                        self_token: Default::default(),
                    }));
                    let self_ty = &param.ty;
                    let const_or_mut = mutability.map_or_else(|| quote!(const), |x| quote!(#x));
                    call_code =
                        Some(quote!(#call_code <#self_ty>::from_raw(self.vtable, self.ptr),));
                    self_call =
                        Some(quote!(&#mutability (*(#arg_name.as_ptr() as *#const_or_mut T)),));
                    has_self = true;
                    continue;
                }
                sig.inputs.push(typed_arg);
                call_code = Some(quote!(#call_code #arg_name,));
                forward_code = Some(quote!(#forward_code #arg_name,));
            }

            if has_self {
                // Add unsafe: The function are not safe to call unless the self parameter is of the correct type
                f.unsafety = Some(Default::default());
            }

            // Add extern "C" if it isn't there
            if let Some(a) = &f.abi {
                if !a.name.as_ref().map(|s| s.value() == "C").unwrap_or(false) {
                    return Error::new(a.span(), "invalid ABI").to_compile_error().into();
                }
            } else {
                f.abi = sig_extern.abi.clone();
            }

            let mut wrap_trait_call = None;
            if !has_self {
                sig.generics = Generics {
                    where_clause: Some(parse_str("where Self : Sized").unwrap()),
                    ..Default::default()
                };

                // Check if this is a constructor functions
                if let ReturnType::Type(_, ret) = &f.output {
                    if match_generic_type(&**ret, "VBox", &vtable_name) {
                        // Change VBox<VTable> to Self
                        sig.output = parse_str("-> Self").unwrap();
                        wrap_trait_call = Some(quote! {
                            let wrap_trait_call = |x| unsafe {
                                // Put the object on the heap and get a pointer to it
                                let ptr = core::ptr::NonNull::from(Box::leak(Box::new(x)));
                                VBox::<#vtable_name>::from_raw(vtable, ptr.cast())
                            };
                            wrap_trait_call
                        });
                    }
                }
            }

            if ident == "drop" {
                vtable_ctor.push(quote!(#ident: {
                    #sig_extern {
                        unsafe {
                            Box::from_raw((#self_call).0 as *mut _);
                        }
                    }
                    #ident::<T>
                },));

                drop_impl = Some(quote! {
                    impl VTableMetaDrop for #vtable_name {
                        unsafe fn drop(ptr: *mut #to_name) {
                            // Safety: The vtable is valid and inner is a type corresponding to the vtable,
                            // which was allocated such that drop is expected.
                            unsafe {
                                let ptr = &*ptr;
                                (ptr.vtable.as_ref().#ident)(VRefMut::from_raw(ptr.vtable, ptr.ptr)) }
                        }
                        fn new_box<X: HasStaticVTable<#vtable_name>>(value: X) -> VBox<#vtable_name> {
                            // Put the object on the heap and get a pointer to it
                            let ptr = core::ptr::NonNull::from(Box::leak(Box::new(value)));
                            unsafe { VBox::from_raw(core::ptr::NonNull::from(X::static_vtable()), ptr.cast()) }
                        }
                    }
                    pub type #box_name = VBox<#vtable_name>;
                });
                continue;
            }

            generated_trait.items.push(TraitItem::Method(TraitItemMethod {
                attrs: field.attrs.clone(),
                sig: sig.clone(),
                default: None,
                semi_token: Some(Default::default()),
            }));

            generated_to_fn_trait.push(ImplItemMethod {
                attrs: vec![],
                vis: Visibility::Public(VisPublic { pub_token: Default::default() }),
                defaultness: None,
                sig: sig.clone(),
                block: parse2(if has_self {
                    quote!({
                        // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                        unsafe {
                            let vtable = self.vtable.as_ref();
                            if let #some(func) = vtable.#ident {
                                func (#call_code)
                            } else {
                                panic!("Called a not-implemented method")
                            }
                        }
                    })
                } else {
                    // This should never happen: nobody should be able to access the Trait Object directly.
                    quote!({ panic!("Calling Sized method on a Trait Object") })
                })
                .unwrap(),
            });

            if !has_self {
                sig.inputs.insert(
                    0,
                    FnArg::Receiver(Receiver {
                        attrs: Default::default(),
                        reference: Some(Default::default()),
                        mutability: None,
                        self_token: Default::default(),
                    }),
                );
                sig.output = sig_extern.output.clone();
                generated_type_assoc_fn.push(ImplItemMethod {
                    attrs: vec![],
                    vis: generated_trait.vis.clone(),
                    defaultness: None,
                    sig,
                    block: parse2(quote!({
                        let vtable = self;
                        // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                        unsafe { (self.#ident)(#call_code) }
                    }))
                    .unwrap(),
                });

                vtable_ctor.push(quote!(#ident: {
                    #sig_extern {
                        // This is safe since the self must be a instance of our type
                        #[allow(unused)]
                        let vtable = unsafe { core::ptr::NonNull::from(&*_0) };
                        #wrap_trait_call(T::#ident(#self_call #forward_code))
                    }
                    #some(#ident::<T>)
                },));
            } else {
                vtable_ctor.push(quote!(#ident: {
                    #sig_extern {
                        // This is safe since the self must be a instance of our type
                        unsafe { T::#ident(#self_call #forward_code) }
                    }
                    #ident::<T>
                },));
            }
        } else {
            // associated constant

            let generated_trait_assoc_const =
                generated_trait_assoc_const.get_or_insert_with(|| ItemTrait {
                    attrs: Attribute::parse_outer.parse_str(&format!(
                        "/** Trait containing the associated constant relative to the the trait {}.\n{} */",
                        trait_name, additional_doc
                    )).unwrap(),
                    ident: quote::format_ident!("{}Consts", trait_name),
                    items: vec![],
                    ..generated_trait.clone()
                });

            let const_type = if let Some(o) = field
                .attrs
                .iter()
                .position(|a| a.path.get_ident().map(|a| a == "offset").unwrap_or(false))
            {
                let a = field.attrs.remove(o);
                let member_type = match parse2::<Type>(a.tokens) {
                    Err(e) => return e.to_compile_error().into(),
                    Ok(ty) => ty,
                };

                match &field.ty {
                    Type::Path(p) if p.path.get_ident().map(|i| i == "usize").unwrap_or(false) => {}
                    ty @ _ => {
                        return Error::new(
                            ty.span(),
                            "The type of an #[offset] member in the vtable must be 'usize'",
                        )
                        .to_compile_error()
                        .into()
                    }
                }

                // add `: Sized` to the trait in case it does not have it
                if generated_trait_assoc_const.supertraits.is_empty() {
                    generated_trait_assoc_const.colon_token = Some(Default::default());
                    generated_trait_assoc_const.supertraits.push(parse2(quote!(Sized)).unwrap());
                }

                let offset_type =
                    parse2::<Type>(quote!(vtable::FieldOffset<Self, #member_type>)).unwrap();

                vtable_ctor.push(quote!(#ident: T::#ident.get_byte_offset(),));

                let vis = &field.vis;
                generated_to_fn_trait.push(
                    parse2(quote! {
                        #vis fn #ident(&self) -> &#member_type {
                            unsafe {
                                &*(self.ptr.as_ptr().add(self.vtable.as_ref().#ident) as *const #member_type)
                            }
                        }
                    })
                    .unwrap(),
                );
                let ident_mut = quote::format_ident!("{}_mut", ident);
                generated_to_fn_trait.push(
                    parse2(quote! {
                        #vis fn #ident_mut(&mut self) -> &mut #member_type {
                            unsafe {
                                &mut *(self.ptr.as_ptr().add(self.vtable.as_ref().#ident) as *mut #member_type)
                            }
                        }
                    })
                    .unwrap(),
                );

                offset_type
            } else {
                vtable_ctor.push(quote!(#ident: T::#ident,));
                field.ty.clone()
            };

            generated_trait_assoc_const.items.push(TraitItem::Const(TraitItemConst {
                attrs: field.attrs.clone(),
                const_token: Default::default(),
                ident: ident.clone(),
                colon_token: Default::default(),
                ty: const_type,
                default: None,
                semi_token: Default::default(),
            }));
        };
    }

    let vis = input.vis;
    input.vis = Visibility::Public(VisPublic { pub_token: Default::default() });

    let new_trait_extra = generated_trait_assoc_const.as_ref().map(|x| {
        let i = &x.ident;
        quote!(+ #i)
    });

    let static_vtable_macro_doc = format!(
        r"Instentiate a static {vtable} for a given type and implements `vtable::HasStaticVTable<{vtable}>` for it.

```ignore
// The preview above is misleading because of rust-lang/rust#45939, so it is reproctuced bellow
macro_rules! {macro} {{
    ($(#[$meta:meta])* $vis:vis static $ident:ident for $ty:ty) => {{ ... }}
}}
```

Given a type `MyType` that implements the trait `{trait} {trait_extra}`,
create a static variable of type {vtable},
and implements HasStaticVTable for it.

```ignore
    struct Foo {{ ... }}
    impl {trait} for Foo {{ ... }}
    {macro}!(static FOO_VTABLE for Foo);
    // now VBox::new can be called
    let vbox = VBox::new(Foo{{ ... }});
```

        {extra}",
        vtable = vtable_name,
        trait = trait_name,
        trait_extra = new_trait_extra.as_ref().map(|x| x.to_string()).unwrap_or_default(),
        macro = static_vtable_macro_name,
        extra = additional_doc,
    );

    let result = quote!(
        #[allow(non_snake_case)]
        #[macro_use]
        /// This private module is generated by the `vtable` macro
        mod #module_name {
            #![allow(unused_parens)]
            #[allow(unused)]
            use super::*;
            use ::vtable::*;
            use ::std::boxed::Box; // make sure `Box` was not overriden in super
            #input

            impl #vtable_name {
                // unfortunately cannot be const in stable rust because of the bounds (depends on rfc 2632)
                pub /*const*/ fn new<T: #trait_name #new_trait_extra>() -> Self {
                    Self {
                        #(#vtable_ctor)*
                    }
                }
                #(#generated_type_assoc_fn)*
            }

            #generated_trait
            #generated_trait_assoc_const

            /// Invariant, same as vtable::Inner: vtable and ptr has to be valid and ptr an instance macthcin the vtable
            #[doc(hidden)]
            #[repr(C)]
            pub struct #to_name {
                vtable: core::ptr::NonNull<#vtable_name>,
                ptr: core::ptr::NonNull<u8>,
            }
            impl #to_name {
                #(#generated_to_fn_trait)*

                pub fn get_vtable(&self) -> &#vtable_name {
                    unsafe { self.vtable.as_ref() }
                }

                pub fn as_ptr(&self) -> *const u8 {
                    self.ptr.as_ptr()
                }
            }

            unsafe impl VTableMeta for #vtable_name {
                type VTable = #vtable_name;
                type Target = #to_name;
            }

            #drop_impl

            pub type #ref_name<'a> = VRef<'a, #vtable_name>;
            pub type #refmut_name<'a> = VRefMut<'a, #vtable_name>;

        }
        #[doc(inline)]
        #[macro_use]
        #vis use #module_name::*;

        #[macro_export]
        #[doc = #static_vtable_macro_doc]
        macro_rules! #static_vtable_macro_name {
            ($(#[$meta:meta])* $vis:vis static $ident:ident for $ty:ty) => {
                $(#[$meta])* $vis static $ident : #vtable_name = {
                    use vtable::*;
                    type T = $ty;
                    #vtable_name {
                        #(#vtable_ctor)*
                    }
                };
                unsafe impl vtable::HasStaticVTable<#vtable_name> for $ty {
                    fn static_vtable() -> &'static #vtable_name {
                        &$ident
                    }
                }
            }
        }
    );
    //     println!("{}", result);
    result.into()
}
