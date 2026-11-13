# Text performance improvements (done)

Done items of [../todo/text-performance-improvements.md](../todo/text-performance-improvements.md),
proposals following the measurements in [../text-performance.md](../text-performance.md).

## Cheaper uniform names

**Problem**: uniforms are looked up and copied by `String` name: cloning inherited uniforms,
building dynamic uniforms, setting glyph rectangles in `Text`, and matching shader members when
packing each draw.

**Proposal**: intern uniform names, giving each name a small id when it is first used
(`UniformName(u32)`, kept in a global or per-scene table). `Uniforms` stores ids and values
without allocations. Shader layouts store the ids of their members, so packing compares
integers. The public API can keep taking `&str`.

**Done**: `UniformName` (`uniformname.rs`) interns names in one table for the process.
`Uniforms` is a `Vec` of (id, value) without a `HashMap`, and nodes refill their inherited and
dynamic lists in place. Shader layout members and samplers are named by `UniformName`, and
`Shader` keeps its engine uniform names as ids (`ShaderUniformIds`). `Text` sets glyph
rectangles by id.

The CPU benchmark `text::tests::uniform_benchmark` (2,000 glyph nodes, no GPU;
`cargo test --release -p feather -- --ignored uniform_benchmark --nocapture`), measured on
2026-11-11, in ms per frame (mean of three runs each):

| | before | after |
|---|---|---|
| `set_text` of all 40 lines (glyph uniforms and transforms) | 0.58 | 0.27 |
| `update_nodes` | 2.98 | 0.41 |
| packing every draw's uniforms | 2.44 | 2.10 |

Packing barely changed: its cost is now mostly the per-draw allocations (*Fewer allocations per draw in `collect_draws`* in
[../todo/text-performance-improvements.md](../todo/text-performance-improvements.md)). The frame-time
scenarios of [../text-performance.md](../text-performance.md) weren't re-run (they need a
temporary demo that isn't in the repository).

**Check**: setting the four rectangle uniforms of 2,000 glyph nodes (1.8 ms) should get close
to the cost of one-mesh layout, and prepare and collect should drop further.
