//! Does `AsyncReader` actually work? — a direct test of kaduceus's read/seek primitives.
//!
//! Motivation: the I/O trace over a real decode showed **zero seeks in 33 calls**, so
//! `AsyncReader::seek` has never executed. It reads plausibly, but plausible is not tested, and
//! kaduceus's own `tests/` do not compile so nothing covers it. Before anyone builds the ABI v2 pull
//! source on top of it, it needs to be known-good rather than assumed-good.
//!
//! No Kakadu here at all — `AsyncReader::new` is public and takes any `AsyncRead + AsyncSeek`, so
//! this exercises the primitives directly over a synthetic buffer.
//!
//! **The buffer is self-describing.** It holds consecutive `u32`s little-endian, value == index, so
//! the four bytes at byte-offset `4k` decode to `k`. Any read tells you exactly where it came from,
//! which turns "did the seek land correctly?" into an equality check rather than a judgement.
//!
//! ```text
//! cp /mnt/c/git/tomcrane/jpjp/docs/kakadu-k2-reader-probe.rs ~/src/kaduceus/examples/reader-probe.rs
//! cd ~/src/kaduceus && cargo run --release --example reader-probe
//! ```
//!
//! An `example` rather than a `#[test]`, deliberately: `cargo test` cannot build in this crate while
//! `tests/decompression.rs` is stale against the current API.

use std::sync::Arc;

use futures::io::Cursor;
use kaduceus::AsyncReader;
use tokio::runtime::Runtime;

const WORDS: usize = 65_536; // 256 KiB, comfortably more than one read

fn corpus() -> Vec<u8> {
    let mut v = Vec::with_capacity(WORDS * 4);
    for i in 0..WORDS as u32 {
        v.extend_from_slice(&i.to_le_bytes());
    }
    v
}

fn reader(rt: &Arc<Runtime>, data: &[u8]) -> AsyncReader {
    AsyncReader::new(
        Arc::clone(rt),
        Cursor::new(data.to_vec()),
        tracing::Span::none(),
    )
}

/// The word index encoded at the start of `buf` — i.e. where the read actually landed.
fn word_at(buf: &[u8]) -> u32 {
    u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])
}

struct Report {
    pass: u32,
    fail: u32,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: String) {
        if ok {
            self.pass += 1;
            println!("  PASS  {name}  ({detail})");
        } else {
            self.fail += 1;
            println!("  FAIL  {name}  ({detail})");
        }
    }
}

fn main() {
    let rt = Arc::new(Runtime::new().expect("tokio runtime"));
    let data = corpus();
    let mut r = Report { pass: 0, fail: 0 };

    println!("AsyncReader direct probe — {} bytes, u32 LE index at every 4-byte offset\n", data.len());

    // ---- 1. sequential read from the start ------------------------------------------------
    {
        let mut rd = reader(&rt, &data);
        let mut buf = [0u8; 16];
        let n = rd.read(&mut buf).expect("read");
        r.check(
            "read from start",
            n == 16 && word_at(&buf) == 0,
            format!("n={n}, first word={}", word_at(&buf)),
        );
    }

    // ---- 2. reads advance the cursor ------------------------------------------------------
    {
        let mut rd = reader(&rt, &data);
        let mut buf = [0u8; 16];
        rd.read(&mut buf).expect("read");
        let n = rd.read(&mut buf).expect("read");
        r.check(
            "second read continues",
            n == 16 && word_at(&buf) == 4,
            format!("n={n}, first word={} (want 4)", word_at(&buf)),
        );
    }

    // ---- 3. THE ONE THAT MATTERS: seek forward, then read ---------------------------------
    // If Kakadu's seek is absolute-from-start (the assumption the pull source rests on) then
    // seeking to byte 4000 must yield word 1000.
    {
        let mut rd = reader(&rt, &data);
        let pos = rd.seek(4000).expect("seek");
        let mut buf = [0u8; 4];
        let n = rd.read(&mut buf).expect("read");
        r.check(
            "seek(4000) then read",
            pos == 4000 && n == 4 && word_at(&buf) == 1000,
            format!("returned pos={pos}, n={n}, word={} (want 1000)", word_at(&buf)),
        );
    }

    // ---- 4. seek BACKWARDS -----------------------------------------------------------------
    // A decoder revisiting a header needs this, and a source that only ever moves forward would
    // pass every test above and fail here.
    {
        let mut rd = reader(&rt, &data);
        rd.seek(40_000).expect("seek");
        let pos = rd.seek(400).expect("seek back");
        let mut buf = [0u8; 4];
        rd.read(&mut buf).expect("read");
        r.check(
            "seek backwards",
            pos == 400 && word_at(&buf) == 100,
            format!("returned pos={pos}, word={} (want 100)", word_at(&buf)),
        );
    }

    // ---- 5. rewind to 0 --------------------------------------------------------------------
    {
        let mut rd = reader(&rt, &data);
        let mut buf = [0u8; 4];
        rd.read(&mut buf).expect("read");
        rd.seek(0).expect("rewind");
        rd.read(&mut buf).expect("read");
        r.check(
            "seek(0) rewinds",
            word_at(&buf) == 0,
            format!("word={} (want 0)", word_at(&buf)),
        );
    }

    // ---- 6. seek to the last word ----------------------------------------------------------
    {
        let mut rd = reader(&rt, &data);
        let last = (WORDS as i64 - 1) * 4;
        rd.seek(last).expect("seek");
        let mut buf = [0u8; 4];
        let n = rd.read(&mut buf).expect("read");
        r.check(
            "seek to final word",
            n == 4 && word_at(&buf) == WORDS as u32 - 1,
            format!("n={n}, word={} (want {})", word_at(&buf), WORDS - 1),
        );
    }

    // ---- 7. seek PAST the end --------------------------------------------------------------
    // Expected: the seek succeeds (Cursor allows it) and the read returns 0. Recorded so the
    // behaviour is documented rather than discovered later.
    {
        let mut rd = reader(&rt, &data);
        let past = data.len() as i64 + 4096;
        let seek_result = rd.seek(past);
        let mut buf = [0u8; 4];
        let read_result = rd.read(&mut buf);
        println!(
            "  INFO  seek past EOF -> seek={seek_result:?}, read={read_result:?}  \
             (a 0-byte read here is EOF, not an error)"
        );
    }

    // ---- 8. NEGATIVE OFFSET — the suspected silent-corruption path -------------------------
    // `seek` takes i64 and does `offset as u64`, so -1 becomes u64::MAX. Cursor accepts that
    // happily and every later read returns 0 bytes. If Kakadu ever passes a negative or
    // relative offset, the decoder is fed nothing and sees a truncated codestream — which is
    // the shape of the "illegal inclusion tag tree" failure we are chasing on the BL master.
    {
        let mut rd = reader(&rt, &data);
        let seek_result = rd.seek(-1);
        let mut buf = [0u8; 4];
        let read_result = rd.read(&mut buf);
        let silently_wrong = seek_result.is_ok() && matches!(read_result, Ok(0));
        r.check(
            "negative offset is REJECTED",
            !silently_wrong,
            format!(
                "seek={seek_result:?}, read={read_result:?}{}",
                if silently_wrong {
                    " <- wraps to u64::MAX and reads nothing, with no error"
                } else {
                    ""
                }
            ),
        );
    }

    // ---- 9. a read larger than the remaining bytes -----------------------------------------
    // `read` loops until the buffer is full or EOF. Confirm a short read is reported honestly,
    // because Kakadu's whole-object buffering depends on the final short read being accurate.
    {
        let mut rd = reader(&rt, &data);
        rd.seek(data.len() as i64 - 10).expect("seek");
        let mut buf = [0u8; 4096];
        let n = rd.read(&mut buf).expect("read");
        r.check(
            "short read near EOF reports the true count",
            n == 10,
            format!("n={n} (want 10)"),
        );
    }

    println!("\n{} passed, {} failed", r.pass, r.fail);
    if r.fail > 0 {
        println!("\nA failure here is worth more than a passing decode: it means the pull source in");
        println!("ABI v2 §7a cannot be built on these primitives until it is fixed.");
        std::process::exit(1);
    }
}
