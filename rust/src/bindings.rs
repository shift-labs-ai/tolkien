//! Node-API (napi-rs v2) bindings.
//!
//! Lexing is fast enough to run on the calling thread: a megabyte takes a
//! few milliseconds, and a long text is lexed on every core.

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::describe::{catalog_json, Described};

/// The catalog as a JSON array: the kind, the group and a summary of each
/// matcher, in the order they are to be tried.
#[napi]
pub fn catalog() -> String {
    catalog_json()
}

/// The tokens of a text: for each, the place of its kind in
/// `Lexer.kinds()` and its end as an index into the JavaScript string.
#[napi(object)]
pub struct TokenArrays {
    pub kinds: Uint16Array,
    pub ends: Uint32Array,
}

#[napi(js_name = "Lexer")]
pub struct JsLexer {
    described: Described,
}

#[napi]
impl JsLexer {
    /// The lexer of the built-in matchers, with the matchers `matchers`
    /// describes tried before them: a JSON array.
    #[napi(constructor)]
    pub fn new(matchers: String) -> Result<Self> {
        let described = Described::new(&matchers).map_err(Error::from_reason)?;
        Ok(Self { described })
    }

    #[napi]
    pub fn kinds(&self) -> Vec<String> {
        self.described.kinds()
    }

    #[napi]
    pub fn tokens(&self, text: String) -> TokenArrays {
        let arrays = self.described.tokens(&text);
        TokenArrays {
            kinds: Uint16Array::new(arrays.kinds),
            ends: Uint32Array::new(arrays.ends),
        }
    }
}
