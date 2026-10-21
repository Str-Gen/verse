//! Bounded, repeatable workloads for CPU/allocation profiling of the shared reader.
use reader_core::book_store::DiskBook;
use std::{error::Error, hint::black_box, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: profile_reader BOOK.json [open|window|piece] [iterations]")?;
    let mode = args.get(2).map_or("window", String::as_str);
    let iterations: usize = args.get(3).map_or(Ok(100), |s| s.parse())?;
    if !matches!(mode, "open" | "window" | "piece") || iterations == 0 {
        return Err("invalid workload or iteration count".into());
    }
    let started = Instant::now();
    let book = DiskBook::open(Path::new(path))?;
    let open = started.elapsed();
    let total = book.total_pieces();
    if total == 0 {
        return Err("empty fixture".into());
    }
    let started = Instant::now();
    for i in 0..iterations {
        let at = i.wrapping_mul(7919) % total;
        match mode {
            "open" => {
                black_box(DiskBook::open(Path::new(path))?);
            }
            "window" => {
                black_box(book.load_window(at)?);
            }
            _ => {
                black_box(book.piece(at)?);
            }
        }
    }
    println!(
        "mode={mode} iterations={iterations} pieces={total} open_ms={:.3} work_ms={:.3}",
        open.as_secs_f64() * 1000.0,
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
