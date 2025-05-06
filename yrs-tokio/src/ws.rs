use crate::conn::Connection;
use crate::AwarenessRef;
use futures_util::{Sink, Stream};
use std::pin::Pin;
use std::task::{Context, Poll};
use yrs::sync::Error;

/// Connection Wrapper over a generic stream, which implements a Yjs/Yrs awareness and update exchange
/// protocol.
#[repr(transparent)]
#[derive(Debug)]
pub struct YrsConn<S, T>(Connection<S, T>);

impl<S, T> YrsConn<S, T>
where
    S: Sink<Vec<u8>, Error = Error> + Unpin + Send + Sync + 'static,
    T: Stream<Item = Result<Vec<u8>, Error>> + Unpin + Send + Sync + 'static,
{
    pub fn new(awareness: AwarenessRef, sink: S, stream: T) -> Self {
        let conn = Connection::new(awareness, sink, stream);
        YrsConn(conn)
    }
}

impl<S, T> Future for YrsConn<S, T>
where
    S: Sink<Vec<u8>, Error = Error> + Unpin + Send + Sync + 'static,
    T: Stream<Item = Result<Vec<u8>, Error>> + Unpin + Send + Sync + 'static,
{
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Other(e.into()))),
            Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
        }
    }
}

/// A generic sink wrapper that implements futures `Sink` in a way that makes it compatible
/// with y-sync protocol.
#[repr(transparent)]
#[derive(Debug)]
pub struct YrsSink<S>(S);

impl<S> From<S> for YrsSink<S> {
    fn from(sink: S) -> Self {
        YrsSink(sink)
    }
}

impl<S> Sink<Vec<u8>> for YrsSink<S>
where
    S: Sink<Vec<u8>, Error = Error> + Unpin + Send + Sync + 'static,
{
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.0).poll_ready(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Other(e.into()))),
            Poll::Ready(_) => Poll::Ready(Ok(())),
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        if let Err(e) = Pin::new(&mut self.0).start_send(item) {
            Err(Error::Other(e.into()))
        } else {
            Ok(())
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.0).poll_flush(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Other(e.into()))),
            Poll::Ready(_) => Poll::Ready(Ok(())),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match Pin::new(&mut self.0).poll_close(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Other(e.into()))),
            Poll::Ready(_) => Poll::Ready(Ok(())),
        }
    }
}

/// A generic stream wrapper that implements futures `Stream` in a way that makes it compatible
/// with y-sync protocol.
#[derive(Debug)]
pub struct YrsStream<T>(T);

impl<T> From<T> for YrsStream<T> {
    fn from(stream: T) -> Self {
        YrsStream(stream)
    }
}

impl<T> Stream for YrsStream<T>
where
    T: Stream<Item = Result<Vec<u8>, Error>> + Unpin + Send + Sync + 'static,
{
    type Item = Result<Vec<u8>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(res)) => Poll::Ready(Some(res)),
        }
    }
}
