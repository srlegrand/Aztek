# Builds app/icons/aztek-splash.png: the logo on black, and a black and white photo with the mark inked on the cheek.
#   python3 app/icons/make_splash.py app/icons/splash-photo.jpg app/icons/splash-mask.png app/icons/aztek-logo.svg app/icons/aztek-splash.png
# The mask separates the subject from the background (made with rembg, isnet-general-use); any size, it is resized to the photo.
# The version text is drawn by the app (splash_pixmap in app/src/widgets.cpp).
import numpy as np, subprocess, io, sys
from PIL import Image, ImageFilter
from scipy import ndimage
W,H=1200,700
CX,CY=1052,428
photo=Image.open(sys.argv[1]).convert('L')
mask=Image.open(sys.argv[2]).convert('L').resize(photo.size, Image.LANCZOS)
logo_svg=open(sys.argv[3]).read()
out=sys.argv[4]
def svg(s,w,h):
    png=subprocess.run(['rsvg-convert','-w',str(w),'-h',str(h)],input=s.encode(),capture_output=True).stdout
    return Image.open(io.BytesIO(png)).convert('RGBA')
# keep only the dog: largest component of the mask
m=np.asarray(mask,float)/255
lab,n=ndimage.label(m>0.5); sizes=ndimage.sum(m>0.5,lab,range(1,n+1))
keep=lab==(1+int(np.argmax(sizes)))
keep=ndimage.binary_fill_holes(keep)
m=np.clip(ndimage.gaussian_filter(keep.astype(float),3),0,1)*np.maximum(m,keep*0.0+0)  # soft edge
m=ndimage.gaussian_filter(keep.astype(float),2.5)
g=np.asarray(photo,float)/255
# cheek mark: the Aztek A, inked on the left cheek
icon='''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><g fill="none" stroke="#000" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"><path d="M90 450 L210 62 L330 450"/><path d="M135 310 H422 V400 H332 V355 H377"/></g></svg>'''
# crop the face
x0,y0,x1,y1=240,30,1390,1148
g=g[y0:y1,x0:x1]; m=m[y0:y1,x0:x1]
tw,th=720,700
g=np.asarray(Image.fromarray((g*255).astype(np.uint8)).resize((tw,th),Image.LANCZOS),float)/255
m=np.asarray(Image.fromarray((m*255).astype(np.uint8)).resize((tw,th),Image.LANCZOS),float)/255
# black and white grade: lift contrast, deep shadows
g=np.clip((g-0.06)/0.8,0,1)
g=g**1.05
g=np.clip(1.15*g,0,1)
# fade into the black on the left
xx=np.arange(tw)[None,:]
fade=np.clip(xx/220,0,1)**1.5
# cheek mark in output space
ic=svg(icon,62,62).rotate(8,resample=Image.BICUBIC,expand=True)
ia=ndimage.gaussian_filter(np.asarray(ic,float)[:,:,3]/255,0.7)*0.7
cx,cy=CX-(W-tw),CY
h_,w_=ia.shape; py,px=cy-h_//2,cx-w_//2
ink=np.zeros_like(g); ink[py:py+h_,px:px+w_]=ia
g=g*(1-ink)
face=g*m*fade
canvas=np.zeros((H,W))
canvas[:,W-tw:]=face
img=Image.fromarray((canvas*255).astype(np.uint8)).convert('RGB')
lg=svg(logo_svg,460,233)
lgm=lg.split()[0]  # white strokes on black
img.paste(Image.new('RGB',lg.size,(255,255,255)),(40,50),lgm)
img.save(out)
