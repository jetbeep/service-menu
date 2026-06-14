use alloc::vec::Vec;

#[derive(Debug, Default)]
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn last(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}
