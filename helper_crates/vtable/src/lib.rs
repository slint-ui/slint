extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::*;

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
    let module_name = quote::format_ident!("{}_vtable_mod", trait_name);
    let box_name = quote::format_ident!("{}Box", trait_name);
    let ref_name = quote::format_ident!("{}Ref", trait_name);
    let refmut_name = quote::format_ident!("{}RefMut", trait_name);
    let vtable_name = input.ident.clone();

    let ref_doc = format!("This is an equivalent to a `&'a dyn {}`", trait_name);
    let refmut_doc = format!("This is an equivalent to a `&'a mut dyn {}`", trait_name);
    let box_doc = format!("This is an equivalent to a `Box<dyn {}>`", trait_name);

    let mut box_impl = None;

    let mut generated_trait = ItemTrait {
        attrs: input
            .attrs
            .iter()
            .filter(|a| a.path.get_ident().as_ref().map(|i| *i == "repr").unwrap_or(false))
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

    let mut generated_to_fn_trait = vec![];
    let mut generated_to_fn_assoc = vec![];
    let mut generated_constructor = vec![];
    let mut vtable_ctor = vec![];

    for field in &mut fields.named {
        if let Type::BareFn(f) = &mut field.ty {
            let ident = field.ident.as_ref().unwrap();

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

                match &param.ty {
                    Type::Ptr(TypePtr { mutability, elem, .. })
                    | Type::Reference(TypeReference { mutability, elem, .. }) => {
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
                                    call_code = Some(quote!(vtable.as_ptr(),));
                                    continue;
                                } else if pointer_to == &impl_name {
                                    if sig.inputs.len() > 0 {
                                        return Error::new(p.span(), "Impl pointer need to be the first (with the exception of VTable)").to_compile_error().into();
                                    }
                                    sig.inputs.push(FnArg::Receiver(Receiver {
                                        attrs: param.attrs.clone(),
                                        reference: Some(Default::default()),
                                        mutability: mutability.clone(),
                                        self_token: Default::default(),
                                    }));
                                    call_code = Some(quote!(#call_code ptr.as_ptr(),));
                                    let const_or_mut = mutability
                                        .map(|x| quote!(#x))
                                        .unwrap_or_else(|| quote!(const));
                                    self_call = Some(
                                        quote!(&#mutability (*(#arg_name as *#const_or_mut T)), ),
                                    );
                                    has_self = true;
                                    continue;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                sig.inputs.push(typed_arg);
                call_code = Some(quote!(#call_code #arg_name,));
                forward_code = Some(quote!(#forward_code #arg_name,));
            }

            // Add unsafe
            f.unsafety = Some(Default::default());
            // Add extern "C" if it isn't there
            if let Some(a) = &f.abi {
                if !a.name.as_ref().map(|s| s.value() != "C").unwrap_or(false) {
                    return Error::new(a.span(), "invalid ABI").to_compile_error().into();
                }
            } else {
                f.abi = sig_extern.abi.clone();
            }
            // Remove pub, if any
            field.vis = Visibility::Inherited;

            let mut wrap_trait_call = None;
            if !has_self {
                sig.generics = Generics {
                    where_clause: Some(parse_str("where Self : Sized").unwrap()),
                    ..Default::default()
                };
                if let ReturnType::Type(_, ret) = &f.output {
                    if let Type::Path(ret) = &**ret {
                        if let Some(seg) = ret.path.segments.last() {
                            if let PathArguments::AngleBracketed(args) = &seg.arguments {
                                if let Some(GenericArgument::Type(Type::Path(arg))) =
                                    args.args.first()
                                {
                                    if let Some(arg) = arg.path.get_ident() {
                                        // that's quite a lot of if let to get the argument of the type
                                        if seg.ident == "Box" && arg == &impl_name {
                                            // Consider this is a constructor, so change Box<Self> to Self
                                            sig.output = parse_str("-> Self").unwrap();
                                            wrap_trait_call = Some(quote! {
                                                let wrap_trait_call = |x| Box::from_raw(Box::into_raw(Box::new(x)) as *mut #impl_name);
                                                wrap_trait_call
                                            });
                                        }
                                    }
                                }
                            }
                        }
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

                box_impl = Some(quote! {
                    #[doc = #box_doc]
                    pub struct #box_name {
                        inner: #to_name,
                    }
                    impl #box_name {
                        /// Construct the box from raw pointer of a vtable and a corresponding pointer
                        pub unsafe fn from_raw(
                            vtable: core::ptr::NonNull<#vtable_name>,
                            ptr: core::ptr::NonNull<#impl_name>,
                        ) -> Self {
                            Self{inner: #to_name{vtable, ptr}}
                        }
                    }
                    impl core::ops::Deref for #box_name {
                        type Target = dyn #trait_name;
                        fn deref(&self) -> &Self::Target {
                            &self.inner
                        }
                    }
                    impl core::ops::DerefMut for #box_name {
                        fn deref_mut(&mut self) -> &mut Self::Target {
                            &mut self.inner
                        }
                    }
                    impl core::ops::Drop for #box_name {
                        fn drop(&mut self) {
                            #[allow(unused)]
                            let (vtable, ptr) = (&self.inner.vtable, &self.inner.ptr);
                            // Safety: The vtable is valid and inner is a type corresponding to the vtable,
                            // which was allocated such that drop is expected.
                            unsafe { (vtable.as_ref().#ident)(#call_code) }
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
                block: parse(
                    if has_self {
                        quote!({
                            #[allow(unused)]
                            let (vtable, ptr) = (&self.vtable, &self.ptr);
                            // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                            unsafe { (vtable.as_ref().#ident)(#call_code) }
                        })
                    } else {
                        // This should never happen: nobody should be able to access the Trait Object directly.
                        quote!({ panic!("Calling Sized method on a Trait Object") })
                    }
                    .into(),
                )
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
                if wrap_trait_call.is_some() {
                    sig.output = parse(quote!(-> #box_name).into()).unwrap();
                    generated_constructor.push(ImplItemMethod {
                        attrs: vec![],
                        vis: generated_trait.vis.clone(),
                        defaultness: None,
                        sig,
                        block: parse(
                            quote!({
                                // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                                unsafe {
                                    #[allow(unused)]
                                    let vtable = core::ptr::NonNull::from(self);
                                    #box_name::from_raw(vtable, std::ptr::NonNull::from(Box::leak((self.#ident)(#call_code))))
                                }
                            })
                            .into(),
                        )
                        .unwrap(),
                    });
                } else {
                    generated_to_fn_assoc.push(ImplItemMethod {
                        attrs: vec![],
                        vis: generated_trait.vis.clone(),
                        defaultness: None,
                        sig,
                        block: parse(
                            quote!({
                                #[allow(unused_parens)]
                                #[allow(unused)]
                                let vtable = core::ptr::NonNull::from(self);
                                // Safety: this rely on the vtable being valid, and the ptr being a valid instance for this vtable
                                unsafe {  #wrap_trait_call((vtable.as_ref().#ident)(#call_code)) }
                            })
                            .into(),
                        )
                        .unwrap(),
                    });
                }
            }

            vtable_ctor.push(quote!(#ident: {
                #sig_extern {
                    #[allow(unused_parens)]
                    // This is safe since the self must be a instance of our type
                    unsafe {
                        #wrap_trait_call(T::#ident(#self_call #forward_code))
                    }
                }
                #ident::<T>
            },));
        } else {
            return Error::new(field.span(), "member must only be functions")
                .to_compile_error()
                .into();
        };
    }

    let vis = input.vis;
    input.vis = Visibility::Public(VisPublic { pub_token: Default::default() });

    let result = quote!(
        #[allow(non_snake_case)]
        /// This private module is generated by the `vtable` macro
        mod #module_name {
            #input

            impl #vtable_name {
                // unfortunately cannot be const in stable rust because of the bounds (depends on rfc 2632)
                pub /*const*/ fn new<T: #trait_name>() -> Self {
                    Self {
                        #(#vtable_ctor)*
                    }
                }

                #(#generated_constructor)*
                #(#generated_to_fn_assoc)*
            }

            #generated_trait
            pub struct #impl_name { _private: [u8; 0] }
            #[derive(Clone, Copy)]
            struct #to_name {
                vtable: core::ptr::NonNull<#vtable_name>,
                ptr: core::ptr::NonNull<#impl_name>,
            }
            impl #trait_name for #to_name { #(#generated_to_fn_trait)* }

            #[doc = #ref_doc]
            #[derive(Clone, Copy)]
            pub struct #ref_name<'a> {
                inner: #to_name,
                _phantom: core::marker::PhantomData<&'a #impl_name>,
            }
            impl<'a> core::ops::Deref for #ref_name<'a> {
                 type Target = dyn #trait_name;
                 fn deref(&self) -> &Self::Target {
                    &self.inner
                 }
            }

            #[doc = #refmut_doc]
            pub struct #refmut_name<'a> {
                inner: #to_name,
                _phantom: core::marker::PhantomData<&'a *mut #impl_name>,
            }
            impl<'a> core::ops::Deref for #refmut_name<'a> {
                type Target = dyn #trait_name;
                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }
            impl<'a> core::ops::DerefMut for #refmut_name<'a> {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.inner
                }
            }
            #box_impl
        }
        #[doc(inline)]
        #vis use #module_name::*;
    );
    //println!("{}", result);
    result.into()
}
