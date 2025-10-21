use std::sync::atomic::{AtomicU64, Ordering};

use crate::Message;

use dashmap::{DashMap, DashSet};
use tokio::sync::mpsc;

// TODO: use Arc<str> for channel name
#[derive(Default)]
pub struct Broker {
	// channel name -> client ids
	channels: DashMap<String, DashSet<u64>>,
	subscribers: DashMap<u64, mpsc::Sender<Message>>,
	seq: AtomicU64,
}

pub struct Client {
	id: u64,
	rx: mpsc::Receiver<Message>,
}

impl Client {
	pub fn id(&self) -> u64 {
		self.id
	}

	pub fn receiver(&mut self) -> &mut mpsc::Receiver<Message> {
		&mut self.rx
	}
}

pub const MESSAGE_CHANNEL_QUEUE_LIMIT: usize = 8;

impl Broker {
	pub fn new_client(&self) -> Client {
		let (tx, rx) = mpsc::channel::<Message>(MESSAGE_CHANNEL_QUEUE_LIMIT);
		let id = self.issue_id();
		self.subscribers.insert(id, tx);

		Client { id, rx }
	}

	pub fn subscribe(&self, channel: String, client: &Client) -> Result<(), SubscribeError> {
		if self.subscribers.get(&client.id).is_none() {
			// this should never be true
			return Err(SubscribeError::ClientNotRegistered);
		}

		match self.channels.entry(channel) {
			dashmap::Entry::Occupied(occupied_entry) => {
				occupied_entry.get().insert(client.id());
			}
			_ => return Err(SubscribeError::ChannelNotFound),
		}

		Ok(())
	}

	/// unsubscribe from a particular channel
	pub fn unsubscribe(&self, channel: String, client: &Client) -> Result<(), UnsubscribeError> {
		// FIXME: should not panic
		self.channels
			.get(&channel)
			.ok_or(UnsubscribeError::ChannelNotFound)?
			.remove(&client.id())
			.ok_or(UnsubscribeError::AlreadyNotSubscribed)?;

		Ok(())
	}

	// TODO: return count/list?
	/// unsubscibe from all channels
	pub fn unsubscribe_all(&self, subscriber: &Client) {
		self.channels.iter().for_each(|set| {
			set.remove(&subscriber.id());
		});
	}

	pub(super) fn issue_id(&self) -> u64 {
		self.seq.fetch_add(1, Ordering::Relaxed)
	}

	pub async fn publish(
		&self,
		channel_name: &String,
		message: Message,
	) -> Result<usize, PublishError> {
		let subscriber_ids = self
			.channels
			.get(channel_name)
			.ok_or(PublishError::ChannelNotFound)?;

		let count = futures::future::join_all(subscriber_ids.iter().filter_map(|id| {
			self.subscribers.get(&id).map(|sender| {
				let msg_clone = message.clone();
				async move {
					// if it errors out, it means receiver is dropped
					let rx_dropped = sender.send(msg_clone).await.is_err();
					if rx_dropped {
						self.subscribers.remove(&id);
						self.channels.iter().for_each(|c| {
							c.remove(&id);
						});
					}
					rx_dropped
				}
			})
		}))
		.await
		.into_iter()
		.filter(|b| *b)
		.count();

		Ok(count)
	}

	// TODO: unregister/destroy client?
}

#[derive(Debug)]
pub enum SubscribeError {
	ChannelNotFound,
	ClientNotRegistered,
}

#[derive(Debug)]
pub enum UnsubscribeError {
	ChannelNotFound,
	AlreadyNotSubscribed,
}

#[derive(Debug)]
pub enum PublishError {
	ChannelNotFound,
}
