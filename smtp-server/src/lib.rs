extern crate smtp_message;
extern crate tokio;

use smtp_message::*;
use std::mem;
use tokio::prelude::*;

pub type MailAddress = Vec<u8>;
pub type MailAddressRef<'a> = &'a [u8];

pub struct ConnectionMetadata<U> {
    user: U,
}

pub struct MailMetadata {
    from: MailAddress,
    to:   Vec<MailAddress>,
}

pub struct Refusal {
    code: ReplyCode,
    msg:  String,
}

pub enum Decision<T> {
    Accept(T),
    Reject(Refusal),
}

// The streams will be read 1-by-1, so make sure they are buffered
pub fn interact<
    'a,
    ReaderError,
    Reader: 'a + Stream<Item = u8, Error = ReaderError>,
    WriterError,
    Writer: Sink<SinkItem = u8, SinkError = WriterError>,
    UserProvidedMetadata: 'a,
    HandleReaderError: 'a + FnMut(ReaderError) -> (),
    HandleWriterError: 'a + FnMut(WriterError) -> (),
    State,
    FilterFrom: FnMut(MailAddressRef, &ConnectionMetadata<UserProvidedMetadata>) -> Decision<State>,
    FilterTo: FnMut(MailAddressRef, State, &ConnectionMetadata<UserProvidedMetadata>, &MailMetadata)
        -> Decision<State>,
    HandleMail: FnMut(MailMetadata, State, &ConnectionMetadata<UserProvidedMetadata>, &mut Reader)
        -> Decision<()>,
>(
    incoming: Reader,
    outgoing: &'a mut Writer,
    metadata: UserProvidedMetadata,
    handle_reader_error: HandleReaderError,
    handle_writer_error: HandleWriterError,
    filter_from: FilterFrom,
    filter_to: FilterTo,
    handler: HandleMail,
) -> Box<'a + Future<Item = (), Error = ()>> {
    // TODO: return `impl Future`
    let conn_meta = ConnectionMetadata { user: metadata };
    let writer = outgoing
        .sink_map_err(handle_writer_error)
        .with_flat_map(|c: Reply| {
            // TODO: actually make smtp-message's send_to work with sinks
            let mut v = Vec::new();
            c.send_to(&mut v).unwrap(); // and this is ugly
            stream::iter_ok(v)
        });
    Box::new(
        CrlfLines::new(incoming)
            .map_err(handle_reader_error)
            .fold(
                (writer, conn_meta, None as Option<MailMetadata>),
                |(writer, conn_meta, mail_meta), line| {
                    let cmd = Command::parse(&line);
                    match cmd {
                        Ok(Command::Mail(m)) => {
                            let from = m.raw_from().to_vec();
                            Box::new(
                                // TODO: reject if a mail from was already given
                                // TODO: actually call filter_from
                                send_reply(writer, ReplyCode::OKAY, b"Okay").and_then(|writer| {
                                    future::ok((
                                        writer,
                                        conn_meta,
                                        Some(MailMetadata {
                                            from,
                                            to: Vec::new(),
                                        }),
                                    ))
                                }),
                            ) as Box<Future<Item = _, Error = ()>>
                        }
                        Ok(_) => Box::new(
                            // TODO: look for a way to eliminate this alloc
                            send_reply(
                                writer,
                                ReplyCode::COMMAND_UNIMPLEMENTED,
                                b"Command not implemented",
                            ).and_then(|writer| future::ok((writer, conn_meta, mail_meta))),
                        ) as Box<Future<Item = _, Error = ()>>,
                        Err(_) => Box::new(
                            send_reply(
                                writer,
                                ReplyCode::COMMAND_UNRECOGNIZED,
                                b"Command not recognized",
                            ).and_then(|writer| future::ok((writer, conn_meta, mail_meta))),
                        ) as Box<Future<Item = _, Error = ()>>,
                    }
                },
            )
            .map(|_| ()), // TODO: warn of unfinished commands?
    )
}

// Panics if `text` has a byte not in {9} \union [32; 126]
fn send_reply<'a, W>(
    writer: W,
    code: ReplyCode,
    text: &'a [u8],
) -> Box<'a + Future<Item = W, Error = W::SinkError>>
where
    W: 'a + Sink<SinkItem = Reply<'a>>,
    W::SinkError: 'a,
{
    let replies = map_is_last(text.chunks(Reply::MAX_LEN), move |t, l| {
        Reply::build(code, if l { IsLastLine::Yes } else { IsLastLine::No }, t).unwrap()
    });
    Box::new(writer.send_all(stream::iter_ok(replies)).map(|(w, _)| w))
}

// TODO: maybe it'd be possible to use upstream buffers instead of re-buffering
// here, for fewer copies
struct CrlfLines<S> {
    source:   S,
    cur_line: Vec<u8>,
}

impl<S: Stream<Item = u8>> CrlfLines<S> {
    pub fn new(s: S) -> CrlfLines<S> {
        CrlfLines {
            source:   s,
            cur_line: Self::initial_cur_line(),
        }
    }

    pub fn underlying(&mut self) -> &mut S {
        &mut self.source
    }

    fn initial_cur_line() -> Vec<u8> {
        Vec::with_capacity(1024)
    }

    fn next_line(&mut self) -> Vec<u8> {
        mem::replace(&mut self.cur_line, Self::initial_cur_line())
    }
}

impl<S: Stream<Item = u8>> Stream for CrlfLines<S> {
    type Item = Vec<u8>;
    type Error = S::Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        use Async::*;
        loop {
            match self.source.poll()? {
                NotReady => return Ok(NotReady),
                Ready(None) if self.cur_line.is_empty() => return Ok(Ready(None)),
                Ready(None) => return Ok(Ready(Some(self.next_line()))),
                Ready(Some(c)) => {
                    self.cur_line.push(c);
                    let l = self.cur_line.len();
                    if c == b'\n' && l >= 2 && self.cur_line[l - 2] == b'\r' {
                        return Ok(Ready(Some(self.next_line())));
                    }
                }
            }
        }
    }
}

struct MapIsLast<I: Iterator, F> {
    iter: std::iter::Peekable<I>,
    f:    F,
}

impl<R, I: Iterator, F: FnMut(I::Item, bool) -> R> Iterator for MapIsLast<I, F> {
    type Item = R;

    #[inline]
    fn next(&mut self) -> Option<R> {
        let res = self.iter.next();
        let is_last = self.iter.peek().is_none();
        res.map(|x| (self.f)(x, is_last))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

fn map_is_last<R, I: Iterator, F: FnMut(I::Item, bool) -> R>(iter: I, f: F) -> MapIsLast<I, F> {
    MapIsLast {
        iter: iter.peekable(),
        f,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlflines_looks_good() {
        let stream = CrlfLines::new(
            stream::iter_ok(
                b"MAIL FROM:<foo@bar.example.org>\r\n\
                  RCPT TO:<baz@quux.example.org>\r\n\
                  RCPT TO:<foo2@bar.example.org>\r\n\
                  DATA\r\n\
                  Hello World\r\n\
                  .\r\n\
                  QUIT\r\n"
                    .iter()
                    .cloned(),
            ).map_err(|()| ()),
        );

        assert_eq!(
            stream.collect().wait().unwrap(),
            vec![
                b"MAIL FROM:<foo@bar.example.org>\r\n".to_vec(),
                b"RCPT TO:<baz@quux.example.org>\r\n".to_vec(),
                b"RCPT TO:<foo2@bar.example.org>\r\n".to_vec(),
                b"DATA\r\n".to_vec(),
                b"Hello World\r\n".to_vec(),
                b".\r\n".to_vec(),
                b"QUIT\r\n".to_vec(),
            ]
        );
    }

    #[test]
    fn interacts_ok() {
        let tests: &[(&[u8], &[u8])] = &[
            (
                b"MAIL FROM:<foo@bar.example.org>\r\n\
                  RCPT TO:<baz@quux.example.org>\r\n\
                  RCPT TO:<foo2@bar.example.org>\r\n\
                  DATA\r\n\
                  Hello World\r\n\
                  .\r\n\
                  QUIT\r\n",
                b"250 Okay\r\n\
                  502 Command not implemented\r\n\
                  502 Command not implemented\r\n\
                  502 Command not implemented\r\n\
                  500 Command not recognized\r\n\
                  500 Command not recognized\r\n\
                  502 Command not implemented\r\n",
            ),
            (b"HELP hello\r\n", b"502 Command not implemented\r\n"),
        ];
        for &(inp, out) in tests {
            let mut vec = Vec::new();
            interact(
                stream::iter_ok(inp.iter().cloned()),
                &mut vec,
                "no metadata",
                |()| (),
                |()| (),
                |_, _| Decision::Accept(()),
                |_, _, _, _| Decision::Accept(()),
                |_, _, _, _| {
                    Decision::Reject(Refusal {
                        code: ReplyCode::POLICY_REASON,
                        msg:  "foo".to_owned(),
                    })
                },
            ).wait()
                .unwrap();
            assert_eq!(vec, out);
        }
    }
}
