# Third-party software shipped with TubeGlass

## FFmpeg

The release packages (AppImage, Linux tarball, Windows zip, macOS app) include a copy of
**FFmpeg**, and TubeGlass downloads the same builds on first run when no ffmpeg is installed.
TubeGlass runs it as a separate program (`ffmpeg`) to decode, download and cut media; it does not
link against it.

- FFmpeg is a project of the FFmpeg developers: <https://ffmpeg.org>
- The builds are **static, GPL version 3** builds (they include libx264 and other GPL components).
  The licence text is in `ffmpeg-LICENSE-GPLv3.txt` next to this file.
- The exact builds, with their SHA-256 checksums, are listed in `packaging/ffmpeg-sources.tsv`. They are
  redistributed unchanged from the [`eugeneware/ffmpeg-static`](https://github.com/eugeneware/ffmpeg-static)
  release `b6.1.1`, which repackages:

| Platform | Original build |
| --- | --- |
| Linux x86_64, aarch64 | John Van Sickle's static builds, FFmpeg 7.0.2 — <https://johnvansickle.com/ffmpeg/> |
| Windows x86_64 | gyan.dev "essentials" build, FFmpeg 6.1.1 — <https://www.gyan.dev/ffmpeg/builds/> |
| macOS x86_64 | evermeet.cx static build — <https://evermeet.cx/ffmpeg/> |
| macOS arm64 | static build for Apple silicon (see the `ffmpeg-static` release notes) |

- **Source code.** The complete corresponding source of FFmpeg is available from
  <https://ffmpeg.org/download.html> (and <https://github.com/FFmpeg/FFmpeg>); each build's own page
  linked above documents its exact version, configuration and the source used. If you would like the
  source for the build in a TubeGlass package and cannot find it, open an issue at
  <https://github.com/flawlesscode254/TubeGlass/issues> and it will be provided.
- To use your own ffmpeg instead, set the `TUBEGLASS_FFMPEG` environment variable to its path.

TubeGlass itself is MIT licensed (see `LICENSE`); the GPL applies to the bundled FFmpeg program only.
