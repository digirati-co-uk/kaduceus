//! K2 probe — answers §7's three unknowns directly against kaduceus.
//!
//! No facade, no C ABI, no jpjp. If a result here is wrong, there is exactly one place it can be
//! wrong, which is the entire point of running this before writing the facade.
//!
//! Install and run (from your WSL shell):
//!
//! ```text
//! mkdir -p ~/src/kaduceus/examples
//! cp /mnt/c/git/tomcrane/jpjp/docs/kakadu-k2-probe.rs ~/src/kaduceus/examples/probe.rs
//! cd ~/src/kaduceus
//! cargo run --release --example probe -- /mnt/c/path/to/some.jp2 [output-dir]
//! ```
//!
//! Everything it needs is already in kaduceus's manifest -- `tokio` and `futures` are
//! dependencies, `image` is a dev-dependency, and examples can use both -- so no manifest edit is
//! required. Pass a second argument to write the PNGs somewhere Windows can reach.

use std::error::Error;
use std::sync::Arc;

use futures::io::Cursor;
use kaduceus::{KakaduContext, KakaduImage, Region};
use tokio::runtime::Runtime;

/// A fresh image per experiment. Slightly wasteful, deliberately: a diagnostic must not let state
/// from one decode leak into the reading of the next.
fn open_image(rt: &Arc<Runtime>, bytes: &[u8]) -> Result<KakaduImage, Box<dyn Error>> {
    KakaduImage::new(
        Arc::clone(rt),
        KakaduContext::default(),
        Cursor::new(bytes.to_vec()),
        Some("probe".to_string()),
    )
}

/// Decode one region. `bytes_per_pixel` is a GUESS used only to size the buffer — experiment 1
/// measures the real value, so pass something generous until you know it.
fn decode(
    rt: &Arc<Runtime>,
    bytes: &[u8],
    region: Region,
    scaled_w: u32,
    scaled_h: u32,
    bytes_per_pixel: usize,
) -> Result<(Vec<u8>, Region), Box<dyn Error>> {
    let mut img = open_image(rt, bytes)?;
    let mut buf = vec![0u8; scaled_w as usize * scaled_h as usize * bytes_per_pixel];
    let mut decompressor = img.open_region(region, scaled_w, scaled_h)?;
    let out = decompressor.process(&mut buf)?;
    Ok((buf, out))
}

/// Report a failure and carry on. The whole point of making the bridge fallible is that one bad
/// decode no longer costs us the rest of the run.
fn try_decode(
    label: &str,
    r: Result<(Vec<u8>, Region), Box<dyn Error>>,
) -> Option<(Vec<u8>, Region)> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            println!("!! {label} FAILED: {e}");
            None
        }
    }
}

/// Index just past the last non-zero byte. With a zeroed buffer this is how much was actually
/// written, which is what tells us bytes-per-pixel and whether the decode finished.
fn filled_len(buf: &[u8]) -> usize {
    buf.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1)
}

fn hex(buf: &[u8], n: usize) -> String {
    buf.iter()
        .take(n)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// PNG, because netpbm is a Unix-ism that Windows viewers generally refuse. `image` is already in
/// kaduceus's `[dev-dependencies]` with the `png` feature, and examples can use dev-dependencies,
/// so this costs no manifest change.
///
/// `from_raw` + `save` rather than `save_buffer`, deliberately: the latter's colour-type argument
/// was renamed across `image` 0.24 -> 0.25, and this form is stable across both.
fn write_png(path: &str, buf: &[u8], w: u32, h: u32, bpp: usize) {
    let need = w as usize * h as usize * bpp;
    if buf.len() < need {
        println!("      (buffer shorter than {need} bytes — not saving {path})");
        return;
    }
    let data = buf[..need].to_vec();
    let saved = match bpp {
        1 => image::GrayImage::from_raw(w, h, data).map(|i| i.save(path)),
        3 => image::RgbImage::from_raw(w, h, data).map(|i| i.save(path)),
        _ => {
            let raw = path.replace(".png", ".bin");
            std::fs::write(&raw, &data).ok();
            println!("      (bpp {bpp} is not 1 or 3 — wrote raw {raw})");
            return;
        }
    };
    match saved {
        Some(Ok(())) => println!("      wrote {path}"),
        Some(Err(e)) => println!("      FAILED to write {path}: {e}"),
        None => println!("      FAILED to write {path}: buffer/size mismatch"),
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: probe <file.jp2>");
    let bytes = std::fs::read(&path).expect("could not read the JP2");
    let rt = Arc::new(Runtime::new().expect("could not start a tokio runtime"));

    // Second argument: where to write the PNGs. Defaults to /tmp, which is inside WSL and awkward
    // to reach from Windows -- pass a /mnt/c/... path to drop them somewhere Explorer can see.
    let out_dir = std::env::args().nth(2).unwrap_or_else(|| "/tmp".to_string());
    std::fs::create_dir_all(&out_dir).ok();
    println!("(images will be written to {out_dir})");

    let info = match open_image(&rt, &bytes).and_then(|mut i| i.info()) {
        Ok(i) => i,
        Err(e) => {
            println!("!! could not open image: {e}");
            return;
        }
    };
    println!("\n=== image ===");
    println!("{path}");
    println!(
        "{}x{}, tile {}x{}, dwt_levels {}",
        info.width, info.height, info.tile_width, info.tile_height, info.dwt_levels
    );
    let (w, h) = (info.width, info.height);

    // ---------------------------------------------------------------------------------------
    // §7.2 — what does process() write?
    //
    // Decode the whole image small, into a buffer sized at a deliberately over-generous 4 bytes
    // per pixel. However much comes back, divided by the pixel count, IS the bytes per pixel.
    // ---------------------------------------------------------------------------------------
    println!("\n=== §7.2  buffer layout ===");
    let (tw, th) = (64.min(w), (64u32 * h / w.max(1)).max(1));
    let Some((buf, out)) = try_decode(
        "7.2 thumbnail",
        decode(&rt, &bytes, Region { x: 0, y: 0, width: w, height: h }, tw, th, 4),
    ) else {
        return;
    };

    let filled = filled_len(&buf);
    // Against the RETURNED pixel count. Measuring against the requested one gives a nonsense
    // ratio whenever Kakadu returns a different size, which it does more often than not.
    let pixels = (out.width as usize * out.height as usize).max(1);
    println!("requested {tw}x{th}, process() returned region {out:?}");
    if out.width != tw || out.height != th {
        println!("!! SIZE MISMATCH: asked {tw}x{th}, got {}x{} — the facade must use the RETURNED size",
                 out.width, out.height);
    }
    println!("buffer {} bytes, written {filled}", buf.len());
    println!("=> {:.3} bytes per pixel", filled as f64 / pixels as f64);
    println!("first 32 bytes: {}", hex(&buf, 32));
    println!("   (repeating 3-byte pattern => interleaved RGB; smooth single ramp => greyscale)");

    let bpp = ((filled + pixels - 1) / pixels.max(1)).clamp(1, 4);
    println!("=> assuming {bpp} bytes/pixel for the rest of this run");
    // Save at the RETURNED size, not the requested one. Kakadu does not always give you the
    // dimensions you asked for, and writing rows at the wrong stride shears the image diagonally.
    write_png(&format!("{out_dir}/probe-thumb.png"), &buf, out.width, out.height, bpp);

    // ---------------------------------------------------------------------------------------
    // §7.1 — what coordinate space is Region in?
    //
    // Decisive without eyeballing anything. Both decodes ask for the SAME output size:
    //   A: region = the whole image      => full-res grid means "whole image, half scale"
    //   B: region = the top-left quarter => full-res grid means "that quarter, 1:1"
    // Those differ. But if Region were already in reduced coordinates, B would ALSO mean
    // "whole image at half scale" and the two buffers would be byte-identical.
    //
    //   A != B  =>  Region is on the FULL-RESOLUTION grid  (keep the `<< shift` in the facade)
    //   A == B  =>  Region is in REDUCED coordinates       (drop the shift)
    // ---------------------------------------------------------------------------------------
    println!("\n=== §7.1  coordinate space ===");
    let (sw, sh) = ((w / 2).max(1), (h / 2).max(1));
    let a_res = try_decode(
        "7.1 A (whole image)",
        decode(&rt, &bytes, Region { x: 0, y: 0, width: w, height: h }, sw, sh, bpp),
    );
    let b_res = try_decode(
        "7.1 B (top-left quarter)",
        decode(&rt, &bytes, Region { x: 0, y: 0, width: sw, height: sh }, sw, sh, bpp),
    );
    let (Some((a, ra)), Some((b, rb))) = (a_res, b_res) else {
        println!("(skipping the rest: 7.1 needs both decodes)");
        return;
    };
    println!("A whole image  -> {sw}x{sh}, returned {ra:?}");
    println!("B top-left qtr -> {sw}x{sh}, returned {rb:?}");
    for (tag, r) in [("A", &ra), ("B", &rb)] {
        if r.width != sw || r.height != sh {
            println!("!! {tag} SIZE MISMATCH: asked {sw}x{sh}, got {}x{}", r.width, r.height);
        }
    }
    if a == b {
        println!("=> IDENTICAL: Region is in REDUCED coordinates. Drop the `<< shift`.");
    } else {
        println!("=> DIFFERENT: Region is on the FULL-RESOLUTION grid. Keep the `<< shift`.");
    }
    write_png(&format!("{out_dir}/probe-A-whole.png"), &a, ra.width, ra.height, bpp);
    write_png(&format!("{out_dir}/probe-B-quarter.png"), &b, rb.width, rb.height, bpp);
    println!("   (look at both: A should be the whole picture, B a corner of it)");

    // Off-by-one at edges. jpjp reduces with ceil, and kaduceus has a commit about exactly this.
    // Odd sizes, away from the origin — the case a 512x512 at (0,0) will never catch.
    println!("\n--- odd region away from the origin ---");
    if w > 200 && h > 120 {
        // Region derives Debug/Eq/PartialEq/Default but NOT Clone, so build it twice rather than
        // cloning. Same reason there is no `let odd = ...` binding shared between the two calls.
        // A small sweep rather than two points: the question is whether the shortfall follows a
        // rule (a fixed set of achievable scales) or is ad hoc, and two samples cannot tell you.
        for (want_w, want_h) in [(101u32, 53u32), (51, 27), (67, 35), (25, 13)] {
            match decode(
                &rt,
                &bytes,
                Region { x: 13, y: 7, width: 101, height: 53 },
                want_w,
                want_h,
                bpp,
            ) {
                Ok((_, r)) => {
                    let flag = if r.width != want_w || r.height != want_h { "  <-- MISMATCH" } else { "" };
                    println!(
                        "asked 101x53 at (13,7) -> {want_w}x{want_h}, returned {}x{}{flag}",
                        r.width, r.height
                    );
                }
                Err(e) => println!("asked 101x53 at (13,7) -> {want_w}x{want_h}: FAILED: {e}"),
            }
        }
        println!("   (a returned width/height off by one from the request is the bug to watch for)");
    } else {
        println!("(image too small — rerun on something over 200x120)");
    }

    // ---------------------------------------------------------------------------------------
    // §7.3 — does ONE process() call finish the job?
    //
    // Exact-sized buffer, one call, then look at the tail. Kakadu's decompressor is incremental
    // and kaduceus swallows the completion flag, so a short write is invisible to the caller.
    // ---------------------------------------------------------------------------------------
    println!("\n=== §7.3  does one process() call complete? ===");
    let want = sw as usize * sh as usize * bpp;
    println!("exact buffer {want} bytes, written {}", filled_len(&a));
    if filled_len(&a) < want {
        let short = want - filled_len(&a);
        println!(
            "=> INCOMPLETE: {short} bytes ({:.1}%) of the tail never written.",
            100.0 * short as f64 / want as f64
        );
        println!("   One call is NOT enough. kaduceus must surface the flag — that is now our fix.");
    } else {
        println!("=> One call filled the buffer. Necessary but not sufficient:");
        println!("   a fully-black bottom edge would also look 'filled'. Check probe-A-whole.png.");
    }

    // Deliberately last. Calling into a finished C++ decompressor may abort the process, and if it
    // does we still want everything above to have been printed.
    println!("\n--- second process() call on the same decompressor (may crash; results above are safe) ---");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let Ok(mut img) = open_image(&rt, &bytes) else { return };
    let mut buf2 = vec![0u8; want];
    let Ok(mut d) = img.open_region(Region { x: 0, y: 0, width: w, height: h }, sw, sh) else {
        println!("open_region failed");
        return;
    };
    match d.process(&mut buf2) {
        Ok(r) => println!("call 1 -> Ok({r:?}), {} bytes written", filled_len(&buf2)),
        Err(e) => println!("call 1 -> Err({e})"),
    }
    let mut buf3 = vec![0u8; want];
    match d.process(&mut buf3) {
        Ok(r) => println!("call 2 -> Ok({r:?}), {} bytes written", filled_len(&buf3)),
        Err(e) => println!("call 2 -> Err({e})"),
    }
    println!("   (call 2 writing 0 bytes and returning Ok is the defect: 'done' and 'more to come'");
    println!("    are indistinguishable to the caller.)");

    println!("\ndone. images in {out_dir}/probe-*.png\n");
}
