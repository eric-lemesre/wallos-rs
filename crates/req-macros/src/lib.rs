//! Proc-macros de traçabilité des exigences.

use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let crate_dir = std::path::Path::new(&manifest_dir);
    crate_dir
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn load_valid_ids() -> HashSet<String> {
    let lock_path = workspace_root().join("spec/requirements.lock.yaml");

    let content = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|e| panic!("cannot read {:?}: {}", lock_path, e));
    let docs = yaml_rust2::YamlLoader::load_from_str(&content)
        .unwrap_or_else(|e| panic!("cannot parse {:?}: {}", lock_path, e));
    let doc = &docs[0];
    let mut ids = HashSet::new();

    if let Some(reqs) = doc["requirements"].as_vec() {
        for req in reqs {
            if let Some(id) = req["id"].as_str() {
                let status = req["status"].as_str().unwrap_or("draft");
                if status == "deprecated" {
                    ids.insert(format!("DEPRECATED:{id}"));
                }
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

fn extract_id(args: TokenStream) -> String {
    let stream = proc_macro2::TokenStream::from(args);

    // Si le premier token est un littéral chaîne, on le parse directement.
    let mut iter = stream.clone().into_iter().peekable();
    if let Some(proc_macro2::TokenTree::Literal(_)) = iter.peek() {
        let lit: syn::Lit = syn::parse2(stream.clone())
            .expect("#[requirement(...)] attend un identifiant REQ-XXX-NNN");
        if let syn::Lit::Str(s) = lit {
            return s.value();
        }
    }

    // Sinon, on concatène tous les tokens jusqu'à la première virgule.
    let mut id = String::new();
    for tt in stream {
        if let proc_macro2::TokenTree::Punct(p) = &tt
            && p.as_char() == ','
        {
            break;
        }
        id.push_str(&tt.to_string());
    }
    if id.is_empty() {
        panic!("#[requirement(...)] attend un identifiant REQ-XXX-NNN");
    }
    id
}

fn validate_id(id: &str) {
    let valid = load_valid_ids();
    if valid.contains(&format!("DEPRECATED:{id}")) {
        panic!("l'exigence {id} est deprecated");
    }
    if !valid.contains(id) {
        panic!("exigence inconnue: {id}");
    }
}

/// Attache une fonction de production à une exigence et valide son existence.
#[proc_macro_attribute]
pub fn requirement(args: TokenStream, input: TokenStream) -> TokenStream {
    let id = extract_id(args);
    validate_id(&id);

    let input = parse_macro_input!(input as ItemFn);
    let expanded = quote! {
        #input
    };
    expanded.into()
}

/// Attache un test à une exigence.
#[proc_macro_attribute]
pub fn verifies(args: TokenStream, input: TokenStream) -> TokenStream {
    let id = extract_id(args);
    validate_id(&id);

    let input = parse_macro_input!(input as ItemFn);
    let expanded = quote! {
        #input
    };
    expanded.into()
}
