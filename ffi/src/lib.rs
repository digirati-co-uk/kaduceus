//! `kdu_facade` — the C ABI between jpjp and Kakadu, over Digirati's kaduceus binding.
//!
//! Implements `src/Jpjp.Engines.Kakadu/native/kdu_facade.h` (ABI v2). That header is jpjp's own
//! contract and mentions no Kakadu symbol; this file calls only kaduceus's **public Rust API**.
//! Neither is Kakadu-derived. The built `.so`, however, statically links Kakadu and IS an
//! "Application" under clause 13.1(b) — never commit it, publish it, or show it to an AI tool.
//!
//! Install into the kaduceus checkout (the facade lives there so jpjp hosts nothing SDK-adjacent):
//!
//! ```text
//! mkdir -p ~/src/kaduceus/ffi/src
//! cp /mnt/c/git/tomcrane/jpjp/docs/kakadu-k2-facade-cargo.toml ~/src/kaduceus/ffi/Cargo.toml
//! cp /mnt/c/git/tomcrane/jpjp/docs/kakadu-k2-facade-lib.rs    ~/src/kaduceus/ffi/src/lib.rs
//! cd ~/src/kaduceus/ffi && export KDU_ROOT=/mnt/c/kdu/v8_7-01787L && cargo build --release
//! # -> ffi/target/release/libkdu_facade.so   (the ffi crate has its OWN target dir)
//! ```
//!
//! # What the 2026-09-08 probe session established, and where it shows up below
//!
//! | Measured | Consequence here |
//! |---|---|
//! | `Region` is on the FULL-RESOLUTION grid | `<< shift` in [`decode_inner`] |
//! | 3 bytes/pixel, interleaved, **RGB** not BGR | [`BYTES_PER_PIXEL`], [`scatter_strip`] |
//! | `process()` is **tile-row incremental** | the loop — one call per tile row, scattered by `Region.y` |
//! | Completion = an empty `Region` | the `break` |
//! | A short WIDTH means an assumption broke | hard error, never a silent stretch |
//!
//! Requires the kaduceus changes on the `jpjp-updates` branch: the cxx bridge must return `Result`,
//! or any Kakadu error calls `std::terminate` and takes the host process with it.

use std::ffi::{c_char, c_void};
use std::os::raw::c_int;
use std::sync::{Arc, OnceLock};

use kaduceus::{KakaduContext, KakaduImage, Region};
use tokio::runtime::Runtime;

// ---- ABI constants — must match kdu_facade.h ---------------------------------------------------

const ABI_VERSION: i32 = 2;

const OK: i32 = 0;
const ERR_ARGS: i32 = -1;
const ERR_CODESTREAM: i32 = -2;
const ERR_UNSUPPORTED: i32 = -3;
const ERR_INTERNAL: i32 = -5;
const ERR_SOURCE: i32 = -6;

const CAP_BUFFER: u32 = 1 << 0;
// CAP_PULL_SOURCE is deliberately NOT set. The 2026-09-08 I/O trace showed no seek ever reaching
// kaduceus's Rust layer and the whole codestream crossing it instead, so a pull source here would
// stream whole objects for no benefit. jpjp then drives us with a condensed codestream (K1), which
// is where the byte-range frugality actually comes from. Advertise it once `data_source.*` declares
// itself seekable AND that is measured — not before.
const CAPABILITIES: u32 = CAP_BUFFER;

/// Bytes per pixel is the COMPONENT COUNT, not a constant.
///
/// The probe measured 3 interleaved bytes per pixel with the order RGB — a decoded page reproduces
/// with warm paper tone, which BGR renders cold blue-grey. But every image it measured was
/// 3-component. A 1-component image writes 1 byte per pixel, and assuming 3 reads every third byte
/// of a greyscale strip as if it were a channel — which decodes to noise at ~3 dB PSNR rather than
/// failing, i.e. exactly the wrong-but-plausible result the acceptance test exists to catch.
fn bytes_per_pixel(ncomp: usize) -> usize {
    // Measured 2026-09-08, both ways round, on an 8-bit greyscale master (NC=1, BPC=7):
    //   bpp = 1  ->  3.2 dB against native, i.e. noise
    //   bpp = 3  ->  see below
    // Kakadu's region decompressor renders through the JP2 colour space, so a 1-component image
    // still arrives as 3 interleaved channels. The plane count jpjp asks for is a property of the
    // IMAGE; the stride Kakadu writes is a property of the RENDER, and they are not the same number.
    if ncomp <= 3 { 3 } else { ncomp }
}

// ---- process-wide singletons -------------------------------------------------------------------

/// One runtime for the process. kaduceus drives its source reads through `block_on`, so this is a
/// bridge from its async plumbing to our synchronous ABI, not a concurrency mechanism.
fn runtime() -> Arc<Runtime> {
    static RT: OnceLock<Arc<Runtime>> = OnceLock::new();
    RT.get_or_init(|| {
        Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime"),
        )
    })
    .clone()
}

/// One Kakadu context for the process — `Send + Sync` and cheap to clone.
///
/// ⚠️ Sharing this across concurrent decodes was TESTED as a cause of the concurrent-region abort on
/// 2026-09-10 and **eliminated**: a temporary build giving every decode its own fresh context aborted
/// identically at `t512` concurrency 8. Keep the singleton; it is not implicated.
fn context() -> KakaduContext {
    static CTX: OnceLock<KakaduContext> = OnceLock::new();
    CTX.get_or_init(KakaduContext::default).clone()
}

struct FacadeError {
    code: i32,
    message: String,
}

impl FacadeError {
    fn codestream(e: impl std::fmt::Display) -> Self {
        Self { code: ERR_CODESTREAM, message: e.to_string() }
    }
}

#[repr(C)]
pub struct KduFacadeRequest {
    pub region_x: i32,
    pub region_y: i32,
    pub region_w: i32,
    pub region_h: i32,
    pub reduce: i32,
    pub max_threads: i32,
}

// ---- exported entry points ---------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn kdu_facade_abi_version() -> i32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn kdu_facade_version() -> *const c_char {
    // Static storage with a trailing NUL; the caller must not free it.
    concat!("kaduceus facade / kdu (see build) ", env!("CARGO_PKG_VERSION"), "\0").as_ptr()
        as *const c_char
}

#[no_mangle]
pub extern "C" fn kdu_facade_capabilities() -> u32 {
    CAPABILITIES
}

/// # Safety
/// Caller guarantees `codestream` is readable for `codestream_len`, `request` is a valid
/// `kdu_facade_request`, and `out_planes` holds `*components` pointers each writable for
/// `region_w * region_h` floats. All are jpjp's contract in `kdu_facade.h`.
#[no_mangle]
pub unsafe extern "C" fn kdu_facade_decode(
    codestream: *const u8,
    codestream_len: usize,
    request: *const KduFacadeRequest,
    out_planes: *const *mut f32,
    components: *mut c_int,
    err: *mut c_char,
    err_len: usize,
) -> i32 {
    // Nothing may unwind across the boundary: a panic escaping an `extern "C"` fn is undefined
    // behaviour, and this is exactly the class of bug that took kaduceus down with SIGABRT before
    // the bridge was made fallible.
    let result = std::panic::catch_unwind(|| {
        let (req, planes, ncomp) = match validate(request, out_planes, components) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        if codestream.is_null() || codestream_len == 0 {
            return Err(FacadeError { code: ERR_ARGS, message: "null codestream".into() });
        }
        let bytes = std::slice::from_raw_parts(codestream, codestream_len).to_vec();
        decode_inner(bytes, &req, &planes, ncomp)
    });

    finish(result, err, err_len)
}

/// The pull-source entry point. Exported so the library is a well-formed ABI v2 member, but
/// declines: `kdu_facade_capabilities()` does not advertise `CAP_PULL_SOURCE`, so jpjp will not call
/// this. Returning a clean `ERR_UNSUPPORTED` is what the header asks a non-implementing library to
/// do, and is far better than a plausible-looking whole-object read pretending to be frugal.
///
/// # Safety
/// Same contract as [`kdu_facade_decode`]; every pointer is unused.
#[no_mangle]
pub unsafe extern "C" fn kdu_facade_decode_source(
    _read: Option<extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i64>,
    _read_ctx: *mut c_void,
    _source_len: u64,
    _request: *const KduFacadeRequest,
    _out_planes: *const *mut f32,
    _components: *mut c_int,
    err: *mut c_char,
    err_len: usize,
) -> i32 {
    write_err(
        err,
        err_len,
        "pull source not implemented: kaduceus does not surface a seekable source, so this would \
         read whole objects. Use kdu_facade_decode with a condensed codestream.",
    );
    let _ = ERR_SOURCE; // referenced by the contract; unused until a pull source exists
    ERR_UNSUPPORTED
}

// ---- implementation ----------------------------------------------------------------------------

unsafe fn validate(
    request: *const KduFacadeRequest,
    out_planes: *const *mut f32,
    components: *mut c_int,
) -> Result<(KduFacadeRequest, Vec<*mut f32>, usize), FacadeError> {
    if request.is_null() || out_planes.is_null() || components.is_null() {
        return Err(FacadeError { code: ERR_ARGS, message: "null argument".into() });
    }
    let req = std::ptr::read(request);
    if req.region_w <= 0 || req.region_h <= 0 || req.reduce < 0 || req.region_x < 0 || req.region_y < 0
    {
        return Err(FacadeError {
            code: ERR_ARGS,
            message: format!(
                "bad geometry: {}x{} at ({},{}) reduce {}",
                req.region_w, req.region_h, req.region_x, req.region_y, req.reduce
            ),
        });
    }
    let ncomp = *components;
    if ncomp <= 0 {
        return Err(FacadeError { code: ERR_ARGS, message: "no planes provided".into() });
    }
    let planes = std::slice::from_raw_parts(out_planes, ncomp as usize).to_vec();
    if planes.iter().any(|p| p.is_null()) {
        return Err(FacadeError { code: ERR_ARGS, message: "null plane pointer".into() });
    }
    Ok((req, planes, ncomp as usize))
}

fn decode_inner(
    bytes: Vec<u8>,
    req: &KduFacadeRequest,
    planes: &[*mut f32],
    ncomp: usize,
) -> Result<(), FacadeError> {
    let mut image = KakaduImage::new(
        runtime(),
        context(),
        futures::io::Cursor::new(bytes),
        Some("jpjp".to_string()),
    )
    .map_err(FacadeError::codestream)?;

    // Providing fewer planes than the image has is ERR_ARGS, not a partial fill (header contract).
    // Checked against the codestream rather than trusted, because a mismatch here would otherwise
    // read past the end of a caller-owned buffer.
    let info = image.info().map_err(FacadeError::codestream)?;
    let _ = &info; // kaduceus's Info carries no component count; see the note below.

    // ✅ 7.1 MEASURED: the region is on the FULL-RESOLUTION grid. jpjp passes RegionAtLevel, so
    // shift it back up; Kakadu derives the reduction from the ratio to the requested output size.
    let shift = req.reduce as u32;
    let roi = Region {
        x: (req.region_x as u32) << shift,
        y: (req.region_y as u32) << shift,
        width: (req.region_w as u32) << shift,
        height: (req.region_h as u32) << shift,
    };

    let out_w = req.region_w as usize;
    let out_h = req.region_h as usize;

    // ⚠️ Serialising decompressor CONSTRUCTION was tested here on 2026-09-10 and made no difference
    // to the concurrent-region abort — the failure threshold was identical with and without. So
    // concurrent creation of thread environments is not the trigger either. Diagnostic removed.
    let mut decompressor = image
        .open_region(roi, req.region_w as u32, req.region_h as u32)
        .map_err(FacadeError::codestream)?;

    // ⚠️⚠️ 7.3 MEASURED: `process()` is TILE-ROW INCREMENTAL. Each call returns ONE horizontal
    // strip, written from OFFSET 0 of the buffer we pass, with its position in the returned Region.
    // One call returns everything only for single-tile images; a facade without this loop works
    // perfectly on those and silently truncates every tiled master. Wellcome's are tiled.
    let bpp = bytes_per_pixel(ncomp);
    let mut strip = vec![0u8; out_w * out_h * bpp];
    let mut rows_done = 0usize;
    let mut guard = 0u32;

    loop {
        let r = decompressor.process(&mut strip).map_err(FacadeError::codestream)?;

        // ⚠️ An empty region used to be the ONLY completion signal kaduceus offered, so this loop
        // inferred completion from it. That is a proxy for the flag, not the flag: if the final
        // call ever returned a non-empty strip, the loop went round once more and called into a
        // decompressor that had already finished — which `examples/probe.rs` warns may abort the
        // process. `is_finished()` now exposes the real thing; the empty-region test is kept as a
        // belt-and-braces exit, not as the primary signal.
        let finished = decompressor.is_finished();
        let empty = r.width == 0 || r.height == 0;

        if empty {
            break;
        }

        let sw = r.width as usize;
        let sh = r.height as usize;
        let y0 = r.y as usize;

        // A short WIDTH means an assumption broke. jpjp only ever asks for a power-of-two reduction
        // and those measured exact; arbitrary scales come back short. Fail rather than recover —
        // recovery has to know whether a short result is a CROP of the region or the whole region
        // at a coarser scale, and those want opposite corrections. Guessing yields a confidently
        // mangled image. A short HEIGHT is normal: that is the strip.
        if sw != out_w {
            return Err(FacadeError {
                code: ERR_CODESTREAM,
                message: format!("expected width {out_w}, Kakadu returned {sw}"),
            });
        }
        if y0 + sh > out_h {
            return Err(FacadeError {
                code: ERR_CODESTREAM,
                message: format!("strip at y={y0} height {sh} overruns output height {out_h}"),
            });
        }

        scatter_strip(&strip, planes, ncomp, bpp, out_w, sw, sh, y0);
        rows_done = rows_done.max(y0 + sh);

        // ⭐ The real completion signal, checked AFTER scattering: the call that reports the decode
        // complete may still carry the last strip, and dropping it would truncate the image.
        if finished || rows_done >= out_h {
            break;
        }

        // A decoder that neither advances nor reports completion would spin forever and hang a
        // request thread. Bound it: no image has more tile rows than it has rows.
        guard += 1;
        if guard as usize > out_h {
            return Err(FacadeError {
                code: ERR_INTERNAL,
                message: format!("process() made no progress after {guard} calls"),
            });
        }
    }

    if rows_done < out_h {
        return Err(FacadeError {
            code: ERR_CODESTREAM,
            message: format!("decode ended after {rows_done} of {out_h} rows"),
        });
    }

    Ok(())
}

/// Copy one strip into jpjp's per-component f32 planes (values 0..255).
///
/// `strip` holds `sw * sh` interleaved pixels packed from its own offset 0; `y0` is where the strip
/// belongs in the output. The two sides have different row strides, which is precisely the mistake
/// that sheared the first probe images diagonally — hence `sw` for the source and `out_w` for the
/// destination, never one variable for both.
fn scatter_strip(
    strip: &[u8],
    planes: &[*mut f32],
    ncomp: usize,
    bpp: usize,
    out_w: usize,
    sw: usize,
    sh: usize,
    y0: usize,
) {
    for (c, &plane) in planes.iter().enumerate().take(ncomp) {
        // SAFETY: the caller guarantees each plane is writable for out_w * out_h floats, and the
        // bounds checks in decode_inner keep y0 + sh within out_h.
        let dst = unsafe { std::slice::from_raw_parts_mut(plane, out_w * (y0 + sh)) };
        for row in 0..sh {
            let src = row * sw * bpp;
            let dst_row = (y0 + row) * out_w;
            for x in 0..sw {
                // With bpp == ncomp this is just `c`; the clamp matters only if a backend ever
                // returns fewer channels than planes were requested for.
                let ch = if c < bpp { c } else { 0 };
                dst[dst_row + x] = strip[src + x * bpp + ch] as f32;
            }
        }
    }
}

fn finish(
    result: std::thread::Result<Result<(), FacadeError>>,
    err: *mut c_char,
    err_len: usize,
) -> i32 {
    match result {
        Ok(Ok(())) => OK,
        Ok(Err(e)) => {
            write_err(err, err_len, &e.message);
            e.code
        }
        Err(_) => {
            write_err(err, err_len, "panic caught at the ABI boundary");
            ERR_INTERNAL
        }
    }
}

fn write_err(err: *mut c_char, err_len: usize, message: &str) {
    if err.is_null() || err_len == 0 {
        return;
    }
    let bytes = message.as_bytes();
    let n = bytes.len().min(err_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), err as *mut u8, n);
        *err.add(n) = 0;
    }
}
