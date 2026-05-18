use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    os::raw::c_char,
    ptr,
};

use memory_core::{MemoryEngine, NewMemory, RecallQuery};

#[allow(non_camel_case_types)]
pub struct memory_engine_t {
    engine: MemoryEngine,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> =
        RefCell::new(CString::new("no error").expect("static error string is valid"));
}

#[no_mangle]
/// # Safety
///
/// `path` must be a valid, nul-terminated UTF-8 string pointer.
pub unsafe extern "C" fn memory_engine_open(path: *const c_char) -> *mut memory_engine_t {
    clear_error();

    let result = (|| {
        let path = c_string(path, "path")?;
        let engine = MemoryEngine::open_default(path)?;
        Ok::<_, memory_core::MemoryError>(Box::into_raw(Box::new(memory_engine_t { engine })))
    })();

    match result {
        Ok(handle) => handle,
        Err(err) => {
            set_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a pointer returned by `memory_engine_open` and must not be freed more than
/// once.
pub unsafe extern "C" fn memory_engine_free(engine: *mut memory_engine_t) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a valid handle. `content` must be a valid, nul-terminated UTF-8 string.
/// `kind` and `scope` may be null or valid, nul-terminated UTF-8 strings.
pub unsafe extern "C" fn memory_engine_remember(
    engine: *mut memory_engine_t,
    content: *const c_char,
    kind: *const c_char,
    scope: *const c_char,
    importance: f32,
) -> i32 {
    clear_error();

    let result = (|| {
        let engine = engine_ref(engine)?;
        let content = c_string(content, "content")?;
        let kind = optional_c_string(kind).unwrap_or_else(|| "note".to_string());
        let scope = optional_c_string(scope).unwrap_or_else(|| "default".to_string());

        engine.engine.remember(
            NewMemory::new(content)
                .try_kind(kind)?
                .scope(scope)
                .importance(importance),
        )?;

        Ok::<_, memory_core::MemoryError>(())
    })();

    match result {
        Ok(()) => 0,
        Err(err) => {
            set_error(err);
            -1
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a valid handle. `query` must be a valid, nul-terminated UTF-8 string.
/// `scope` may be null or a valid, nul-terminated UTF-8 string. The returned pointer must be
/// released with `memory_string_free`.
pub unsafe extern "C" fn memory_engine_recall_json(
    engine: *mut memory_engine_t,
    query: *const c_char,
    scope: *const c_char,
    limit: usize,
) -> *mut c_char {
    clear_error();

    let result = (|| {
        let engine = engine_ref(engine)?;
        let query = c_string(query, "query")?;
        let mut recall = RecallQuery::new(query).limit(limit.max(1));

        if let Some(scope) = optional_c_string(scope) {
            if !scope.trim().is_empty() {
                recall = recall.scope(scope);
            }
        }

        let memories = engine.engine.recall(recall)?;
        let json = serde_json::to_string(&memories)?;
        Ok::<_, memory_core::MemoryError>(owned_string(json))
    })();

    match result {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a valid handle. `query` must be a valid, nul-terminated UTF-8 string.
/// `scope` may be null or a valid, nul-terminated UTF-8 string. The returned pointer must be
/// released with `memory_string_free`.
pub unsafe extern "C" fn memory_engine_context(
    engine: *mut memory_engine_t,
    query: *const c_char,
    scope: *const c_char,
    limit: usize,
    token_budget: usize,
) -> *mut c_char {
    clear_error();

    let result = (|| {
        let engine = engine_ref(engine)?;
        let query = c_string(query, "query")?;
        let mut recall = RecallQuery::new(query).limit(limit.max(1));

        if let Some(scope) = optional_c_string(scope) {
            if !scope.trim().is_empty() {
                recall = recall.scope(scope);
            }
        }

        let context = engine.engine.context(recall, token_budget.max(64))?;
        Ok::<_, memory_core::MemoryError>(owned_string(context.text))
    })();

    match result {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a valid handle. `id` must be a valid, nul-terminated UTF-8 string.
pub unsafe extern "C" fn memory_engine_delete(
    engine: *mut memory_engine_t,
    id: *const c_char,
) -> i32 {
    clear_error();

    let result = (|| {
        let engine = engine_ref(engine)?;
        let id = c_string(id, "id")?;
        let deleted = engine.engine.delete(&id)?;
        Ok::<_, memory_core::MemoryError>(if deleted { 1 } else { 0 })
    })();

    match result {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            -1
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `engine` must be a valid handle. The returned pointer must be released with
/// `memory_string_free`.
pub unsafe extern "C" fn memory_engine_stats_json(engine: *mut memory_engine_t) -> *mut c_char {
    clear_error();

    let result = (|| {
        let engine = engine_ref(engine)?;
        let stats = engine.engine.stats()?;
        let json = serde_json::to_string(&stats)?;
        Ok::<_, memory_core::MemoryError>(owned_string(json))
    })();

    match result {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn memory_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[no_mangle]
/// # Safety
///
/// `value` must be a pointer returned by memory.cpp, and it must not be freed more than once.
pub unsafe extern "C" fn memory_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

unsafe fn engine_ref<'a>(engine: *mut memory_engine_t) -> memory_core::Result<&'a memory_engine_t> {
    engine
        .as_ref()
        .ok_or_else(|| memory_core::MemoryError::InvalidInput("engine pointer is null".to_string()))
}

unsafe fn c_string(ptr: *const c_char, name: &str) -> memory_core::Result<String> {
    if ptr.is_null() {
        return Err(memory_core::MemoryError::InvalidInput(format!(
            "{name} pointer is null"
        )));
    }

    CStr::from_ptr(ptr)
        .to_str()
        .map(|value| value.to_string())
        .map_err(|err| memory_core::MemoryError::InvalidInput(err.to_string()))
}

unsafe fn optional_c_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    CStr::from_ptr(ptr)
        .to_str()
        .ok()
        .map(|value| value.to_string())
}

fn owned_string(value: String) -> *mut c_char {
    let sanitized = value.replace('\0', "\\0");
    CString::new(sanitized)
        .expect("sanitized string does not contain nul")
        .into_raw()
}

fn clear_error() {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = CString::new("no error").expect("static error string is valid");
    });
}

fn set_error(error: impl std::fmt::Display) {
    let message = error.to_string().replace('\0', "\\0");
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = CString::new(message).expect("sanitized error string is valid");
    });
}
