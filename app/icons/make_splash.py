# Builds the splash from its artwork (app/icons/splash-source.jpg, 2752 x 1536):
#   app/icons/aztek-splash.png  1200 x 670, shown by the app at 600 x 335 with the version drawn on it
#                               (splash_pixmap in app/src/widgets.cpp)
#   docs/splash.png             the same, for the README and the website
#   docs/social-preview.png     1280 x 640, cropped top and bottom, for link previews
#   python3 app/icons/make_splash.py
import os
from PIL import Image

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.join(here, "..", "..")
# The artwork is black and white: one channel keeps the files small.
src = Image.open(os.path.join(here, "splash-source.jpg")).convert("L")

splash = src.resize((1200, 670), Image.LANCZOS)
splash.save(os.path.join(here, "aztek-splash.png"), optimize=True)
splash.save(os.path.join(root, "docs", "splash.png"), optimize=True)

wide = src.resize((1280, round(1280 * src.height / src.width)), Image.LANCZOS)
top = (wide.height - 640) // 2
wide.crop((0, top, 1280, top + 640)).save(os.path.join(root, "docs", "social-preview.png"), optimize=True)
