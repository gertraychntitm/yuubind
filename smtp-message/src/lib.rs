#![feature(async_await, await_macro, futures_api)]

extern crate bytes;
#[macro_use]
extern crate nom;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate futures;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

#[macro_use]
mod stupidparsers;

mod builderror;
mod byteslice;
mod domain;
mod email;
mod parameters;
mod parseresult;
mod sendable;
mod smtpstring;
mod streamext;

mod data;
mod ehlo;
mod expn;
mod helo;
mod help;
mod mail;
mod noop;
mod quit;
mod rcpt;
mod rset;
mod vrfy;

mod command;
mod reply;

pub use byteslice::ByteSlice;
pub use email::Email;
pub use parseresult::ParseError;
pub use sendable::Sendable;
pub use smtpstring::SmtpString;
pub use streamext::{Prependable, StreamExt};

pub use command::Command;
pub use reply::{IsLastLine, ReplyCode, ReplyLine};

pub use data::{DataCommand, DataSink, DataStream};
pub use ehlo::EhloCommand;
pub use expn::ExpnCommand;
pub use helo::HeloCommand;
pub use help::HelpCommand;
pub use mail::MailCommand;
pub use noop::NoopCommand;
pub use quit::QuitCommand;
pub use rcpt::RcptCommand;
pub use rset::RsetCommand;
pub use vrfy::VrfyCommand;
