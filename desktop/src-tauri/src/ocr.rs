//! OCR of scanned PDF pages with the system's Vision framework (macOS only).
//!
//! A scanned PDF has no text layer, so the Research Core could not index it. Here each wanted
//! page is drawn to a bitmap (CoreGraphics) and read by `VNRecognizeTextRequest`; the lines come
//! back top to bottom and are grouped into paragraphs. Everything stays on the machine. The
//! classes are found by name at run time (the frameworks are linked below), so this builds
//! against any SDK. The text is a *recognition*, not the paper's own text: the index marks those
//! pages (`locator_quality = ocr`, parse status `ocr`) and the reader says so.

/// Most pages recognised for one file in one run (a page takes a second or two).
pub const MAX_OCR_PAGES: usize = 80;

/// The text of each of `pages` (1-based; `None` = the first [`MAX_OCR_PAGES`]), as
/// `(page, paragraphs)`, plus the PDF's page count. A page with no recognised text is left out.
pub fn recognize(bytes: &[u8], pages: Option<&[u32]>) -> Result<(Vec<(u32, String)>, u32), String> {
    imp::recognize(bytes, pages)
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn recognize(_: &[u8], _: Option<&[u32]>) -> Result<(Vec<(u32, String)>, u32), String> {
        Err("OCR is only available on macOS".to_string())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::ffi::{c_void, CStr};

    type CFRef = *const c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Affine {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        tx: f64,
        ty: f64,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDataCreate(alloc: CFRef, bytes: *const u8, len: isize) -> CFRef;
        fn CFRelease(cf: CFRef);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGDataProviderCreateWithCFData(data: CFRef) -> CFRef;
        fn CGPDFDocumentCreateWithProvider(provider: CFRef) -> CFRef;
        fn CGPDFDocumentGetNumberOfPages(doc: CFRef) -> usize;
        fn CGPDFDocumentGetPage(doc: CFRef, number: usize) -> CFRef;
        fn CGPDFPageGetBoxRect(page: CFRef, kind: i32) -> NSRect;
        fn CGPDFPageGetDrawingTransform(page: CFRef, kind: i32, rect: NSRect, rotate: i32, preserve_aspect: bool) -> Affine;
        fn CGColorSpaceCreateDeviceRGB() -> CFRef;
        fn CGBitmapContextCreate(data: *mut c_void, w: usize, h: usize, bits: usize, row: usize, space: CFRef, info: u32) -> CFRef;
        fn CGContextSetRGBFillColor(ctx: CFRef, r: f64, g: f64, b: f64, a: f64);
        fn CGContextFillRect(ctx: CFRef, rect: NSRect);
        fn CGContextConcatCTM(ctx: CFRef, t: Affine);
        fn CGContextDrawPDFPage(ctx: CFRef, page: CFRef);
        fn CGBitmapContextCreateImage(ctx: CFRef) -> CFRef;
    }

    // Loads Vision so its classes can be looked up by name.
    #[link(name = "Vision", kind = "framework")]
    extern "C" {}

    /// A CF object released when dropped.
    struct Owned(CFRef);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: a +1 CF object this module created and has not released.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    /// The page drawn on white at about 2000 px on its long side (more would not read better, less loses small print).
    fn render(page: CFRef) -> Result<Owned, String> {
        // SAFETY: `page` is a live CGPDFPage of a document the caller keeps alive; every object created here is owned and released by `Owned`.
        unsafe {
            let media = CGPDFPageGetBoxRect(page, 0);
            let (w, h) = (media.size.width.max(1.0), media.size.height.max(1.0));
            let scale = (2000.0 / w.max(h)).clamp(1.0, 4.0);
            let (pw, ph) = ((w * scale).ceil() as usize, (h * scale).ceil() as usize);
            let space = Owned(CGColorSpaceCreateDeviceRGB());
            // kCGImageAlphaPremultipliedLast
            let ctx = Owned(CGBitmapContextCreate(std::ptr::null_mut(), pw, ph, 8, 0, space.0, 1));
            if ctx.0.is_null() {
                return Err("could not make a bitmap for the page".into());
            }
            CGContextSetRGBFillColor(ctx.0, 1.0, 1.0, 1.0, 1.0);
            CGContextFillRect(ctx.0, NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(pw as f64, ph as f64)));
            let fit = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(pw as f64, ph as f64));
            CGContextConcatCTM(ctx.0, CGPDFPageGetDrawingTransform(page, 0, fit, 0, true));
            CGContextDrawPDFPage(ctx.0, page);
            let image = Owned(CGBitmapContextCreateImage(ctx.0));
            if image.0.is_null() {
                return Err("could not read the page bitmap".into());
            }
            Ok(image)
        }
    }

    unsafe fn nsstring(obj: *mut AnyObject) -> String {
        if obj.is_null() {
            return String::new();
        }
        let c: *const std::ffi::c_char = msg_send![obj, UTF8String];
        if c.is_null() { String::new() } else { CStr::from_ptr(c).to_string_lossy().into_owned() }
    }

    /// The lines of one page image as paragraphs.
    fn read_image(image: CFRef) -> Result<String, String> {
        autoreleasepool(|_| {
            // SAFETY: Vision classes looked up by name and sent the messages their headers document, on a CGImage this module owns; every returned object is autoreleased inside this pool.
            unsafe {
                let (Some(req_class), Some(handler_class), Some(array_class), Some(dict_class)) = (
                    AnyClass::get(c"VNRecognizeTextRequest"),
                    AnyClass::get(c"VNImageRequestHandler"),
                    AnyClass::get(c"NSMutableArray"),
                    AnyClass::get(c"NSDictionary"),
                ) else {
                    return Err("the Vision framework is not available".to_string());
                };
                let request_alloc: *mut AnyObject = msg_send![req_class, alloc];
                let request: *mut AnyObject = msg_send![request_alloc, init];
                if request.is_null() {
                    return Err("could not make the text request".into());
                }
                // VNRequestTextRecognitionLevelAccurate
                let _: () = msg_send![request, setRecognitionLevel: 0_isize];
                let _: () = msg_send![request, setUsesLanguageCorrection: true];
                let langs: *mut AnyObject = msg_send![array_class, array];
                for lang in ["zh-Hans", "zh-Hant", "en-US"] {
                    let s = NSString::from_str(lang);
                    let _: () = msg_send![langs, addObject: &*s];
                }
                let _: () = msg_send![request, setRecognitionLanguages: langs];
                let options: *mut AnyObject = msg_send![dict_class, dictionary];
                let handler_alloc: *mut AnyObject = msg_send![handler_class, alloc];
                let handler: *mut AnyObject = msg_send![handler_alloc, initWithCGImage: image, options: options];
                if handler.is_null() {
                    let _: () = msg_send![request, release];
                    return Err("could not make the image handler".into());
                }
                let list: *mut AnyObject = msg_send![array_class, array];
                let _: () = msg_send![list, addObject: request];
                let mut error: *mut AnyObject = std::ptr::null_mut();
                let ok: bool = msg_send![handler, performRequests: list, error: &mut error];
                let result = if ok {
                    let results: *mut AnyObject = msg_send![request, results];
                    let count: usize = if results.is_null() { 0 } else { msg_send![results, count] };
                    // (top edge, bottom edge, left edge, text) in Vision's normalised, bottom-left space.
                    let mut lines: Vec<(f64, f64, f64, String)> = Vec::new();
                    for i in 0..count {
                        let obs: *mut AnyObject = msg_send![results, objectAtIndex: i];
                        let boxed: NSRect = msg_send![obs, boundingBox];
                        let top: *mut AnyObject = msg_send![obs, topCandidates: 1_usize];
                        let n: usize = if top.is_null() { 0 } else { msg_send![top, count] };
                        if n == 0 {
                            continue;
                        }
                        let first: *mut AnyObject = msg_send![top, objectAtIndex: 0_usize];
                        let text = nsstring(msg_send![first, string]);
                        if !text.trim().is_empty() {
                            lines.push((boxed.origin.y + boxed.size.height, boxed.origin.y, boxed.origin.x, text.trim().to_string()));
                        }
                    }
                    Ok(paragraphs(lines))
                } else {
                    let message = if error.is_null() { String::new() } else { nsstring(msg_send![error, localizedDescription]) };
                    Err(format!("text recognition failed: {message}"))
                };
                let _: () = msg_send![handler, release];
                let _: () = msg_send![request, release];
                result
            }
        })
    }

    /// Lines top to bottom (left to right on one row), cut into paragraphs where the gap is wider than a line.
    fn paragraphs(mut lines: Vec<(f64, f64, f64, String)>) -> String {
        lines.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)));
        let mut out = String::new();
        let mut prev_bottom: Option<f64> = None;
        for (top, bottom, _, text) in lines {
            if let Some(pb) = prev_bottom {
                let height = (top - bottom).max(0.005);
                out.push_str(if pb - top > height * 0.9 { "\n\n" } else { "\n" });
            }
            out.push_str(&text);
            prev_bottom = Some(bottom);
        }
        out
    }

    pub fn recognize(bytes: &[u8], wanted: Option<&[u32]>) -> Result<(Vec<(u32, String)>, u32), String> {
        // SAFETY: the CF data, provider and document are created here, released by `Owned` after the last page is read, and used on this thread only.
        unsafe {
            let data = Owned(CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize));
            let provider = Owned(CGDataProviderCreateWithCFData(data.0));
            let doc = Owned(CGPDFDocumentCreateWithProvider(provider.0));
            if doc.0.is_null() {
                return Err("could not open the PDF for OCR".into());
            }
            let count = CGPDFDocumentGetNumberOfPages(doc.0);
            let pages: Vec<u32> = match wanted {
                Some(list) => list.iter().copied().filter(|p| *p >= 1 && (*p as usize) <= count).take(super::MAX_OCR_PAGES).collect(),
                None => (1..=u32::try_from(count.min(super::MAX_OCR_PAGES)).unwrap_or(0)).collect(),
            };
            let mut out = Vec::new();
            for page_no in pages {
                let page = CGPDFDocumentGetPage(doc.0, page_no as usize);
                if page.is_null() {
                    continue;
                }
                let image = render(page)?;
                let text = read_image(image.0)?;
                if !text.trim().is_empty() {
                    out.push((page_no, text));
                }
            }
            Ok((out, u32::try_from(count).unwrap_or(u32::MAX)))
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) mod fixture {
    /// A page of text drawn into an image-only PDF with the system's tools (no text layer).
    pub fn scanned_pdf(dir: &std::path::Path) -> Option<Vec<u8>> {
        let run = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success());
        let txt = dir.join("p.txt");
        std::fs::write(&txt, "Attention is all you need.\n\nResults: accuracy 91.2 on the test set.\n").ok()?;
        let text_pdf = dir.join("t.pdf");
        std::fs::write(&text_pdf, run("/usr/sbin/cupsfilter", &[txt.to_str()?])?.stdout).ok()?;
        let png = dir.join("p.png");
        run("/usr/bin/sips", &["-s", "format", "png", text_pdf.to_str()?, "--out", png.to_str()?])?;
        // A real scan is about 300 dpi: enlarge the 72 dpi raster the system gives.
        run("/usr/bin/sips", &["--resampleWidth", "2400", png.to_str()?])?;
        let scan = dir.join("scan.pdf");
        run("/usr/bin/sips", &["-s", "format", "pdf", png.to_str()?, "--out", scan.to_str()?])?;
        std::fs::read(scan).ok()
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::fixture::scanned_pdf;
    use super::*;

    #[test]
    fn a_scanned_page_is_read_by_vision() {
        let dir = std::env::temp_dir().join(format!("npi-ocr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let Some(pdf) = scanned_pdf(&dir) else {
            eprintln!("skipped: cupsfilter/sips could not make a scanned PDF here");
            return;
        };
        let (pages, count) = recognize(&pdf, None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, 1);
        let text = pages.iter().map(|(_, t)| t.as_str()).collect::<String>().to_lowercase();
        assert!(text.contains("attention"), "recognised: {text:?}");
        assert!(text.contains("91.2"), "recognised: {text:?}");
    }

    #[test]
    fn a_broken_pdf_is_an_error_not_a_crash() {
        assert!(recognize(b"not a pdf", None).is_err());
    }
}
