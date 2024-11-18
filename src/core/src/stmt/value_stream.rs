use super::*;

use std::{
    collections::VecDeque,
    fmt,
    marker::PhantomData,
    mem,
    pin::Pin,
    task::{Context, Poll},
};
use tokio_stream::{Stream, StreamExt};

pub struct ValueStream<'stmt> {
    buffer: Buffer<'stmt>,
    stream: Option<DynStream<'stmt>>,
}

#[derive(Debug)]
struct Iter<'stmt, I> {
    iter: I,
    _m: PhantomData<&'stmt ()>,
}

#[derive(Clone)]
enum Buffer<'stmt> {
    Empty,
    One(Value<'stmt>),
    Many(VecDeque<Value<'stmt>>),
}

type DynStream<'stmt> = Pin<Box<dyn Stream<Item = crate::Result<Value<'stmt>>> + Send + 'stmt>>;

impl<'stmt> ValueStream<'stmt> {
    pub fn new() -> ValueStream<'stmt> {
        ValueStream {
            buffer: Buffer::Empty,
            stream: None,
        }
    }

    pub fn from_value(value: impl Into<Value<'stmt>>) -> ValueStream<'stmt> {
        ValueStream {
            buffer: Buffer::One(value.into()),
            stream: None,
        }
    }

    pub fn from_stream<T: Stream<Item = crate::Result<Value<'stmt>>> + Send + 'stmt>(
        stream: T,
    ) -> ValueStream<'stmt> {
        ValueStream {
            buffer: Buffer::Empty,
            stream: Some(Box::pin(stream)),
        }
    }

    pub fn from_vec(records: Vec<Value<'stmt>>) -> ValueStream<'stmt> {
        ValueStream {
            buffer: Buffer::Many(records.into()),
            stream: None,
        }
    }

    pub fn from_iter<T, I>(iter: I) -> ValueStream<'stmt>
    where
        T: Into<Value<'stmt>>,
        I: Iterator<Item = crate::Result<T>> + Send + 'stmt,
    {
        ValueStream::from_stream(Iter {
            iter,
            _m: PhantomData,
        })
    }

    /// Returns the next record in the stream
    pub async fn next(&mut self) -> Option<crate::Result<Value<'stmt>>> {
        StreamExt::next(self).await
    }

    /// Peek at the next record in the stream
    pub async fn peek(&mut self) -> Option<crate::Result<&Value<'stmt>>> {
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

    pub async fn collect(mut self) -> crate::Result<Vec<Value<'stmt>>> {
        let mut ret = Vec::with_capacity(self.min_len());

        while let Some(res) = self.next().await {
            ret.push(res?);
        }

        Ok(ret)
    }

    pub async fn dup(&mut self) -> crate::Result<ValueStream<'stmt>> {
        self.buffer().await?;

        Ok(ValueStream {
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

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Value<'stmt>> {
        assert!(self.stream.is_none());

        // TODO: don't box
        match &mut self.buffer {
            Buffer::Empty => Box::new(None.into_iter()),
            Buffer::One(v) => Box::new(Some(v).into_iter()),
            Buffer::Many(v) => {
                Box::new(v.iter_mut()) as Box<dyn Iterator<Item = &mut Value<'stmt>>>
            }
        }
    }
}

impl<'stmt> Stream for ValueStream<'stmt> {
    type Item = crate::Result<Value<'stmt>>;

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

impl<'stmt> From<Value<'stmt>> for ValueStream<'stmt> {
    fn from(src: Value<'stmt>) -> ValueStream<'stmt> {
        ValueStream {
            buffer: Buffer::One(src),
            stream: None,
        }
    }
}

impl<'stmt> From<Vec<Value<'stmt>>> for ValueStream<'stmt> {
    fn from(value: Vec<Value<'stmt>>) -> Self {
        ValueStream::from_vec(value)
    }
}

impl<'stmt, I> Unpin for Iter<'stmt, I> {}

impl<'stmt, T, I> Stream for Iter<'stmt, I>
where
    I: Iterator<Item = crate::Result<T>>,
    T: Into<Value<'stmt>>,
{
    type Item = crate::Result<Value<'stmt>>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.iter.next().map(|res| res.map(|item| item.into())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'stmt> fmt::Debug for ValueStream<'stmt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordStream").finish()
    }
}

impl<'stmt> Buffer<'stmt> {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        match self {
            Buffer::Empty => 0,
            Buffer::One(_) => 1,
            Buffer::Many(v) => v.len(),
        }
    }

    fn first(&self) -> Option<&Value<'stmt>> {
        match self {
            Buffer::Empty => None,
            Buffer::One(value) => Some(value),
            Buffer::Many(values) => values.front(),
        }
    }

    fn next(&mut self) -> Option<Value<'stmt>> {
        match self {
            Buffer::Empty => None,
            Buffer::One(_) => {
                let Buffer::One(value) = mem::take(self) else {
                    panic!()
                };
                Some(value)
            }
            Buffer::Many(values) => values.pop_front(),
        }
    }

    fn push(&mut self, value: Value<'stmt>) {
        match self {
            Buffer::Empty => {
                *self = Buffer::One(value);
            }
            Buffer::One(_) => {
                let Buffer::One(first) =
                    mem::replace(self, Buffer::Many(VecDeque::with_capacity(2)))
                else {
                    panic!()
                };

                let Buffer::Many(values) = self else { panic!() };

                values.push_back(first);
                values.push_back(value);
            }
            Buffer::Many(values) => {
                values.push_back(value);
            }
        }
    }
}

impl<'stmt> Default for Buffer<'stmt> {
    fn default() -> Self {
        Buffer::Empty
    }
}
