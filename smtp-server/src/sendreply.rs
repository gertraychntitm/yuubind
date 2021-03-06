use itertools::Itertools;
use smtp_message::{IsLastLine, ReplyCode, ReplyLine, SmtpString, StreamExt};
use tokio::prelude::*;

// TODO: (B) move to smtp_message's Reply builder id:tcHW
// Panics if `text` has a byte not in {9} \union [32; 126]
// TODO: (B) move sending logic to smtp_message::Reply
pub fn send_reply<'a, W>(
    writer: W,
    (code, text): (ReplyCode, SmtpString),
) -> impl Future<Item = W, Error = W::SinkError> + 'a
where
    W: 'a + Sink<SinkItem = ReplyLine>,
    W::SinkError: 'a,
{
    let replies = text
        .byte_chunks(ReplyLine::MAX_LEN)
        .with_position()
        .map(move |t| {
            use itertools::Position::*;
            match t {
                First(t) | Middle(t) => ReplyLine::build(code, IsLastLine::No, t).unwrap(),
                Last(t) | Only(t) => ReplyLine::build(code, IsLastLine::Yes, t).unwrap(),
            }
        });

    stream::iter_ok(replies)
        .forward_not_closing(writer)
        .map(|(_, w)| w)
}
