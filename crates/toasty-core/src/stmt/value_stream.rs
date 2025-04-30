use super::*;

use std::{
    collections::VecDeque,
    fmt, mem,
    pin::Pin,
    task::{Context, Poll},
};
use tokio_stream::{Stream, StreamExt};

#[derive(Default)]
pub struct ValueStream {
    buffer: Buffer,
    stream: Option<DynStream>,
}

#[derive(Debug)]
struct Iter<I> {
    iter: I,
}

#[derive(Clone, Default, PartialEq)]
enum Buffer {
    #[default]
    Empty,
    One(Value),
    Many(VecDeque<Value>),
}

type DynStream = Pin<Box<dyn Stream<Item = crate::Result<Value>> + Send + 'static>>;

impl ValueStream {
    pub fn from_value(value: impl Into<Value>) -> Self {
        Self {
            buffer: Buffer::One(value.into()),
            stream: None,
        }
    }

    pub fn from_stream<T: Stream<Item = crate::Result<Value>> + Send + 'static>(stream: T) -> Self {
        Self {
            buffer: Buffer::Empty,
            stream: Some(Box::pin(stream)),
        }
    }

    pub fn from_vec(records: Vec<Value>) -> Self {
        Self {
            buffer: Buffer::Many(records.into()),
            stream: None,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<T, I>(iter: I) -> Self
    where
        T: Into<Value>,
        I: Iterator<Item = crate::Result<T>> + Send + 'static,
    {
        Self::from_stream(Iter { iter })
    }

    /// Returns the next record in the stream
    pub async fn next(&mut self) -> Option<crate::Result<Value>> {
        StreamExt::next(self).await
    }

    /// Peek at the next record in the stream
    pub async fn peek(&mut self) -> Option<crate::Result<&Value>> {
        if self.buffer.is_empty() {
            match self.next().await {
                Some(Ok(value)) => self.buffer.push(value),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }

        self.buffer.first().map(Ok)
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

    pub async fn collect(mut self) -> crate::Result<Vec<Value>> {
        let mut ret = Vec::with_capacity(self.min_len());

        while let Some(res) = self.next().await {
            ret.push(res?);
        }

        Ok(ret)
    }

    pub async fn dup(&mut self) -> crate::Result<Self> {
        self.buffer().await?;

        Ok(Self {
            buffer: self.buffer.clone(),
            stream: None,
        })
    }

    pub async fn buffer(&mut self) -> crate::Result<()> {
        if let Some(stream) = &mut self.stream {
            while let Some(res) = stream.next().await {
                let value = res?;
                self.buffer.push(value);
            }
        }

        Ok(())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Value> {
        assert!(self.stream.is_none());

        // TODO: don't box
        match &mut self.buffer {
            Buffer::Empty => Box::new(None.into_iter()),
            Buffer::One(v) => Box::new(Some(v).into_iter()),
            Buffer::Many(v) => Box::new(v.iter_mut()) as Box<dyn Iterator<Item = &mut Value>>,
        }
    }

    // NOTE: this method is only used for testing purposes. It should not ever be made
    // available via the public API.
    #[cfg(test)]
    fn into_inner(self) -> (Buffer, Option<DynStream>) {
        (self.buffer, self.stream)
    }
}

impl Stream for ValueStream {
    type Item = crate::Result<Value>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(next) = self.buffer.next() {
            Poll::Ready(Some(Ok(next)))
        } else if let Some(stream) = self.stream.as_mut() {
            Pin::new(stream).poll_next(cx)
        } else {
            Poll::Ready(None)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (mut low, mut high) = match &self.stream {
            Some(stream) => stream.size_hint(),
            None => (0, Some(0)),
        };

        let buffered = self.buffer.len();

        low += buffered;

        if let Some(high) = high.as_mut() {
            *high += buffered;
        }

        (low, high)
    }
}

impl From<Value> for ValueStream {
    fn from(src: Value) -> Self {
        Self {
            buffer: Buffer::One(src),
            stream: None,
        }
    }
}

impl From<Vec<Value>> for ValueStream {
    fn from(value: Vec<Value>) -> Self {
        Self::from_vec(value)
    }
}

impl<I> Unpin for Iter<I> {}

impl<T, I> Stream for Iter<I>
where
    I: Iterator<Item = crate::Result<T>>,
    T: Into<Value>,
{
    type Item = crate::Result<Value>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.iter.next().map(|res| res.map(|item| item.into())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl fmt::Debug for ValueStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordStream").finish()
    }
}

impl Buffer {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }

    fn first(&self) -> Option<&Value> {
        match self {
            Self::Empty => None,
            Self::One(value) => Some(value),
            Self::Many(values) => values.front(),
        }
    }

    fn next(&mut self) -> Option<Value> {
        match self {
            Self::Empty => None,
            Self::One(_) => {
                let Self::One(value) = mem::take(self) else {
                    panic!()
                };
                Some(value)
            }
            Self::Many(values) => values.pop_front(),
        }
    }

    fn push(&mut self, value: Value) {
        match self {
            Self::Empty => {
                *self = Self::One(value);
            }
            Self::One(_) => {
                let Self::One(first) = mem::replace(self, Self::Many(VecDeque::with_capacity(2)))
                else {
                    panic!()
                };

                let Self::Many(values) = self else { panic!() };

                values.push_back(first);
                values.push_back(value);
            }
            Self::Many(values) => {
                values.push_back(value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() {
        let (buffer, stream) = ValueStream::default().into_inner();
        assert!(buffer == Buffer::Empty);
        assert!(stream.is_none());
    }
}
