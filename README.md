A work-in-progress OpenStreetMap raster tile renderer that compiles to a native Windows/Linux/macOS binary with no external dependencies.

All you need is an `*.xml` file with raw OSM data.

Currently, you also need to install [Rust](https://rustup.rs), since no pre-compiled binaries are available.

## Importing data

This command takes a `city.xml` data file (get one from [Geofabrik](https://download.geofabrik.de) or a simliar service) and outputs `city.bin`, which will be used for rendering.

```
$ cargo run --release --bin importer city.xml city.bin
```

## Rendering data

```
$ cat city.conf
[http]
address = localhost:8080

[geodata]
file = city.bin

[style]
file = mapcss/osmosnimki-minimal.mapcss
type = josm

$ cargo run --release --bin renderer city.conf
```

Raster tiles are now being served from `http://localhost:8080/{z}/{x}/{y}.png`. This URL template should work out of the box with leaflet.js, MKTileOverlay, or any map library that supports [slippy tile layers](https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames). 

## Rendering sample

The rendering style is based on [osmosnimki](https://github.com/kothic/kothic-js-mapcss/blob/master/styles/osmosnimki-maps.mapcss).

![London centre](/samples/london.png)
