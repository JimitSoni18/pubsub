use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Message, subscriber::Subscriber};

use dashmap::{DashMap, DashSet, Map};
use tokio::sync::mpsc;

// TODO: use Arc<str> for channel name
#[derive(Default)]
pub struct Broker {
    // channel name -> client ids
    channels: DashMap<String, DashSet<u64>>,
    subscribers: DashMap<u64, mpsc::Sender<Message>>,
    seq: AtomicU64,
}

impl Broker {
    // NOTE!: if the subscriber sender is already present, it will re-insert the sender
    pub fn subscribe(
        &self,
        channel: String,
        subscriber: &Subscriber,
    ) -> Result<(), SubscribeError> {
        self.subscribers
            .insert(subscriber.id(), subscriber.sender.clone());
        match self.channels.entry(channel) {
            dashmap::Entry::Occupied(occupied_entry) => {
                occupied_entry.get().insert(subscriber.id());
            }
            _ => return Err(SubscribeError::ChannelNotFound),
        }

        Ok(())
    }

    /// unsubscribe from a particular channel
    pub fn unsubscribe(&self, channel: String, subscriber: &Subscriber) {
        // FIXME: should not panic
        self.channels
            .get(&channel)
            .unwrap()
            .remove(&subscriber.id());
    }

    // TODO: return count/list?
    /// unsubscibe from all channels
    pub fn unsubscribe_all(&self, subscriber: &Subscriber) {
        self.channels.iter().for_each(|set| {
            set.remove(&subscriber.id());
        });
    }

    pub(super) fn get_id(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn publish(&self, message: Message) -> Result<usize, PublishError> {
        let subscriber_ids = self
            .channels
            .get(&message.channel_name)
            .ok_or(PublishError::ChannelNotFound)?;

        let count = futures::future::join_all(subscriber_ids.iter().filter_map(|id| {
            self.subscribers.get(&id).map(|sender| {
                let msg_clone = message.clone();
                async move { sender.send(msg_clone).await }
            })
        }))
        .await
        .iter()
        .filter(|t| t.is_ok())
        .count();

        Ok(count)
    }
}

#[derive(Debug)]
pub enum SubscribeError {
    ChannelNotFound,
}

#[derive(Debug)]
pub enum PublishError {
    ChannelNotFound,
}
