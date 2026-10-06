#!/usr/bin/env python3
"""Compare reproducible cover-color heuristics against measured Spotify CSS.
Usage: compare.py /tmp/album-color-study web/public/color-study
Requires Pillow. No album UI behavior is changed by this experiment.
"""
import colorsys
from collections import Counter
import json
import math
from pathlib import Path
import shutil
import sys
from PIL import Image


def lab(rgb):
    v=[((x/255+0.055)/1.055)**2.4 if x/255>0.04045 else x/255/12.92 for x in rgb]
    xyz=[sum(a*b for a,b in zip(row,v))/white for row,white in zip(
        ((.4124564,.3575761,.1804375),(.2126729,.7151522,.0721750),(.0193339,.1191920,.9503041)),(.95047,1,1.08883))]
    f=[x**(1/3) if x>216/24389 else (24389/27*x+16)/116 for x in xyz]
    return (116*f[1]-16,500*(f[0]-f[1]),200*(f[1]-f[2]))


def distance(a,b): return math.sqrt(sum((x-y)**2 for x,y in zip(a,b)))
def hexcolor(rgb): return '#'+''.join(f'{max(0,min(255,round(x))):02X}' for x in rgb)
def fromhex(s): return tuple(int(s[i:i+2],16) for i in (1,3,5))
def hsl(rgb): return colorsys.rgb_to_hls(*(x/255 for x in rgb))


def palette(pixels, n):
    image=Image.new('RGB',(len(pixels),1));image.putdata(pixels)
    quant=image.quantize(colors=n,method=Image.Quantize.MEDIANCUT)
    pal=quant.getpalette()
    return sorted([(count,tuple(pal[3*i:3*i+3])) for count,i in quant.getcolors()],reverse=True)


def clustering(pixels,k=8):
    points=[(n,rgb,lab(rgb)) for rgb,n in Counter(pixels).items()]
    centers=[max(points,key=lambda p:p[0])[2]]
    for _ in range(k-1):
        centers.append(max(points,key=lambda p:p[0]*min(distance(p[2],c)**2 for c in centers))[2])
    for _ in range(18):
        groups=[[] for _ in centers]
        for point in points: groups[min(range(k),key=lambda i:distance(point[2],centers[i]))].append(point)
        new=[tuple(sum(n*x[j] for n,_,x in g)/sum(n for n,_,_ in g) for j in range(3)) if g else centers[i] for i,g in enumerate(groups)]
        if max(distance(a,b) for a,b in zip(new,centers))<.05: break
        centers=new
    return [(sum(n for n,_,_ in g),tuple(round(sum(n*rgb[j] for n,rgb,_ in g)/sum(n for n,_,_ in g)) for j in range(3))) for g in groups if g]


def methods(image):
    pixels=list(image.convert('RGB').resize((64,64),Image.Resampling.LANCZOS).getdata())
    mean=tuple(sum(p[j] for p in pixels)/len(pixels) for j in range(3))
    raw=palette(pixels,8)
    filtered=[p for p in pixels if .12<hsl(p)[1]<.9]
    filtered=filtered or pixels
    dominant=palette(filtered,8)[0][1]
    groups=clustering(pixels)
    def score(item):
        count,rgb=item;_,light,sat=hsl(rgb)
        return (count/len(pixels))**.65*(.2+sat)*min(1,light/.15,(1-light)/.15)
    weighted=max(groups,key=score)[1]
    # A Vibrant-style middle-lightness target; discard tiny clusters (<1%).
    swatches=palette(pixels,24)
    vivid=[(n,rgb) for n,rgb in swatches if n>=len(pixels)*.01 and hsl(rgb)[2]>.3 and .15<hsl(rgb)[1]<.8]
    vibrant=max(vivid,key=lambda c:.5*hsl(c[1])[2]+.35*(1-abs(hsl(c[1])[1]-.5)*2)+.15*c[0]/max(n for n,_ in swatches))[1] if vivid else dominant
    return {'Average':mean,'Dominant palette':raw[0][1],'Filtered dominant':dominant,'Weighted Lab clusters':weighted,'Vibrant swatch':vibrant}


def main():
    source,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True)
    samples=json.loads((source/'samples.json').read_text());results=[]
    android_path=source/'android-palette.json'
    android=json.loads(android_path.read_text()) if android_path.exists() else None
    android_samples={r['file']:r for r in android['samples']} if android else {}
    for sample in samples:
        selected=methods(Image.open(source/sample['file']))
        fallbacks={}
        if android:
            profiles=android_samples[sample['file']]['profiles']
            for name,swatch in profiles.items():
                key='Android · '+name
                fallback=profiles['Dominant']
                selected[key]=fromhex((swatch or fallback or {'color':'#191414'})['color'])
                if swatch is None: fallbacks[key]='Dominant fallback' if fallback else 'Neutral fallback'
        target=lab(fromhex(sample['top']))
        row={k:v for k,v in sample.items() if k!='sourceColors'}
        row['methods']={name:{'color':hexcolor(rgb),'delta':round(distance(lab(rgb),target),2)} for name,rgb in selected.items()}
        for name,note in fallbacks.items(): row['methods'][name]['note']=note
        results.append(row);shutil.copy2(source/sample['file'],out/sample['file'])
        print(sample['title'],sample['top'],[(name,value['color'],value['delta']) for name,value in row['methods'].items()])
    scores={name:round(sum(r['methods'][name]['delta'] for r in results)/len(results),2) for name in results[0]['methods']}
    (out/'results.json').write_text(json.dumps({'measured_at':'2026-10-07','metric':'CIE76 Lab distance to measured Spotify gradient-top color; lower is closer. Unfitted raw extraction outputs.', 'scores':scores,'samples':results},indent=2))
    if android: shutil.copy2(android_path,out/'android-palette.json')
    print('MEAN DELTA',scores)

if __name__=='__main__': main()
