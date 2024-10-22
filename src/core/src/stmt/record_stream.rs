use super::*;

use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use tokio_stream::{Stream, StreamExt};

pub struct RecordStream<'stmt> {
    next: Option<Record<'stmt>>,
    inner: Pin<Box<dyn Stream<Item = crate::Result<Record<'stmt>>> + Send + 'stmt>>,
}

impl<'stmt> RecordStream<'stmt> {
    /// Returns an empty record stream
    pub fn empty() -> RecordStream<'stmt> {
        RecordStream::from_iter(std::iter::empty())
    }

    pub fn single(row: Record<'stmt>) -> RecordStream<'stmt> {
        RecordStream::from_iter(Some(Ok(row)).into_iter())
    }

    pub fn from_stream<T: Stream<Item = crate::Result<Record<'stmt>>> + Send + 'stmt>(
        stream: T,
    ) -> RecordStream<'stmt> {
        RecordStream {
            next: None,
            inner: Box::pin(stream),
        }
    }

    pub fn from_vec(records: Vec<Record<'stmt>>) -> RecordStream<'stmt> {
        RecordStream::from_iter(records.into_iter().map(Ok))
    }

    pub fn from_iter<T: Iterator<Item = crate::Result<Record<'stmt>>> + Send + 'stmt>(
        iter: T,
    ) -> RecordStream<'stmt> {
        RecordStream::from_stream(Iter { iter })
    }

    /// Returns the next record in the stream
    pub async fn next(&mut self) -> Option<crate::Result<Record<'stmt>>> {
        StreamExt::next(self).await
    }

    /// Peek at the next record in the stream
    pub async fn peek(&mut self) -> Option<crate::Result<&Record<'stmt>>> {
        if self.next.is_none() {
            match self.next().await {
                Some(Ok(record)) => self.next = Some(record),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }

        self.next.as_ref().map(Ok)
    }

    /// Force the stream to preload at least one record, if there are more
    /// records to stream.
    pub async fn tap(&mut self) -> crate::Result<()> {
        if let Some(Err(e)) = self.peek().await {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// The stream will contain at least this number of elements
    pub fn min_len(&self) -> usize {
        let (ret, _) = self.size_hint();
        ret
    }

    pub async fn collect(mut self) -> crate::Result<Vec<Record<'stmt>>> {
        let mut ret = Vec::with_capacity(self.min_len());

        while let Some(res) = self.next().await {
            ret.push(res?);
        }

        Ok(ret)
    }
}

impl<'stmt> Stream for RecordStream<'stmt> {
    type Item = crate::Result<Record<'stmt>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(next) = self.next.take() {
            Poll::Ready(Some(Ok(next)))
        } else {
            Pin::new(&mut self.inner).poll_next(cx)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'stmt> From<Record<'stmt>> for RecordStream<'stmt> {
    fn from(src: Record<'stmt>) -> RecordStream<'stmt> {
        RecordStream::single(src)
    }
}

#[derive(Debug)]
pub struct Iter<I> {
    iter: I,
}

impl<I> Unpin for Iter<I> {}

impl<I> Stream for Iter<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<I::Item>> {
        Poll::Ready(self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'stmt> fmt::Debug for RecordStream<'stmt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordStream").finish()
    }
}
