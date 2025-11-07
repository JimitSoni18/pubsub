use std::{
	borrow::Cow,
	hash::Hash,
	sync::atomic::{AtomicU64, Ordering},
};

use dashmap::{DashMap, DashSet};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ClientSubscription<M> {
	id: u64,
	tx: mpsc::Sender<M>,
}

impl<M> Hash for ClientSubscription<M> {
	// depends on the guarantee that each client will have a unique id
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl<M> PartialEq for ClientSubscription<M> {
	fn eq(&self, other: &Self) -> bool {
		self.id.eq(&other.id)
	}
}

impl<M> Eq for ClientSubscription<M> {}

impl<M> ClientSubscription<M> {
	pub fn sender(&mut self) -> &mut mpsc::Sender<M> {
		&mut self.tx
	}

	pub fn id(&self) -> u64 {
		self.id
	}
}

// TODO: use Arc<str> for channel name
#[derive(Default)]
pub struct Broker<'a, M> {
	// channel name -> client ids
	// TODO: use Arc<str> or something lighter for channel
	// TODO: do channel name -> ClientSubscription instead
	channels: DashMap<Cow<'a, str>, DashSet<ClientSubscription<M>>>,
	// names?
	// subscribers: DashMap<ClientId, mpsc::Sender<M>>,
	seq: AtomicU64,
}

pub const MESSAGE_CHANNEL_QUEUE_LIMIT: usize = 8;

impl<'a, M> Broker<'a, M> {
	pub fn new() -> Self {
		Self {
			channels: Default::default(),
			seq: Default::default(),
		}
	}

	pub fn new_client(&self) -> (mpsc::Receiver<M>, ClientSubscription<M>) {
		let (tx, rx) = mpsc::channel::<M>(MESSAGE_CHANNEL_QUEUE_LIMIT);
		(
			rx,
			ClientSubscription {
				tx,
				id: self.seq.fetch_add(1, Ordering::Relaxed),
			},
		)
	}

	pub fn subscribe(
		&self,
		channel: String,
		client: ClientSubscription<M>,
	) -> Result<(), SubscribeError> {
		match self.channels.entry(Cow::Owned(channel)) {
			dashmap::Entry::Occupied(occupied_entry) => {
				occupied_entry.get().insert(client);
			}
			_ => return Err(SubscribeError::ChannelNotFound),
		}

		Ok(())
	}

	/// unsubscribe from a particular channel
	pub fn unsubscribe(
		&self,
		channel: &str,
		client: &ClientSubscription<M>,
	) -> Result<(), UnsubscribeError> {
		self.channels
			.get(&*Cow::Borrowed(channel))
			.ok_or(UnsubscribeError::ChannelNotFound)?
			.remove(client)
			.ok_or(UnsubscribeError::AlreadyNotSubscribed)?;

		Ok(())
	}

	// TODO: returl count/list of channels unsubscribed?
	// TODO: return error in case channel not found or client not subscribed
	/// unsubscribe from many channels
	pub fn unsubscribe_many(
		&self,
		channels: impl IntoIterator<Item = impl AsRef<str>>,
		client: &ClientSubscription<M>,
	) {
		for channel in channels {
			if let Some(subscription_set) = self.channels.get(&*Cow::Borrowed(channel.as_ref())) {
				subscription_set.remove(client);
			}
		}
	}

	// TODO: return count/list?
	/// unsubscibe from all channels
	pub fn unsubscribe_all(&self, client: &ClientSubscription<M>) {
		self.channels.iter().for_each(|set| {
			set.remove(client);
		});
	}

	pub async fn publish(&self, channel_name: &str, message: M) -> Result<usize, PublishError>
	where
		M: Clone,
	{
		let subscribers = self
			.channels
			.get(&*Cow::Borrowed(channel_name))
			.ok_or(PublishError::ChannelNotFound)?;

		let count = futures::future::join_all(subscribers.iter().map(|client| {
			let message = message.clone();
			// cloned so we don't get Vec<RefMulti>, which causes deadlock in the
			// `subscribers.remove()` below
			let client = client.clone();
			async move { (client.tx.send(message.clone()).await.is_ok(), client) }
		}))
		.await
		.into_iter()
		.filter(|result| {
			if result.0 {
				subscribers.remove(&result.1);
			}
			result.0
		})
		.count();

		Ok(count)
	}

	// maybe just return bool?: true -> channel created; false -> channel already exists
	// and then:
	// #[must_use]
	pub fn create_channel(&self, channel_name: &str) -> Result<(), NewChannelError> {
		let channel_name = Cow::Borrowed(channel_name);
		if self.channels.contains_key(&*channel_name) {
			return Err(NewChannelError::ChannelAlreadyExists);
		}

		self.channels
			.insert(Cow::Owned(channel_name.into_owned()), Default::default());

		Ok(())
	}

	// TODO: unregister/destroy client?; 48 hours later: not planned
}

#[derive(Debug)]
pub enum SubscribeError {
	ChannelNotFound,
	ClientNotRegistered,
}

#[derive(Debug)]
pub enum NewChannelError {
	ChannelAlreadyExists,
}

#[derive(Debug)]
pub enum UnsubscribeError {
	ChannelNotFound,
	AlreadyNotSubscribed,
	SubscriberMissing,
}

#[derive(Debug)]
pub enum PublishError {
	ChannelNotFound,
}
