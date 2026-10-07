"""Compose preserved screenshots into labelled comparison boards, without changing source files."""

from pathlib import Path

from PIL import Image, ImageDraw


HERE = Path(__file__).resolve().parent
for route in ("home", "library", "detail"):
    web = Image.open(HERE / "web" / f"web-{route}-390-light-en.jpg").convert("RGB")
    mobile = Image.open(HERE / "screenshots" / f"discovery-390-light-en-registry-{route}.png").convert("RGB")
    # Web captures are full-page: compare only their original first viewport pixels.
    # All original screenshots remain alongside these derived boards for inspection.
    web = web.crop((0, 0, web.width, min(844, web.height)))
    gap, top = 24, 40
    board = Image.new("RGB", (web.width + mobile.width + gap * 3, max(web.height, mobile.height) + top + gap), "white")
    draw = ImageDraw.Draw(board)
    draw.text((gap, 14), f"Web / 390 px viewport, {web.width} px content / en / light", fill="black")
    draw.text((web.width + gap * 2, 14), "Shared CMP / 390 dp / en / light", fill="black")
    board.paste(web, (gap, top))
    board.paste(mobile, (web.width + gap * 2, top))
    board.save(HERE / f"comparison-{route}-390-light-en.png")
