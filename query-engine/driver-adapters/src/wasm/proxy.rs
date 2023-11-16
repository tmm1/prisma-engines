use ducktor::FromJsValue as DuckType;
use futures::Future;
use js_sys::{Function as JsFunction, Object as JsObject, Promise as JsPromise};

use super::{async_js_function::AsyncJsFunction, send_future::SendFuture, transaction::JsTransaction};
pub use crate::types::{ColumnType, JSResultSet, Query, TransactionOptions};
use metrics::increment_gauge;
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

type JsResult<T> = core::result::Result<T, JsValue>;

/// Proxy is a struct wrapping a javascript object that exhibits basic primitives for
/// querying and executing SQL (i.e. a client connector). The Proxy uses Wasm's JsFunction to
/// invoke the code within the node runtime that implements the client connector.
#[wasm_bindgen(getter_with_clone)]
#[derive(DuckType, Default)]
pub(crate) struct CommonProxy {
    /// Execute a query given as SQL, interpolating the given parameters.
    query_raw: AsyncJsFunction<Query, JSResultSet>,

    /// Execute a query given as SQL, interpolating the given parameters and
    /// returning the number of affected rows.
    execute_raw: AsyncJsFunction<Query, u32>,

    /// Return the flavour for this driver.
    pub(crate) flavour: String,
}

/// This is a JS proxy for accessing the methods specific to top level
/// JS driver objects
#[wasm_bindgen(getter_with_clone)]
#[derive(DuckType)]
pub(crate) struct DriverProxy {
    start_transaction: AsyncJsFunction<(), JsTransaction>,
}

/// This a JS proxy for accessing the methods, specific
/// to JS transaction objects
#[wasm_bindgen(getter_with_clone)]
#[derive(DuckType, Default)]
pub(crate) struct TransactionProxy {
    /// transaction options
    options: TransactionOptions,

    /// commit transaction
    commit: AsyncJsFunction<(), ()>,

    /// rollback transaction
    rollback: AsyncJsFunction<(), ()>,

    /// dispose transaction, cleanup logic executed at the end of the transaction lifecycle
    /// on drop.
    dispose: JsFunction,
}

impl CommonProxy {
    pub fn new(object: &JsObject) -> Self {
        CommonProxy::from(&object.into())
    }

    pub async fn query_raw(&self, params: Query) -> quaint::Result<JSResultSet> {
        self.query_raw.call(params).await
    }

    pub async fn execute_raw(&self, params: Query) -> quaint::Result<u32> {
        self.execute_raw.call(params).await
    }
}

impl DriverProxy {
    pub fn new(object: &JsObject) -> Self {
        Self::from(&object.into())
    }

    async fn start_transaction_inner(&self) -> quaint::Result<Box<JsTransaction>> {
        let tx = self.start_transaction.call(()).await?;

        // Decrement for this gauge is done in JsTransaction::commit/JsTransaction::rollback
        // Previously, it was done in JsTransaction::new, similar to the native Transaction.
        // However, correct Dispatcher is lost there and increment does not register, so we moved
        // it here instead.
        increment_gauge!("prisma_client_queries_active", 1.0);
        Ok(Box::new(tx))
    }

    pub fn start_transaction<'a>(
        &'a self,
    ) -> SendFuture<impl Future<Output = quaint::Result<Box<JsTransaction>>> + 'a> {
        SendFuture(self.start_transaction_inner())
    }
}

impl TransactionProxy {
    pub fn new(object: &JsObject) -> Self {
        Self::from(&object.into())
    }

    pub fn options(&self) -> &TransactionOptions {
        &self.options
    }

    pub fn commit<'a>(&'a self) -> SendFuture<impl Future<Output = quaint::Result<()>> + 'a> {
        SendFuture(self.commit.call(()))
    }

    pub fn rollback<'a>(&'a self) -> SendFuture<impl Future<Output = quaint::Result<()>> + 'a> {
        SendFuture(self.rollback.call(()))
    }
}

impl Drop for TransactionProxy {
    fn drop(&mut self) {
        _ = self.dispose.call0(&JsValue::null());
    }
}

// Assume the proxy object will not be sent to service workers, we can unsafe impl Send + Sync.
unsafe impl Send for TransactionProxy {}
unsafe impl Sync for TransactionProxy {}

unsafe impl Send for DriverProxy {}
unsafe impl Sync for DriverProxy {}

unsafe impl Send for CommonProxy {}
unsafe impl Sync for CommonProxy {}
