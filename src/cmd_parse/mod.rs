#[derive(Debug)]
pub enum CommandParsingError {
	// General errors
	MalformedCommand,
    // TODO: remove this?
	UnknownCommand,
	ChannelNameNotAscii,

	PubWithBadArgs,
	PubWithNoArgs,

    // TODO: why not used
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

#[non_exhaustive]
pub enum Command<'a> {
	Pub { channel: &'a str, message: &'a str },
	Sub { channels: Vec<&'a str> },
	UnSub { channels: ChanList<'a> },
	PSub { channel_patterns: Vec<&'a str> },
	PUnSub { channel_patterns: Vec<&'a str> },
	PubSub(PubSubSubCmd<'a>),
}

#[inline]
pub fn parse_cmd(cmd: &str) -> Result<Command, CommandParsingError> {
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

