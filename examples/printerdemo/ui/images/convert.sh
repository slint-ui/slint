#!/bin/bash

for s in *.svg; do
    rsvg-convert -w 256 -h 256 $s > `basename $s .svg`.png
done
