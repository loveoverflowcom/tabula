"""Bounded read-only sampling of public test-canvas pixels after capture."""
from PIL import Image


def public_samples(png_path, region, foreground, background):
    """Select a few observed glyph/background pixels, never transform an asset."""
    with Image.open(png_path) as source:
        image = source.convert("RGB")
        x0, y0, x1, y1 = region
        rows = [(x, y, image.getpixel((x, y)))
                for y in range(y0, min(y1, image.height))
                for x in range(x0, min(x1, image.width))]
    selected, used = [], set()
    white = [p for p in rows if p[2] == (255, 255, 255)]
    midpoint = [(foreground[i] + background[i]) / 2 for i in range(3)]
    antialias = sorted(rows, key=lambda p: sum((p[2][i] - midpoint[i]) ** 2 for i in range(3)))
    categories = [
        ("white_edge_candidate" if white else "intermediate_edge_candidate", white or antialias),
        ("dark_ink", sorted(rows, key=lambda p: sum((p[2][i] - foreground[i]) ** 2 for i in range(3)))),
        ("background", [p for p in rows if p[2] == background]),
    ]
    for category, candidates in categories:
        count = 0
        for x, y, rgb in candidates:
            if (x, y) in used:
                continue
            used.add((x, y))
            selected.append({"category": category, "x": x, "y": y, "png_rgb": list(rgb)})
            count += 1
            if count == 5:
                break
    return selected


def probe(page, samples):
    """Existing context only; no settings, framebuffer binds or game state writes."""
    return page.evaluate("""async samples => new Promise(resolve => {
      requestAnimationFrame(() => {
        const canvas = document.querySelector('#glcanvas');
        const context = window.gl;
        if (!canvas || !context || typeof context.readPixels !== 'function') {
          resolve({status:'INCONCLUSIVE',reason:'existing WebGL context not publicly available'}); return;
        }
        try {
          const rect = canvas.getBoundingClientRect();
          const ratioX = context.drawingBufferWidth / rect.width;
          const ratioY = context.drawingBufferHeight / rect.height;
          const values = samples.map(sample => {
            const rgba = new Uint8Array(4);
            const x = Math.floor(sample.x * ratioX);
            const y = context.drawingBufferHeight - 1 - Math.floor(sample.y * ratioY);
            context.readPixels(x,y,1,1,context.RGBA,context.UNSIGNED_BYTE,rgba);
            return {...sample,raw_framebuffer_rgba:Array.from(rgba)};
          });
          resolve({status:values.some(x=>x.raw_framebuffer_rgba.some(v=>v!==0))?'OBSERVED':'INCONCLUSIVE',
            context_attributes:context.getContextAttributes(),
            drawing_buffer:{width:context.drawingBufferWidth,height:context.drawingBufferHeight},
            current_blend:{enabled:context.isEnabled(context.BLEND),src_rgb:context.getParameter(context.BLEND_SRC_RGB),
              dst_rgb:context.getParameter(context.BLEND_DST_RGB),src_alpha:context.getParameter(context.BLEND_SRC_ALPHA),
              dst_alpha:context.getParameter(context.BLEND_DST_ALPHA)},
            samples:values,scope:'Read-only public static local-fixture pixels; PNG and framebuffer sample are different observations. Current blend state is not a proven per-glyph pipeline. Cleared buffers or compositing differences are inconclusive'});
        } catch (_) { resolve({status:'INCONCLUSIVE',reason:'existing-context readback unsupported'}); }
      });
    })""", samples)
