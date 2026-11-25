//! Node-API (napi-rs v2) bindings.
//!
//! Lexing is fast enough to run on the calling thread: a megabyte takes a
//! few milliseconds, and a long text is lexed on every core.

use napi::bindgen_prelude::*;
use napi::{JsObject, JsTypedArray, TypedArrayType};
use napi_derive::napi;

use crate::describe::{catalog_json, Described};

/// The catalog as a JSON array: the kind, the group and a summary of each
/// matcher, in the order they are to be tried.
#[napi]
pub fn catalog() -> String {
    catalog_json()
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

    /// The tokens of a text: for each, the place of its kind in `kinds()`
    /// and its end as an index into the JavaScript string.
    ///
    /// The arrays are made by the JavaScript engine and filled here. An
    /// array that Rust owns is freed by a finalizer, which runs only when
    /// the event loop turns, and the engine does not count its size: a
    /// loop that lexes many texts without a pause would keep them all.
    #[napi(ts_return_type = "{ kinds: Uint16Array; ends: Uint32Array }")]
    pub fn tokens(&self, env: Env, text: String) -> Result<JsObject> {
        let arrays = self.described.tokens(&text);
        let mut tokens = env.create_object()?;
        tokens.set_named_property(
            "kinds",
            typed_array(&env, &arrays.kinds, TypedArrayType::Uint16)?,
        )?;
        tokens.set_named_property(
            "ends",
            typed_array(&env, &arrays.ends, TypedArrayType::Uint32)?,
        )?;
        Ok(tokens)
    }
}

/// A typed array of the JavaScript engine's own, holding a copy of `values`.
fn typed_array<T: Copy>(env: &Env, values: &[T], kind: TypedArrayType) -> Result<JsTypedArray> {
    let bytes = std::mem::size_of_val(values);
    let mut buffer = env.create_arraybuffer(bytes)?;
    // An empty buffer may have no address at all.
    if bytes > 0 {
        // SAFETY: `values` is `bytes` long, the buffer was made with that
        // length, and the two do not overlap: the engine has just
        // allocated it.
        unsafe {
            std::ptr::copy_nonoverlapping(values.as_ptr().cast::<u8>(), buffer.as_mut_ptr(), bytes);
        }
    }
    buffer.into_raw().into_typedarray(kind, values.len(), 0)
}
