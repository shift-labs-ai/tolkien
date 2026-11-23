//! WebAssembly (wasm-bindgen) bindings.
//!
//! The same API as the Node addon over the same engine. Threads cannot be
//! started here, so a text is lexed on the calling thread: run a long one
//! in a Web Worker to keep a page responsive.

use wasm_bindgen::prelude::*;

use crate::describe::{catalog_json, Described, TokenArrays};

/// The catalog as a JSON array: the kind, the group and a summary of each
/// matcher, in the order they are to be tried.
#[wasm_bindgen]
pub fn catalog() -> String {
    catalog_json()
}

/// The tokens of a text: for each, the place of its kind in
/// `Lexer.kinds()` and its end as an index into the JavaScript string.
#[wasm_bindgen(js_name = "TokenArrays")]
pub struct JsTokenArrays {
    arrays: TokenArrays,
}

#[wasm_bindgen(js_class = "TokenArrays")]
impl JsTokenArrays {
    #[wasm_bindgen(getter)]
    pub fn kinds(&self) -> Vec<u16> {
        self.arrays.kinds.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn ends(&self) -> Vec<u32> {
        self.arrays.ends.clone()
    }
}

#[wasm_bindgen(js_name = "Lexer")]
pub struct JsLexer {
    described: Described,
}

#[wasm_bindgen(js_class = "Lexer")]
impl JsLexer {
    /// The lexer of the built-in matchers, with the matchers `matchers`
    /// describes tried before them: a JSON array.
    #[wasm_bindgen(constructor)]
    pub fn new(matchers: &str) -> Result<JsLexer, JsError> {
        let described = Described::new(matchers).map_err(|reason| JsError::new(&reason))?;
        Ok(Self { described })
    }

    pub fn kinds(&self) -> Vec<String> {
        self.described.kinds()
    }

    pub fn tokens(&self, text: &str) -> JsTokenArrays {
        JsTokenArrays {
            arrays: self.described.tokens(text),
        }
    }
}
