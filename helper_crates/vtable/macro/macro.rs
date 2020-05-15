/*!




*/

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
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
    let impl_name = quote::format_ident!("{}Impl", trait_name);
    let type_name = quote::format_ident!("{}Type", trait_name);
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
    let mut generated_trait_assoc_const = None;

    let mut generated_to_fn_trait = vec![];
    let mut generated_type_assoc_fn = vec![];
    let mut vtable_ctor = vec![];

    for field in &mut fields.named {
        // The vtable can only be accessed in unsafe code, so it is ok if all its fields are Public
        field.vis = Visibility::Public(VisPublic { pub_token: Default::default() });

        let ident = field.ident.as_ref().unwrap();
        if let Type::BareFn(f) = &mut field.ty {
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
                                call_code = Some(quote!(self.vtable.as_ptr(),));
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
                    call_code = Some(quote!(#call_code <#self_ty>::from_inner(*self),));
                    self_call = Some(
                        quote!(&#mutability (*(<#self_ty>::get_ptr(&#arg_name).as_ptr() as *#const_or_mut T)),),
                    );
                    has_self = true;
                    continue;
                }
                sig.inputs.push(typed_arg);
                call_code = Some(quote!(#call_code #arg_name,));
                forward_code = Some(quote!(#forward_code #arg_name,));
            }

            // Add unsafe
            f.unsafety = Some(Default::default());
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
                            let wrap_trait_call = |x| {
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
                        unsafe fn drop(ptr: #to_name) {
                            // Safety: The vtable is valid and inner is a type corresponding to the vtable,
                            // which was allocated such that drop is expected.
                            unsafe { (ptr.vtable.as_ref().#ident)(VRefMut::from_inner(ptr)) }
                        }
                    }

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
                vis: Visibility::Inherited,
                defaultness: None,
                sig: sig.clone(),
                block: parse2(if has_self {
                    quote!({
                        // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                        unsafe { (self.vtable.as_ref().#ident)(#call_code) }
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
                        // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                        unsafe { (self.vtable.as_ref().#ident)(#call_code) }
                    }))
                    .unwrap(),
                });

                vtable_ctor.push(quote!(#ident: {
                    #sig_extern {
                        #[allow(unused_parens)]
                        // This is safe since the self must be a instance of our type
                        unsafe {
                            #[allow(unused)]
                            let vtable = core::ptr::NonNull::new_unchecked(_0 as *mut #vtable_name);
                            #wrap_trait_call(T::#ident(#self_call #forward_code))
                        }
                    }
                    #ident::<T>
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
            let ty = &field.ty;

            let generated_trait_assoc_const =
                generated_trait_assoc_const.get_or_insert_with(|| ItemTrait {
                    attrs: vec![],
                    ident: quote::format_ident!("{}Consts", trait_name),
                    items: vec![],
                    ..generated_trait.clone()
                });
            generated_trait_assoc_const.items.push(TraitItem::Const(TraitItemConst {
                attrs: field.attrs.clone(),
                const_token: Default::default(),
                ident: ident.clone(),
                colon_token: Default::default(),
                ty: ty.clone(),
                default: None,
                semi_token: Default::default(),
            }));

            vtable_ctor.push(quote!(#ident: T::#ident,));

            generated_type_assoc_fn.push(
                parse2(quote! {
                    pub fn #ident(&self) -> #ty  {
                        // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                        unsafe { self.vtable.as_ref().#ident }
                    }
                })
                .unwrap(),
            );
        };
    }

    let vis = input.vis;
    input.vis = Visibility::Public(VisPublic { pub_token: Default::default() });

    let new_trait_extra = generated_trait_assoc_const.as_ref().map(|x| {
        let i = &x.ident;
        quote!(+ #i)
    });

    let result = quote!(
        #[allow(non_snake_case)]
        /// This private module is generated by the `vtable` macro
        mod #module_name {
            #[allow(unused)]
            use super::*;
            use ::vtable::*;
            #input

            impl #vtable_name {
                // unfortunately cannot be const in stable rust because of the bounds (depends on rfc 2632)
                pub /*const*/ fn new<T: #trait_name #new_trait_extra>() -> Self {
                    Self {
                        #(#vtable_ctor)*
                    }
                }
            }

            #generated_trait
            #generated_trait_assoc_const

            struct #impl_name { _private: [u8; 0] }

            /// This structure is highly unsafe, as it just has pointers. One could call trait functions
            /// directly.  However, it should not be possible, in safe code, to construct or to obtain a reference
            /// to this structure, as it cannot be constructed safely. And none of the safe api allow accessing
            /// a reference or a copy of this structure
            #[doc(hidden)]
            #[derive(Clone, Copy)]
            #[repr(C)]
            pub struct #to_name {
                vtable: core::ptr::NonNull<#vtable_name>,
                ptr: core::ptr::NonNull<#impl_name>,
            }
            impl #trait_name for #to_name { #(#generated_to_fn_trait)* }

            #[repr(transparent)]
            /// Safe wrapper around a VTable.
            pub struct #type_name {
                vtable: core::ptr::NonNull<#vtable_name>
            }
            impl #type_name {
                pub const unsafe fn from_raw(vtable: core::ptr::NonNull<#vtable_name>) -> Self {
                     Self { vtable }
                }
                #(#generated_type_assoc_fn)*
            }
            unsafe impl core::marker::Sync for #type_name {}

            unsafe impl VTableMeta for #vtable_name {
                type Trait = dyn #trait_name;
                type VTable = #vtable_name;
                type TraitObject = #to_name;
                type Type = #type_name;
                #[inline]
                unsafe fn map_to(from: &Self::TraitObject) -> &Self::Trait { from }
                #[inline]
                unsafe fn map_to_mut(from: &mut Self::TraitObject) -> &mut Self::Trait { from }
                #[inline]
                unsafe fn get_ptr(from: &Self::TraitObject) -> core::ptr::NonNull<u8> { from.ptr.cast() }
                #[inline]
                unsafe fn get_vtable(from: &Self::TraitObject) -> core::ptr::NonNull<Self::VTable> { from.vtable }
                #[inline]
                unsafe fn from_raw(vtable: core::ptr::NonNull<Self::VTable>, ptr: core::ptr::NonNull<u8>) -> Self::TraitObject
                { #to_name { vtable, ptr : ptr.cast() } }
                #[inline]
                unsafe fn get_type(from: &Self::TraitObject) -> Self::Type { #type_name::from_raw(from.vtable) }
            }

            #drop_impl

            pub type #ref_name<'a> = VRef<'a, #vtable_name>;
            pub type #refmut_name<'a> = VRefMut<'a, #vtable_name>;
            pub type #box_name = VBox<#vtable_name>;

            #[macro_export]
            macro_rules! #static_vtable_macro_name {
                ($ty:ty) => {
                    {
                        type T = $ty;
                        static VTABLE : #vtable_name = #vtable_name {
                            #(#vtable_ctor)*
                        };
                        unsafe {
                            <#vtable_name as ::vtable::VTableMeta>::Type::from_raw(
                                core::ptr::NonNull::new_unchecked(&VTABLE as *const _ as *mut #vtable_name)
                            )
                        }
                    }
                }
            }

        }
        #[doc(inline)]
        #vis use #module_name::*;
    );
    //println!("{}", result);
    result.into()
}
