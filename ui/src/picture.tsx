// Copy Picture: a canvas onto the clipboard (through the shell, Windows), as PNG and as a bitmap
// (DIB) on white, so it pastes into documents, slides and workbooks.
import { call, toast } from "./store";

const b64 = (u8: Uint8Array) => { let s = ""; for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000)); return btoa(s); };

/** The picture on white, as a packed DIB: a BITMAPINFOHEADER and 32-bit pixels, bottom row first. */
export const dibOf = (cv: HTMLCanvasElement) => {
  const w = cv.width, h = cv.height, px = cv.getContext("2d")!.getImageData(0, 0, w, h).data;
  const out = new Uint8Array(40 + w * h * 4), v = new DataView(out.buffer);
  v.setUint32(0, 40, true); v.setInt32(4, w, true); v.setInt32(8, h, true); v.setUint16(12, 1, true); v.setUint16(14, 32, true);
  v.setUint32(16, 0, true); v.setUint32(20, w * h * 4, true); v.setInt32(24, 3780, true); v.setInt32(28, 3780, true);
  for (let y = 0; y < h; y++) {
    const src = y * w * 4, dst = 40 + (h - 1 - y) * w * 4;
    for (let x = 0; x < w * 4; x += 4) { out[dst + x] = px[src + x + 2]; out[dst + x + 1] = px[src + x + 1]; out[dst + x + 2] = px[src + x]; out[dst + x + 3] = 255; }
  }
  return out;
};

/** Copies a canvas (drawn on white first, so transparent parts paste white). */
export const copyCanvas = async (src: HTMLCanvasElement, what = "Picture") => {
  const cv = document.createElement("canvas");
  cv.width = src.width; cv.height = src.height;
  const ctx = cv.getContext("2d")!;
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, cv.width, cv.height); ctx.drawImage(src, 0, 0);
  const ok = await call("copy_picture", { png: cv.toDataURL("image/png"), dib: b64(dibOf(cv)) });
  if (ok !== undefined) toast(`${what} copied: paste it into a document, slides or a workbook.`, true);
};
