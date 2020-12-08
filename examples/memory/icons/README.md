The icons originate from Font-Awesome font ( http://fontawesome.io ) and licensed under the OFL

    https://fontawesome.com/license/free

The PNGs were generated using

```sh
for f in *.svg; do rsvg-convert -a -h 128 -o `echo $f | sed -e "s,-solid\.svg,.png,"` $f; done
```
