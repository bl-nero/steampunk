use std::collections::VecDeque;

/// A buffer that enables elements to be retrieved after a specified delay after
/// they were put inside. The delay is measured in number of elements being
/// pushed.
#[derive(Debug)]
pub struct DelayBuffer<T: Default + Clone> {
    buf: VecDeque<T>,
    /// If set to `true`, the buffer behaves as a 0-sized buffer.
    immediate: bool,
}

impl<T: Default + Clone> DelayBuffer<T> {
    /// Creates a new `DelayBuffer` with a given size. The size determines the
    /// delay.
    pub fn new(size: usize) -> Self {
        Self {
            buf: VecDeque::from(vec![T::default(); std::cmp::max(size, 1)]),
            immediate: size == 0,
        }
    }

    /// Adds an item to the buffer, and retrieves the oldest item.
    pub fn shift(&mut self, item: T) -> T {
        let output = self.buf.pop_front().expect("Empty buffer");
        if self.immediate {
            self.buf.push_back(item.clone());
            return item;
        } else {
            self.buf.push_back(item);
            return output;
        }
    }

    /// Peeks the oldest item in the buffer.
    pub fn peek(&self) -> &T {
        self.buf.front().expect("Empty buffer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift() {
        let mut buffer: DelayBuffer<i32> = DelayBuffer::new(3);
        buffer.shift(4);
        buffer.shift(5);
        buffer.shift(6);
        assert_eq!(buffer.shift(7), 4);
        assert_eq!(buffer.shift(8), 5);
        assert_eq!(buffer.shift(9), 6);
        assert_eq!(buffer.shift(10), 7);
        assert_eq!(buffer.shift(11), 8);
    }

    #[test]
    fn peek() {
        let mut buffer: DelayBuffer<i32> = DelayBuffer::new(2);
        buffer.shift(4);
        buffer.shift(5);
        assert_eq!(*buffer.peek(), 4);

        buffer.shift(6);
        assert_eq!(*buffer.peek(), 5);
    }

    #[test]
    fn zero_delay() {
        let mut buffer: DelayBuffer<i32> = DelayBuffer::new(0);
        assert_eq!(buffer.shift(3), 3);
        assert_eq!(buffer.shift(8), 8);
        assert_eq!(*buffer.peek(), 8);
    }
}
