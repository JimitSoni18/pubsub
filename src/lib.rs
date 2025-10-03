// use fasthash::Murmur3Hasher;

// pub type MurMap<K, V> = DashMap<K, V, Murmur3Hasher>;

pub mod broker;
pub mod subscriber;

#[derive(Clone)]
pub struct Message {
    channel_name: String,
    message: String,
}
