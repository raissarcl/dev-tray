use std::collections::VecDeque;

#[derive(Debug, Clone)]
#[allow(dead_code)] // ring-buffer helpers for future log UI
pub struct LogBuffer {
    lines: VecDeque<String>,
    capacity: usize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
        }
    }

    pub fn append(&mut self, line: impl Into<String>) {
        if self.lines.len() >= self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line.into());
    }

    pub fn append_chunk(&mut self, chunk: &str) {
        for line in chunk.lines() {
            self.append(line.to_string());
        }
        if chunk.ends_with('\n') {
            // lines() drops trailing empty; nothing extra needed
        }
    }

    pub fn snapshot(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
