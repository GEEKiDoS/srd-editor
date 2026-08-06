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

The optional `--integer-frames` mode uses fresh runtime state for each frame.
It reduces repeated tails exactly: non-wrapped tracks are constant after their
last range end, while wrapped integer-frame tracks repeat after the least
common multiple of their selected spans. Geometry-heavy full-corpus execution
is retained as a diagnostic tool, but no incomplete run is recorded as a
result here.

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
