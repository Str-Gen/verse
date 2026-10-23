//! Both shells use the same local vocabulary process and its durable SQLite store.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

pub struct Vocabulary {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Vocabulary {
    pub fn open(root: &Path, book: &crate::Book) -> Result<Self, String> {
        let mut child = Command::new("python3")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/vocabulary/worker.py"))
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot start vocabulary: {e}"))?;
        let input = child.stdin.take().ok_or("Vocabulary input unavailable")?;
        let output = BufReader::new(child.stdout.take().ok_or("Vocabulary output unavailable")?);
        let mut result = Self {
            child,
            input: BufWriter::new(input),
            output,
        };
        result.request(&json!({"action":"open", "book": {"book_id":book.book_id, "title":book.title, "documents":book.documents, "pieces":book.pieces}}))?;
        Ok(result)
    }
    pub fn request(&mut self, request: &Value) -> Result<Value, String> {
        serde_json::to_writer(&mut self.input, request).map_err(|e| e.to_string())?;
        self.input
            .write_all(b"\n")
            .and_then(|()| self.input.flush())
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        let response: Value = serde_json::from_str(&line)
            .map_err(|e| format!("Vocabulary stopped responding: {e}"))?;
        if let Some(error) = response["error"].as_str() {
            return Err(error.into());
        }
        Ok(response["result"].clone())
    }
}
impl Drop for Vocabulary {
    fn drop(&mut self) {
        // Captures are committed before ranking starts; interrupted jobs remain retryable.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
