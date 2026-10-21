# File format parity

> **Last reviewed:** 2026-10-12 · **Last updated:** 2026-10-12 · **Change:** minor (closed issue references re-audited against `upstream/main` 470ea7a3) · **Target:** Adobe After Effects 2026

Every format After Effects reads or writes, with Unwritten Studio's support. After Effects' list comes
from its installed bundle (26.5.0.89: `Info.plist` `CFBundleDocumentTypes`, `Plug-ins/Format`,
`Resources/usd_plugins`) and its public documentation; ours from `crates/media/src/lib.rs`
(`STILL_EXTENSIONS`, `VIDEO_EXTENSIONS`, `MODEL_EXTENSIONS`, FilmCraft's `AUDIO_EXTENSIONS`),
`crates/engine/src/commands/file.rs` (`DATA_EXTENSIONS`) and `OutputFormat::ALL` in
`crates/project/src/render_queue.rs`. Video codecs come from FilmCraft's pure-Rust crates
([architecture.md](architecture.md)).

**Overall ≈ 50%** (estimated, weighted by use): modern delivery and interchange formats are
strong, but After Effects' own project, template and preset formats, which decide whether
existing work moves over, are 0%.

Fidelity key: **exact** (bit-exact or within a stated tolerance of a reference), **good** (round
trips and renders, checked by tests, not against After Effects), **partial** (named limitations),
**none**.

## After Effects' own formats

| Format | After Effects | Unwritten Studio read | Write | Fidelity / notes | Tested by |
|---|---|---|---|---|---|
| Project `.aep` | R/W (native) | none (direct) | none (direct) | No native binary reader or writer. Experimental desktop bridge: AE exports `.unwae` for import here; generated JSX reconstructs supported layers in AE, which can save a new `.aep`. See the bridge workflow below; not a full-fidelity round trip. Blocks native-format parity | Synthetic bridge and format-guard tests; not verified in real AE |
| XML project `.aepx` | R/W | none (direct) | none (direct) | XML with embedded hexadecimal binary data, not the native XML project format (`.uwprojx`). AE desktop can open it for bridge conversion; no native reader or writer | Synthetic helper tests; not verified in real AE |
| Templates `.aet` / `.aetx` | R/W | none | none | Own `.uwtemplate` (Home ▸ Templates, Save as Template) | `templates.rs` tests |
| Motion Graphics template `.mogrt` | W (and R in Premiere) | none | none | Own `.uwtemplate` Essential Graphics templates; FilmCraft rejects our `.fcgt` export (#611) | `essential.rs` tests |
| Animation preset `.ffx` | R/W, 621 bundled | none | none | Own JSON `.ecpreset`; `.ffx` is out of clean-room scope (AGENTS.md §2) | `animation.rs` tests |
| Render Settings / Output Module templates `.ars` / `.aom` | R/W | none | none | Templates are stored in the project and settings instead | `render_templates.rs` tests |
| Scripts `.jsx` | R (run) | yes | — | After Effects object model and ScriptUI ([scripting-parity.md](scripting-parity.md)) | `crates/script` tests |
| Binary scripts `.jsxbin` | R (run) | none | — | No public specification: out of scope | — |
| Motion graphics JSON `.mgjson` | R (data) | none | none | Media Browser lists it; import accepts only `.json` / `.csv` / `.tsv` data | — |
| Captions `.aecap`, graphics `.aegraphic` | R/W | none | none | — | — |
| Native `.unw` (preferred; `.uwproj` / `.ecproj` aliases) | — | yes | yes | Versioned canonical JSON; schema migrations and `.unw` round-trip tested. `.uwprojx` / `.ecprojx` remain native XML copies | engine project tests |

## Footage: stills

| Format | After Effects | Read | Write | Fidelity / notes |
|---|---|---|---|---|
| PNG | R/W | yes | yes (sequence) | good |
| JPEG | R/W | yes | yes (sequence) | good |
| TIFF | R/W | yes | yes (sequence) | good |
| OpenEXR (multi-layer, Cryptomatte) | R/W | yes | yes (32-bit sequence) | good; HTJ2K compression fails (#482); EXRs without an unnamed RGB layer decode twice (#474) |
| Photoshop PSD / PSB | R (footage, composition) / W (layers, sequence) | yes (layers, groups, masks, styles, smart objects) | Save Frame As ▸ Photoshop Layers; no Photoshop sequence output | good, from Adobe's published spec |
| Illustrator `.ai`, PDF, EPS | R | yes (vector, Continuously Rasterize, Create Shapes) | — | partial: no JPX / JBIG2 images, no Illustrator procset EPS |
| SVG | R | yes | — | good; SVG markup on the clipboard pastes as a shape layer (26.3, #738); Illustrator's own clipboard data (PDF/AI) doesn't |
| GIF | R/W | yes | yes (animated) | good |
| BMP | R | yes | — | good |
| WebP | — | yes (still; animated WebP as footage with its frames, timing and alpha, #762) | — | beyond After Effects |
| Targa `.tga` | R/W | yes (still and sequence; alpha interpreted on import) | none | good for import (#745); no Targa output |
| DPX / Cineon `.dpx` `.cin` | R/W | yes (still and sequence; 8/10/12/16-bit luma, RGB, RGBA, both byte orders; log code values shown as stored) | none | partial: import (#756); no Interpret Footage ▸ Cineon Settings (log to linear), no DPX output; common in film VFX |
| Radiance `.hdr` | R/W | yes (still and sequence, 32-bit linear float like OpenEXR) | none | good for import (#745); no Radiance output |
| SGI, PICT, ElectricImage, RLA/RPF images | R | none | none | Legacy; RPF camera data import exists (`keys.rpfCameraImport`) |
| Camera Raw (DNG, CR3, NEF…) | R (Camera Raw) | none | none | LightCraft has decoders that could be shared |
| HEIF / AVIF | R (platform) | none | none | — |

## Footage: video and audio

| Format | After Effects | Read | Write | Fidelity / notes |
|---|---|---|---|---|
| MP4 / MOV with H.264 | R/W | yes | yes (own encoder, + AAC) | decoder bit-exact on conformance streams (FilmCraft) |
| HEVC (H.265) | R/W | yes | yes (MP4 `hvc1`, Main / Main 10) | encoder bit-exact with ffmpeg's decode; no B-frames / SAO, larger files |
| AV1 | R | yes | yes (MP4 `av01`, WebM) | beyond After Effects' export; no B-frames / CDEF |
| Apple ProRes (422 family, 4444) | R/W | yes | yes | good; 4444 colour tag wrong (#340); no ProRes RAW |
| WebM / Matroska (VP9, AV1, Opus) | R (limited) | yes, incl. VP9 alpha | yes | beyond After Effects |
| MXF (OP1a / OP-Atom) | R/W | yes (FilmCraft's demuxer: AVC, MPEG-2 incl. XDCAM / D-10, VC-3 / DNxHD, ProRes; PCM audio) | none | partial: import (#767), checked against ffmpeg; no MXF output |
| AVI | R | none | none | Media Browser lists it |
| DNxHD / DNxHR | R/W | unverified (FilmCraft decodes it; reachable only inside `.mov`) | none | — |
| BRAW, R3D, ARRIRAW | R (plug-ins / SDKs) | none | none | proprietary SDKs |
| Variable-frame-rate phone video | R | unverified | — | needs a real-media corpus |
| WAV, AIFF | R/W | yes | yes (16-bit PCM) | good |
| MP3, AAC / M4A, FLAC, Ogg, Opus | R | yes | AAC and Opus inside video only | MP3 silence (#324) is closed |

## 3D models

| Format | After Effects 26 | Read | Notes |
|---|---|---|---|
| glTF / GLB | R | yes (meshes, PBR, skins, animations) | good |
| OBJ / MTL | R | yes | good |
| FBX | R (`usdFbx`) | yes (7.x binary and ASCII: meshes with normals, UVs and per-polygon materials, the Model hierarchy with pivots and pre/post rotation, Lambert/Phong materials with embedded or relative textures, axes turned to Y-up; no animation, skinning, cameras or lights; 6.x rejected) | partial |
| USD / USDZ | R (`usd_plugins`) | none | gap 13 |
| STL | R (`usdStl`) | yes (binary and ASCII, VisCAM and Magics facet colours; Z-up turned to Y-up) | good |
| Substance 3D materials `.sbsar` | R (26.0) | none | gap 13 |

## Interchange and data

| Format | After Effects | Read | Write | Notes |
|---|---|---|---|---|
| Premiere Pro `.prproj` | R (import) | none | none | Out of clean-room scope pending a decision; we interchange through FCP7 XML, FCPXML, OTIO, EDL, AAF and OMF instead (beyond After Effects) |
| Lottie JSON / dotLottie | via third-party plug-ins | yes | yes | beyond After Effects; a warnings list names what Lottie can't express |
| JSON / CSV / TSV data | R | yes (`footage()`, data layers) | — | good |
| LUTs (`.cube`, `.3dl`, `.csp`, `.spi1d/3d`, CDL) | R | yes (Apply Color LUT, OCIO effects) | — | Apply Color LUT bug (#514) is closed |
| OpenColorIO configs | R (OCIO 2.5.1, ACES 2.0 in 26.5) | yes (OCIO v2 subset, Blender configs) | — | partial: no ACES 2.0 output transforms |
| Fonts (TTF, OTF, TTC, variable) | R | yes | — | good |
| Mocha shapes | R (Mocha AE) | own documented JSON | — | Mocha's format is not public |
| Vanishing Point `.vpe` | R | none | — | no public specification; the menu entry explains why |

## Render queue outputs (`OutputFormat::ALL`, 13)

H.264 MP4, HEVC MP4, AV1 MP4, ProRes MOV, uncompressed 8-bit RGB / RGB + Alpha MOV (`raw `, + PCM;
#372), WebM (VP9 or AV1 + Opus), PNG / JPEG / TIFF / OpenEXR sequences, animated GIF, WAV, AIFF.
Missing against After Effects + Media Encoder: 16-bpc uncompressed, AVI and lossless-compressed
single-file video (Animation / PNG codecs), DPX / Targa / Photoshop sequences, MXF, DNxHR, hardware
encoders ([hardware-parity.md](hardware-parity.md)), and rendering safely over an existing file
(#325).

## Estimate

**≈ 130–230 h**: `.aep` / `.aepx` import 80–160 h (after the owner decision), wiring TGA / DPX /
HDR / MXF / AVI / DNx import and DPX/TGA output 10–20 h, FBX animation and skinning, USD and Substance 20–40 h,
relinking 6–12 h, `.mogrt`-compatible templates 15–30 h (partly parallel with the rest).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-12 | minor | #324 and #514 marked closed; open `.aep` PRs #251 and #860 noted |
| 2026-10-10 | major | First file format parity document, from the After Effects 26.5 bundle and this repository's import/export registries |

## After Effects desktop bridge (experimental, partial)

Direct `.aep` / `.aepx` reading and writing remain **unsupported**. Those files are never renamed to `.unw`, decoded by this bridge or overwritten. The bridge transfers a deliberately limited subset through the **public After Effects desktop scripting API**. AE desktop is required for conversion; it does not run on iOS.

Adobe documents `.aep` as binary and `.aepx` as XML containing some hexadecimal-encoded binary data. An XML extension alone therefore does not make arbitrary project properties readable by this app. AE can open `.aepx` and save it as `.aep`; that does not establish a native iOS codec. See [Adobe's project-format documentation](https://helpx.adobe.com/after-effects/desktop/work-with-projects/after-effects-projects/projects.html).

The supplied `marblesoda.aep` was inspected only as a container: its 1,501,872-byte header is `RIFX … Egg!`, a native binary AEP container rather than JSON or a documented interchange stream. It is therefore a valid example for the **select-and-convert-through-After-Effects** workflow, not an input that Unwritten can decode directly. No sample bytes or proprietary decoder are included.

### Bring an AE project into Unwritten Studio

1. Choose **File > After Effects Bridge > Save AE Import Helper JSX…** and save the script using Files on iOS (or the native desktop save dialog).
2. On a computer with After Effects, run that helper through **File > Scripts > Run Script File…**. It offers the current project or an existing `.aep` / `.aepx` chosen with the public AE file dialog. Selecting another project opens it only after the current project is confirmed clean; an unsaved current project is never closed or discarded. The source project is read without edits and the helper asks where to save a `.unwae` interchange.
3. Open or import `.unwae` in Unwritten Studio. Opening creates an **untitled native project**, so Save asks for a new `.unw`; it cannot overwrite the interchange or source AEP. Transfer footage separately and relink unavailable paths.

### Take a native project into AE

Choose **File > After Effects Bridge > Export Project to AE JSX…**. Run the generated JSX on AE desktop. It creates compositions in the current AE project (start with an empty project for a clean reconstruction), reports warnings, and optionally asks for a new `.aep` name. It refuses to overwrite the currently opened source AEP and asks before replacing another existing destination. Cancelling the optional save keeps the imported compositions open without writing an AEP.

### What transfers

The v1 subset is composition dimensions/rate/duration/background, nested precomps, solids, nulls, footage references, static base text content/font/size/fill, layer order and timing, and unparented 2D anchor/position/scale/rotation/opacity with linear or hold transform keys. Key times are converted between AE composition time and native layer time, including positive time stretch. Bezier easing/spatial curves are approximated as linear with warnings.

Audio, colour management, plugins/effects, masks, expressions, 3D/cameras/lights, shapes, parenting, mattes, adjustment layers, reverse stretch, time remapping, sequences, markers and advanced/mixed text styling are **not preserved**. Unsupported layers are omitted or the affected features are explicitly warned about, not silently replaced by invented content. Footage metadata/interpretation is approximate; media is not embedded. Keep the native project and original AEP; this is not a fidelity-preserving round trip. No successful conversion of an arbitrary binary AEP is claimed.

Incoming `.unwae` is fully validated before replacement: version, bounded dimensions/times, positive stretch, finite values, monotonic keys, unique composition ids and an acyclic precomp graph. Limits: 32 MiB input, 512 compositions, 20,000 layers, 100,000 transform keys, 64 nesting levels. Corrupt/newer schemas fail without replacing the current document.

Synthetic coverage: `cargo test -p unwritten-studio-engine ae_bridge`, native-format/overwrite guards, and `node scripts/test-ae-bridge.cjs` (a mock AE DOM without a JSON global). These are not certification against a real After Effects installation. Public API behaviour: [AE project scripting](https://ae-scripting.docsforadobe.dev/general/project/), [Adobe project formats](https://helpx.adobe.com/after-effects/desktop/work-with-projects/after-effects-projects/projects.html).
