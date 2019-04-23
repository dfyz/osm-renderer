An OpenStreetMap raster tile renderer that compiles to a native Windows/Linux/macOS binary with no external dependencies.

You do have to install [Rust](https://rustup.rs) to compile the binary, but other than that, all you need is an `*.xml` file with raw OSM data.

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

You can use the `@2x` suffix to request [high-resolution tiles](https://wiki.openstreetmap.org/wiki/High-resolution_tiles) (i.e. change your URL template to `http://localhost:8080/{z}/{x}/{y}{r}.png` for leaflet.js).

## Rendering sample

The rendering style is based on [MAPS.ME](https://github.com/mapsme/omim).

![London centre](/samples/london_2x.png)
