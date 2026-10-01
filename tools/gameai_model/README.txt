Python models used by docs/retail-gameai-semantics.md (run from this directory, e.g. `python cmp.py 3000`).
retail.py  parser+node structure of cshell.dll 0x10015cc0 (+ script statistics: `python retail.py`)
sim.py     executor model of 0x1001a140 (class Retail) and of crates/mission-runtime/src/lib.rs tick/condition/execute (class Remake)
cmp.py     per-action comparison of the two models over random flag/world states (all 213 actions)
order2.py  whole-level frames: retail node semantics in script order vs the remake (shows the order is the only structural difference)
static.py, latch.py  static same-frame interaction analysis (which actions depend on the execution order)
order.py  whole-level frames: retail reverse order vs remake forward order
