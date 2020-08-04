#!/bin/bash

for s in *.svg; do
    rsvg-convert -w 64 -h 64 $s > `basename $s .svg`.png
    # rsvg-convert -w 256 -h 256 $s > `basename $s .svg`_large.png
done
