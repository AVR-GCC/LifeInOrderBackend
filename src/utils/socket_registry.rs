use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, Arc},
};

use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct Entry {
    user_id: i32,
    conn_id: String,
    cancel: CancellationToken,
}

#[derive(Default, Clone, Debug)]
pub struct SocketRegistry {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
}

impl SocketRegistry {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add(&self, family_id: String, user_id: i32, conn_id: String, cancel: CancellationToken) {
        let entry = Entry {
            user_id,
            conn_id,
            cancel,
        };
        // println!("add entry: {:?}", entry);
        let old = self.lock().insert(family_id, entry);
        if let Some(old) = old {
            old.cancel.cancel();
        }
        // println!("self: {:?}", self);
    }

    pub fn remove(&self, family_id: String, conn_id: String) {
        let mut map = self.lock();
        // println!("remove entry: {} {}", family_id, conn_id);
        if matches!(map.get(&family_id), Some(e) if e.conn_id == conn_id) {
            map.remove(&family_id);
        }
        // println!("self: {:?}", self);
    }

    pub fn close(&self, family_id: String) {
        // println!("close: {}", family_id);
        if let Some(entry) = self.lock().remove(&family_id) {
            entry.cancel.cancel();
        }
        // println!("self: {:?}", self);
    }

    pub fn close_user(&self, user_id: i32) -> usize {
        // println!("close user: {}", user_id);
        let mut map = self.lock();
        let family_ids: Vec<String> = map
            .iter()
            .filter(|(_, e)| e.user_id == user_id)
            .map(|(family_id, _)| family_id.clone())
            .collect();
        for family_id in &family_ids {
            if let Some(entry) = map.remove(family_id) {
                entry.cancel.cancel();
            }
        }
        // println!("self: {:?}", self);
        family_ids.len()
    }
}
