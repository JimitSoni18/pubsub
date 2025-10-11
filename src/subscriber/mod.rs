use std::{pin::Pin, task::Poll};

use crate::{Message, broker::Broker};

use futures::{SinkExt, Stream, StreamExt};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};

const CHANNEL_BOUNDED_SIZE: usize = 8;

// TODO: Arc<subscriber>
pub struct Subscriber {
    id: u64, // needed?
    pub(super) sender: mpsc::Sender<Message>,
}

pub struct SubscriberFuture {
    broker: &'static Broker,
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    // stream: tokio::net::TcpStream,
    rx: tokio::sync::mpsc::Receiver<Message>,
    // tx: tokio_util::sync::PollSender<Message>,
}

impl Future for SubscriberFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.framed).poll_next(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let cmd_str = str::from_utf8(&frame).unwrap();
                println!("received command: {cmd_str}");
                let cmd = parse_cmd(cmd_str).unwrap();
                // let inputb
            }
            Poll::Ready(None) => todo!(),
            _ => todo!(),
        }

        // Pin::new(&mut self.framed).poll

        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(message)) => {}
            // when and why would a channel close - trace
            Poll::Ready(None) => {
                loop {
                    let poll = self.framed.poll_flush_unpin(cx);
                    //? should block after channel is closed?
                    match poll {
                        Poll::Ready(_) => todo!(),
                        Poll::Pending => todo!(),
                    }
                }
            }
            _ => {}
        }

        Poll::Pending
    }
}

impl Subscriber {
    pub async fn listen(broker: &'static Broker, stream: tokio::net::TcpStream) {
        // FIXME: bound
        let (tx, mut rx) = mpsc::channel(CHANNEL_BOUNDED_SIZE);
        // let (tx2, mut rx2) = mpsc::channel(CHANNEL_BOUNDED_SIZE);
        let codec = LengthDelimitedCodec::new();
        // let framed = Framed::new(stream, codec)
        let (reader, writer) = stream.into_split();
        let subscriber = Self {
            id: broker.get_id(),
            sender: tx.clone(),
        };
        let mut framed_read = FramedRead::new(reader, codec.clone());
        let mut framed_write = FramedWrite::new(writer, codec);
        // let mut framed_write_clone = framed_write.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                framed_write.send(message.message.into()).await.unwrap();
            }
            if let Err(why) = framed_write.close().await {
                eprintln!("failed to close writer: {why}");
            }
        });
        tokio::spawn(async move {
            loop {
                let next_frame = framed_read.next().await.unwrap().unwrap();
                let cmd_str = str::from_utf8(&next_frame).unwrap();
                let cmd = parse_cmd(cmd_str).unwrap();
                match cmd {
                    Command::Pub { channel, message } => {
                        broker
                            .publish(Message {
                                message: message.to_string(),
                                channel_name: channel.to_string(),
                            })
                            .await
                            .unwrap();
                    }
                    Command::Sub { channels } => {
                        channels.iter().for_each(|ch| {
                            // TODO: how should i handle failures
                            broker.subscribe(ch.to_string(), &subscriber).unwrap();
                        });
                    }
                    Command::UnSub { channels } => match channels {
                        ChanList::All => {
                            broker.unsubscribe_all(&subscriber);
                        }
                        ChanList::Many(channels) => {
                            channels.iter().for_each(|ch| {
                                broker.unsubscribe(ch.to_string(), &subscriber);
                            });
                        }
                    },
                    // TODO: implement
                    Command::PSub { channel_patterns } => unimplemented!(),
                    Command::PUnSub { channel_patterns } => unimplemented!(),
                    Command::PubSub(pub_sub_sub_cmd) => unimplemented!(),
                }
                // !unimplemented: parse message from command
                // broker.subscribe(cmd, &subscriber).unwrap();
            }
        });
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

#[inline]
fn parse_cmd(cmd: &str) -> Result<Command, CommandParsingError> {
    // TODO: use split_once + match
    const SPACE: char = ' ';
    let Some((command, rest)) = cmd.trim().split_once(SPACE) else {
        return Err(CommandParsingError::MalformedCommand);
    };
    match command {
        "PUB" => {
            // FIXME: whitespace is message?
            let channel_message = rest.trim_start();
            if channel_message.is_empty() {
                return Err(CommandParsingError::PubWithNoArgs);
            }
            let Some((channel, message)) = channel_message.split_once(SPACE) else {
                return Err(CommandParsingError::PubWithBadArgs);
            };
            Ok(Command::Pub { channel, message })
        }
        "SUB" => {
            if rest.trim().is_empty() {
                return Err(CommandParsingError::PSubWithNoArgs);
            }
            let mut channels_iter = rest.split_whitespace();
            if !channels_iter.all(str::is_ascii) {
                return Err(CommandParsingError::ChannelNameNotAscii);
            }
            let channels = channels_iter.collect::<Vec<&str>>();

            Ok(Command::Sub { channels })
        }
        "UNSUB" => {
            if rest.trim().is_empty() {
                return Ok(Command::UnSub {
                    channels: ChanList::All,
                });
            }
            let mut channels_iter = rest.split_whitespace();
            if !channels_iter.all(str::is_ascii) {
                return Err(CommandParsingError::ChannelNameNotAscii);
            }
            Ok(Command::UnSub {
                channels: ChanList::Many(channels_iter.collect()),
            })
        }
        "PSUB" => {
            if rest.trim().is_empty() {
                return Err(CommandParsingError::PSubWithNoArgs);
            }
            let mut chan_iter = rest.split_whitespace();
            if !chan_iter.all(str::is_ascii) {
                return Err(CommandParsingError::ChannelNameNotAscii);
            }
            Ok(Command::PSub {
                channel_patterns: chan_iter.collect(),
            })
        }
        "PUNSUB" => {
            if rest.trim().is_empty() {
                return Err(CommandParsingError::PSubWithNoArgs);
            }
            let mut chan_iter = rest.split_whitespace();
            if !chan_iter.all(str::is_ascii) {
                return Err(CommandParsingError::ChannelNameNotAscii);
            }

            Ok(Command::PUnSub {
                channel_patterns: chan_iter.collect(),
            })
        }
        "PUBSUB" => {
            if rest.trim().is_empty() {
                return Err(CommandParsingError::PubSubWithNoArgs);
            }
            let (sub_cmd, args) = rest.split_once(SPACE).unwrap();
            match sub_cmd.trim() {
                "CHANS" => {
                    if args.trim().is_empty() {
                        return Ok(Command::PubSub(PubSubSubCmd::Channels {
                            channel_pattern: None,
                        }));
                    }
                    if args.contains(SPACE) {
                        return Err(CommandParsingError::PubSubChannelsMutipleArgs);
                    }
                    if !args.is_ascii() {
                        return Err(CommandParsingError::ChannelNameNotAscii);
                    }
                    Ok(Command::PubSub(PubSubSubCmd::Channels {
                        channel_pattern: Some(args),
                    }))
                }
                "NUMSUB" => {
                    if args.trim().is_empty() {
                        return Err(CommandParsingError::PubSubNumSubNoArgs);
                    }
                    let mut channels_iter = args.split_whitespace();
                    if !channels_iter.all(str::is_ascii) {
                        return Err(CommandParsingError::ChannelNameNotAscii);
                    }
                    Ok(Command::PubSub(PubSubSubCmd::NumSub {
                        channels: channels_iter.collect(),
                    }))
                }
                _ => Err(CommandParsingError::PubSubUnknownCommand),
            }
        }
        _ => Err(CommandParsingError::MalformedCommand),
    }
}

#[derive(Debug)]
pub enum CommandParsingError {
    // General errors
    MalformedCommand,
    UnknownCommand,
    ChannelNameNotAscii,

    PubWithBadArgs,
    PubWithNoArgs,

    SubWithNoArgs,

    PSubWithNoArgs,

    PubSubWithNoArgs,
    PubSubUnknownCommand,

    PubSubChannelsMutipleArgs,
    PubSubNumSubNoArgs,
}

pub enum PubSubSubCmd<'a> {
    Channels { channel_pattern: Option<&'a str> },
    NumSub { channels: Vec<&'a str> },
}

pub enum ChanList<'a> {
    All,
    Many(Vec<&'a str>),
}

pub enum Command<'a> {
    Pub { channel: &'a str, message: &'a str },
    Sub { channels: Vec<&'a str> },
    UnSub { channels: ChanList<'a> },
    PSub { channel_patterns: Vec<&'a str> },
    PUnSub { channel_patterns: Vec<&'a str> },
    PubSub(PubSubSubCmd<'a>),
}
