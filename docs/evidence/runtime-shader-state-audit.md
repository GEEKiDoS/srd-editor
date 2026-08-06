# SRD runtime shader-state audit boundary

This audit separates three facts that must not be conflated:

1. the exact compact key selected by the currently proven ShapeEnv inputs;
2. membership in the game's 82-key `SimpleShaderVSSimpleShaderPS` collection;
3. availability of the corresponding verified bytecode in the editor.

`src/bin/srd-runtime-state-audit.rs` scans all 91 files under the local
`surfboard` corpus. Its default mode builds a conservative upper bound for
Image/Slice/Number texture-presence changes: an unanimated CREF/CRE1 channel
keeps its exact initial presence, while an animated channel may be absent or
may use any valid texture referenced by that channel. It also includes both
the authored layer dimension and every independently copied RefCast dimension
recorded by `ProjectRuntime`.

The current complete corpus reports:

```text
files=91
potential_upper_bound_shader_keys=82
potential_upper_bound_unpacked_keys=51
potential_upper_bound_keys_outside_game_collection=51
```

The equality of the last two counts is the useful result: every key in this
conservative upper bound that is present in the original 82-key collection now
has an embedded VS/PS pair. The 51 remaining keys are all outside the original
collection; the audit does not claim that any of them reaches a shipped draw.
They include deliberately broad CREF/CRE1 combinations and authored states
that may be disabled, invisible, or replaced by another scene/pass context.
They remain diagnostics, not permission to compile or substitute new shaders.

The existing exact initial-state corpus test remains narrower and reports
19,484 image bases, 65 direct ShapeEnv keys, six keys represented only by a
position-2 sibling, and 45 direct keys outside the collection. Those numbers
also do not establish runtime reachability because that test intentionally
ignores ANMS layer gates and world visibility.

The non-text audit also covers 18,192 Image/Slice/Number node/dimension
contexts (the difference is the 1,292 TextCast definitions). Their exact
initial packet states contain 216 alpha-test contexts across six keys and zero
stencil contexts. Nineteen alpha-test contexts use broad direct keys outside
the original collection; the other 197 already have exact packaged collection
shaders. SrImage alpha/stencil fields are not animation targets, so CREF/CRE1
animation does not turn the zero authored stencil result into a stencil path.

The optional `--integer-frames` mode uses fresh runtime state for each frame.
It reduces repeated tails exactly: non-wrapped tracks are constant after their
last range end, while wrapped integer-frame tracks repeat after the least
common multiple of their selected spans. Geometry-heavy full-corpus execution
is retained as a diagnostic tool, but no incomplete run is recorded as a
result here.

Two concrete hosts have complete integer-cycle results:

```text
AdvertiseLogo / CHU_UI_Advertise_00_v10.srd
  ANMS=19, covered frames=2303, draws=82158
  distinct keys=7, unpackaged=0, outside collection=0
  texture masks: none=898, slot0=62640, slot0+slot1=18620
  stencil=0, alpha-test=0

CommonBackGround / CHU_UI_Common_BK_00_v11.srd
  ANMS=10, covered frames=19580, draws=2644127
  distinct keys=6, unpackaged=0, outside collection=0
  texture masks: none=51599, slot0=2454566, slot0+slot1=137962
  stencil=0
  alpha-test draws=8248, exact key AAEBABBAABCBAAAAAA
```

For Common `ANMS[1] blue_in`, frame `1`, the alpha-test path is exactly eight
Image draws at `LAYR[5]/NODE[10..17]`. A corpus regression fixes that count,
node sequence and key. The D3D9Ex smoke submits all 561 Image/Slice sources and
then submits those eight alpha-test sources alone; both ResetEx passes succeed.
The alpha-test-only readback changes zero RGB pixels at that frame, which is
recorded rather than treated as a failure: successful packet/shader submission
and final visibility are separate facts, and later/failed-alpha pixels must not
be invented to make a diagnostic nonzero.

Advertise `ANMS[10] AS_title_in`, frame `0`, contains ten dual-texture Image
draws at `LAYR[3]/NODE[41,44,47,52,54,55,56,57,60,62]`. Three select
`AAEBABBAADIIEAAAAA` (MultiTex0 value 9), seven select
`AAEBABBAADIAAAAAAA`; every draw binds both stage 0 and stage 1 from TEXL.
At frame `30`, the D3D9Ex smoke submits only these ten sources and changes all
2,073,600 Composition RGB pixels before and after ResetEx. This proves the
shipped dual-texture packet, both TEXL bindings, verified collection shaders
and merged format-14 submission reach the GPU without inventing an override.

The unified runtime draw builder no longer drops Image, Slice, or Number draws
merely because a shader key is not packaged or packet stencil is enabled. It
preserves the exact key and packet so audits can observe the state. The D3D9Ex
backend still rejects an unregistered key, and the merged submission planner
still rejects stencil sequence state whose renderer lifetime is not closed;
neither path silently substitutes or merges an unsupported state.

The complete collection packaging and its HAL evidence are documented in
[`render-shader-bytecode.md`](render-shader-bytecode.md). The unresolved work is
the executable-derived scene/pass context that determines which of the broad
direct states can actually reach a submitted draw.
