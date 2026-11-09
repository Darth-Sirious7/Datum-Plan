//! Copy Picture: puts a picture of the Gantt or a chart on the clipboard (Windows), as a device
//! independent bitmap that every program pastes and as PNG for the programs that prefer it.

/// `png` and `dib` are the picture as PNG and as a packed DIB (header and pixels), in base64.
#[tauri::command]
pub fn copy_picture(png: String, dib: String) -> Result<(), String> {
    use base64::Engine;
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.decode(s.split_once(',').map_or(s, |x| x.1)).map_err(|e| format!("The picture could not be read: {e}"));
    let (png, dib) = (b64(&png)?, b64(&dib)?);
    if dib.len() < 40 || dib.len() > 400_000_000 || png.len() > 200_000_000 { return Err("The picture is too large to copy.".into()) }
    put(&png, &dib)
}

#[cfg(windows)]
fn put(png: &[u8], dib: &[u8]) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_DIB;
    // A moveable block holding the bytes; the clipboard owns it once set.
    unsafe fn set(fmt: u32, data: &[u8]) -> windows::core::Result<()> {
        let h: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, data.len())?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() { return Err(windows::core::Error::from_thread()) }
        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
        let _ = GlobalUnlock(h);
        SetClipboardData(fmt, Some(HANDLE(h.0)))?;
        Ok(())
    }
    unsafe {
        OpenClipboard(None).map_err(|e| format!("The clipboard is busy: {e}"))?;
        let r = (|| -> windows::core::Result<()> {
            EmptyClipboard()?;
            set(CF_DIB.0 as u32, dib)?;
            let fmt = RegisterClipboardFormatW(w!("PNG"));
            if fmt != 0 { set(fmt, png)?; }
            Ok(())
        })();
        let _ = CloseClipboard();
        r.map_err(|e| format!("The picture could not be copied: {e}"))
    }
}

#[cfg(not(windows))]
fn put(_png: &[u8], _dib: &[u8]) -> Result<(), String> { Err("Copy Picture works on Windows.".into()) }
