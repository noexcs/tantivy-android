use jni::JNIEnv;
use jni::objects::{JClass, JString, JObject, JValue};
use jni::sys::{jlong, jobject, jint};
use std::sync::Mutex;

mod index;
mod tokenizer;

struct NativeIndex {
    inner: index::IndexManager,
}

fn get_index(ptr: jlong) -> &'static Mutex<NativeIndex> {
    unsafe { &*(ptr as *const Mutex<NativeIndex>) }
}

fn jstring_to_string(env: &mut JNIEnv, s: &JString) -> Result<String, String> {
    env.get_string(s)
        .map(|s| s.into())
        .map_err(|e| format!("Failed to read JNI string: {}", e))
}

/// Throws a RuntimeException on the Java side with the given message.
fn throw(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new("java/lang/RuntimeException", msg);
}

// ─── nativeCreate ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeCreate(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let mgr = index::IndexManager::new();
    let native = Mutex::new(NativeIndex { inner: mgr });
    Box::into_raw(Box::new(native)) as jlong
}

// ─── nativeRebuildIndex ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeRebuildIndex(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    docs: JObject,
) {
    let native = get_index(ptr);
    let mut idx = match native.lock() {
        Ok(guard) => guard,
        Err(_) => { throw(&mut env, "Lock poisoned"); return; }
    };

    let list = match env.get_list(&docs) {
        Ok(l) => l,
        Err(_) => { throw(&mut env, "docs must be a List"); return; }
    };
    let size = match list.size(&mut env) {
        Ok(s) => s as usize,
        Err(_) => { throw(&mut env, "Failed to get list size"); return; }
    };
    let mut rust_docs = Vec::with_capacity(size);

    for i in 0..size {
        let obj = match list.get(&mut env, i as i32) {
            Ok(Some(o)) => o,
            _ => { throw(&mut env, "Failed to get list element"); return; }
        };

        let id = match (|| -> Result<String, String> {
            let js: JString = env.call_method(&obj, "getId", "()Ljava/lang/String;", &[])
                .map_err(|e| format!("{}", e))?.l().map_err(|e| format!("{}", e))?.into();
            jstring_to_string(&mut env, &js)
        })() {
            Ok(s) => s,
            Err(e) => { throw(&mut env, &e); return; }
        };

        let header_key = match (|| -> Result<String, String> {
            let js: JString = env.call_method(&obj, "getHeaderKey", "()Ljava/lang/String;", &[])
                .map_err(|e| format!("{}", e))?.l().map_err(|e| format!("{}", e))?.into();
            jstring_to_string(&mut env, &js)
        })() {
            Ok(s) => s,
            Err(e) => { throw(&mut env, &e); return; }
        };

        let text = match (|| -> Result<String, String> {
            let js: JString = env.call_method(&obj, "getText", "()Ljava/lang/String;", &[])
                .map_err(|e| format!("{}", e))?.l().map_err(|e| format!("{}", e))?.into();
            jstring_to_string(&mut env, &js)
        })() {
            Ok(s) => s,
            Err(e) => { throw(&mut env, &e); return; }
        };

        rust_docs.push(index::DocInput { id, header_key, text });
    }

    if let Err(e) = idx.inner.rebuild(&rust_docs) {
        throw(&mut env, &format!("Rebuild failed: {}", e));
    }
}

// ─── nativeAddOrUpdate ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeAddOrUpdate(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    id: JString,
    header_key: JString,
    text: JString,
) {
    let native = get_index(ptr);
    let mut idx = match native.lock() {
        Ok(guard) => guard,
        Err(_) => { throw(&mut env, "Lock poisoned"); return; }
    };

    let id_str = match jstring_to_string(&mut env, &id) {
        Ok(s) => s,
        Err(e) => { throw(&mut env, &e); return; }
    };
    let hk_str = match jstring_to_string(&mut env, &header_key) {
        Ok(s) => s,
        Err(e) => { throw(&mut env, &e); return; }
    };
    let text_str = match jstring_to_string(&mut env, &text) {
        Ok(s) => s,
        Err(e) => { throw(&mut env, &e); return; }
    };

    if let Err(e) = idx.inner.add_or_update(&id_str, &hk_str, &text_str) {
        throw(&mut env, &format!("AddOrUpdate failed: {}", e));
    }
}

// ─── nativeRemoveByHeader ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeRemoveByHeader(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    header_key: JString,
) {
    let native = get_index(ptr);
    let mut idx = match native.lock() {
        Ok(guard) => guard,
        Err(_) => { throw(&mut env, "Lock poisoned"); return; }
    };

    let hk_str = match jstring_to_string(&mut env, &header_key) {
        Ok(s) => s,
        Err(e) => { throw(&mut env, &e); return; }
    };

    if let Err(e) = idx.inner.remove_by_header(&hk_str) {
        throw(&mut env, &format!("RemoveByHeader failed: {}", e));
    }
}

// ─── nativeSearch ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeSearch(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    query: JString,
    top_k: jint,
) -> jobject {
    let native = get_index(ptr);
    let idx = match native.lock() {
        Ok(guard) => guard,
        Err(_) => {
            throw(&mut env, "Lock poisoned");
            return std::ptr::null_mut();
        }
    };

    let q = match jstring_to_string(&mut env, &query) {
        Ok(s) => s,
        Err(e) => {
            throw(&mut env, &e);
            return std::ptr::null_mut();
        }
    };

    let results = match idx.inner.search(&q, top_k as usize) {
        Ok(r) => r,
        Err(e) => {
            throw(&mut env, &format!("Search failed: {}", e));
            return std::ptr::null_mut();
        }
    };

    let map_class = match env.find_class("java/util/HashMap") {
        Ok(c) => c,
        Err(e) => {
            throw(&mut env, &format!("{}", e));
            return std::ptr::null_mut();
        }
    };
    let map = match env.new_object(map_class, "()V", &[]) {
        Ok(m) => m,
        Err(e) => {
            throw(&mut env, &format!("{}", e));
            return std::ptr::null_mut();
        }
    };

    for (id, score) in &results {
        if env.new_string(id).is_err() {
            continue;
        }
        let key = match env.new_string(id) {
            Ok(k) => k,
            Err(_) => continue,
        };
        let float_obj = match env.new_object("java/lang/Float", "(F)V", &[JValue::Float(*score)]) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let _ = env.call_method(
            &map,
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::Object(&key), JValue::Object(&float_obj)],
        );
    }
    map.into_raw()
}

// ─── nativeClose ───

#[no_mangle]
pub extern "system" fn Java_com_noexcs_tantivy_TantivyBM25_nativeClose(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let _native = unsafe { Box::from_raw(ptr as *mut Mutex<NativeIndex>) };
}
