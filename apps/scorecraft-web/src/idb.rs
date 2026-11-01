//! The library kept in the browser: its files mirrored into IndexedDB (database `scorecraft`,
//! store `files`, key = path, value = bytes), loaded before the app starts and written behind
//! as they change. The library itself runs on memory storage, as the engine's store is
//! synchronous and IndexedDB is not.

use eframe::wasm_bindgen::closure::Closure;
use eframe::wasm_bindgen::{JsCast, JsValue};
use js_sys::{Array, Promise, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransactionMode};

const DB: &str = "scorecraft";
const STORE: &str = "files";

/// A request's result, as a future.
fn result(req: &IdbRequest) -> JsFuture {
    let req = req.clone();
    let promise = Promise::new(&mut |resolve, reject| {
        let r = req.clone();
        let ok = Closure::once_into_js(move || {
            let _ = resolve.call1(&JsValue::NULL, &r.result().unwrap_or(JsValue::UNDEFINED));
        });
        req.set_onsuccess(Some(ok.unchecked_ref()));
        let err = Closure::once_into_js(move || {
            let _ = reject.call0(&JsValue::NULL);
        });
        req.set_onerror(Some(err.unchecked_ref()));
    });
    JsFuture::from(promise)
}

async fn open() -> Result<IdbDatabase, JsValue> {
    let factory = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .indexed_db()?
        .ok_or_else(|| JsValue::from_str("no IndexedDB"))?;
    let req: IdbOpenDbRequest = factory.open_with_u32(DB, 1)?;
    let upgrade = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        let db = e
            .target()
            .and_then(|t| t.dyn_into::<IdbOpenDbRequest>().ok())
            .and_then(|r| r.result().ok())
            .map(|d| d.unchecked_into::<IdbDatabase>());
        if let Some(db) = db {
            let _ = db.create_object_store(STORE);
        }
    });
    req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    // Lives until the database is open; one per open.
    upgrade.forget();
    Ok(result(&req).await?.unchecked_into())
}

/// Every stored file.
pub async fn load() -> Result<Vec<(String, Vec<u8>)>, JsValue> {
    let db = open().await?;
    let tx = db.transaction_with_str(STORE)?;
    let store = tx.object_store(STORE)?;
    // Keys and values come back in the same (key) order.
    let keys: Array = result(&store.get_all_keys()?).await?.unchecked_into();
    let values: Array = result(&store.get_all()?).await?.unchecked_into();
    Ok(keys
        .iter()
        .zip(values.iter())
        .filter_map(|(k, v)| Some((k.as_string()?, Uint8Array::new(&v).to_vec())))
        .collect())
}

/// Writes (or, for `None`, deletes) the changed files in one transaction.
pub async fn apply(changes: Vec<(String, Option<Vec<u8>>)>) -> Result<(), JsValue> {
    let db = open().await?;
    let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite)?;
    let store = tx.object_store(STORE)?;
    for (path, data) in changes {
        let key = JsValue::from_str(&path);
        match data {
            Some(bytes) => {
                store.put_with_key(&Uint8Array::from(bytes.as_slice()), &key)?;
            }
            None => {
                store.delete(&key)?;
            }
        }
    }
    Ok(())
}
