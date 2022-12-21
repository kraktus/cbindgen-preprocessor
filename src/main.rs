use std::{fs::File, io::Read, path::PathBuf};

use clap::{ArgAction, Parser};
use syn::parse::{Parse, ParseStream};
use syn::visit::{Visit, self};
use syn::{
    parse_quote,
    visit_mut::{self, VisitMut},
    Attribute, Data, Expr, ItemEnum, ItemStruct, Lit, LitInt, Type, Visibility,
};
use syn::{ItemFn, Signature, Abi};

use quote::{quote, format_ident};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(default_value = "src/")]
    dir: PathBuf,
}

// add #[repr(C)]
// to all public types
struct AddReprC;

struct Attrs(pub Vec<Attribute>);

impl Parse for Attrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self(input.call(Attribute::parse_outer)?))
    }
}

fn outer_attr(input: &str) -> Attribute {
    let mut attrs: Attrs = syn::parse_str(input).unwrap();
    assert_eq!(attrs.0.len(), 1);
    attrs.0.remove(0)
}

impl VisitMut for AddReprC {
    fn visit_item_enum_mut(&mut self, enum_: &mut ItemEnum) {
        if matches!(enum_.vis, Visibility::Public(_))
            && enum_.attrs.iter().all(|a| !a.path.is_ident("repr"))
        {
            enum_.attrs.push(outer_attr("#[repr(C)]"));
        }
        visit_mut::visit_item_enum_mut(self, enum_);
    }

    fn visit_item_struct_mut(&mut self, struct_: &mut ItemStruct) {
        if matches!(struct_.vis, Visibility::Public(_))
            && struct_.attrs.iter().all(|a| !a.path.is_ident("repr"))
        {
            struct_.attrs.push(outer_attr("#[repr(C)]"));
        }
        visit_mut::visit_item_struct_mut(self, struct_);
    }
}
// for each file, add at the end of it externalised fn
// regular `pub fn foo(arg1: X, arg2: &Y) -> bool`
// are converted to `#[no_mangle] pub extern "C" fn foo_ffi(arg1: X, arg2: &Y) -> bool`
// method `pub fn foo_method(&self,arg1: X, arg2: &Y) -> bool`
// are converted to `#[no_mangle] pub extern "C" fn foo_method_ffi(&self: Foo,arg1: X, arg2: &Y) -> bool`
struct ExternaliseFn {
    externalised_fn_buf: Vec<ItemFn>,
}

impl<'ast> Visit<'ast> for ExternaliseFn {
    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        if item_fn.sig.asyncness.is_none()
            && item_fn.sig.abi.is_none()
            && matches!(item_fn.vis, Visibility::Public(_))
            && item_fn.attrs.is_empty() // let's start simple
        {
            let mut extern_fn = item_fn.clone();
            extern_fn.attrs.push(outer_attr("#[no_mangle]"));
            let extern_c_abi: Abi = syn::parse_str(r#"extern "C""#).unwrap();
            extern_fn.sig.abi = Some(extern_c_abi);
            extern_fn.sig.ident = format_ident!("{}_ffi", extern_fn.sig.ident);

            self.externalised_fn_buf.push(extern_fn);
        }
        visit::visit_item_fn(self, item_fn);
    }
}

fn main() {
    let args = Cli::parse();
    let entries = args.dir.read_dir().expect("read_dir call failed");
    for entry_res in entries {
        let entry = entry_res.unwrap();
        if entry.file_type().expect("file_type failed").is_file() {
            let mut file = File::open(entry.path()).expect("reading file in src/ failed");
            let mut src = String::new();
            file.read_to_string(&mut src).expect("Unable to read file");
            let mut parsed_file = syn::parse_file(&src).expect("Unable to parse file");
            AddReprC.visit_file_mut(&mut parsed_file);
            println!("{}", quote!(#parsed_file))
        }
    }
}
