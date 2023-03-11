use crate::{client::Client, window_manager::WindowKind};
use std::{collections::HashMap, sync::Arc};
use xcb::Xid;

// We use our own context map instead of the xlib functions so we can store
// more than just pointers.
// This gives use these main advantages:
//  - safer
//  - faster
//  - don't need to do X calls for internal information

/// Since the `Context` enum also doubles as the context id we want to be able
/// to get the id from the variants without constructing them.
pub trait GetContextId {
    fn id(self) -> u32;
}

impl GetContextId for fn(Arc<Client>) -> Context {
    fn id(self) -> u32 {
        2
    }
}

impl GetContextId for fn(WindowKind) -> Context {
    fn id(self) -> u32 {
        3
    }
}

#[derive(Clone, Debug)]
#[repr(u32)]
pub enum Context {
    Client(Arc<Client>),
    WindowKind(WindowKind),
}

impl Context {
    pub fn id(&self) -> u32 {
        match self {
            Self::Client(_) => (Self::Client as fn(_) -> Context).id(),
            Self::WindowKind(_) => (Self::WindowKind as fn(_) -> Context).id(),
        }
    }

    /// Returns the contained `Client` value, consuming the context.
    pub fn unwrap_client(self) -> Arc<Client> {
        if let Self::Client(client) = self {
            client
        } else {
            unreachable!();
        }
    }

    /// Returns the contained `WindowKind` value, consuming the context.
    pub fn unwrap_window_kind(self) -> WindowKind {
        if let Self::WindowKind(window_kind) = self {
            window_kind
        } else {
            unreachable!();
        }
    }

    pub fn all_ids() -> [u32; 2] {
        [
            (Self::Client as fn(_) -> Context).id(),
            (Self::WindowKind as fn(_) -> Context).id(),
        ]
    }
}

#[derive(Default, Debug)]
pub struct ContextMap {
    // The low 32 bits of the key are the xid and the high 32 bits are the
    // context id.
    map: HashMap<u64, Context>,
    // When dispatching an event or signal we will want to repeatedly call
    // `get_window_kind` from the different event sinks without any other
    // access to the context map so we cache that value.
    last_key: u64,
    last_value: Option<Context>,
}

impl ContextMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            // A valid key can never be 0 as no context id is 0.
            last_key: 0,
            last_value: None,
        }
    }

    fn remove_cached(&mut self, key: u64) {
        if self.last_key == key {
            self.last_key = 0;
            self.last_value = None;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn save(&mut self, resource: &impl Xid, context: Context) {
        let key = ((context.id() as u64) << 32) | resource.resource_id() as u64;
        self.map.insert(key, context);
    }

    pub fn find<T>(&mut self, resource: &impl Xid, context: fn(T) -> Context) -> Option<Context>
    where
        fn(T) -> Context: GetContextId,
    {
        let key = ((context.id() as u64) << 32) | resource.resource_id() as u64;
        if self.last_key == key {
            self.last_value.clone()
        } else {
            let value = self.map.get(&key).map(Clone::clone);
            self.last_value = value.clone();
            value
        }
    }

    pub fn delete<T>(&mut self, resource: &impl Xid, context: fn(T) -> Context)
    where
        fn(T) -> Context: GetContextId,
    {
        let key = ((context.id() as u64) << 32) | resource.resource_id() as u64;
        self.remove_cached(key);
        self.map.remove(&key);
    }

    pub fn delete_all(&mut self, resource: &impl Xid) {
        for ctx in Context::all_ids() {
            let key = ((ctx as u64) << 32) | resource.resource_id() as u64;
            self.remove_cached(key);
            self.map.remove(&key);
        }
    }
}
